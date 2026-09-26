use super::*;

struct ValueArm {
    pattern: hir::Pattern,
    guard: Option<hir::WhenGuard>,
    body: ValueBlock,
    span: Span,
}

#[derive(Clone, Copy)]
struct ValueArmFlowFacts {
    pattern_irrefutable: bool,
    /// `None` is an unguarded arm; `Some` records whether guard setup can
    /// reach its condition.
    guard_setup_falls_through: Option<bool>,
}

struct ValueArmPlan {
    deferred: bool,
    body_reachable: bool,
}

fn value_arm_reachability(facts: &[ValueArmFlowFacts]) -> (Vec<bool>, bool) {
    let mut prefix_reachable = true;
    let mut bodies = Vec::with_capacity(facts.len());
    for fact in facts {
        let guard_reaches_condition = fact.guard_setup_falls_through.unwrap_or(true);
        bodies.push(prefix_reachable && guard_reaches_condition);
        let can_try_next =
            !fact.pattern_irrefutable || matches!(fact.guard_setup_falls_through, Some(true));
        prefix_reachable = prefix_reachable && can_try_next;
    }
    (bodies, prefix_reachable)
}

impl Lowerer {
    /// Statement-position pattern `when` (spec 5). The subject must be an
    /// enum, tuple, struct or fixed-width integer — the current
    /// pattern-matching subset has no
    /// Kotlin-style condition `when`. Pattern bindings
    /// scope over the arm's guard and body; exhaustiveness is checked
    /// over the whole statement.
    pub(super) fn lower_when(
        &mut self,
        when: &ast::When,
        out: &mut Vec<hir::Statement>,
    ) -> Option<hir::StatementKind> {
        let mut sink = Vec::new();
        let subject = self.lower_expr(&when.subject, &mut sink, None)?;
        if !matches!(
            self.types[subject.ty],
            Type::Enum(..)
                | Type::ImportedEnum(_)
                | Type::Tuple(..)
                | Type::Struct(..)
                | Type::Integer(_)
        ) {
            let found = self.type_name(subject.ty);
            self.error(
                when.subject.span(),
                format!(
                    "`when` subject must be an enum, tuple, struct or fixed-width integer, found {found}"
                ),
            );
            return None;
        }
        // The subject is evaluated exactly once, right before the
        // `when`, so desugaring statements belong before it.
        out.extend(sink);

        let mut arms = Vec::with_capacity(when.arms.len());
        let mut arms_ok = true;
        for arm in &when.arms {
            self.push_scope();
            let lowered = self.lower_arm(arm, subject.ty);
            self.pop_scope();
            match lowered {
                Some(arm) => arms.push(arm),
                None => arms_ok = false,
            }
        }
        let else_body = when.else_body.as_ref().map(|b| self.lower_block(b));
        // A rejected arm leaves coverage information incomplete. Do not emit
        // a malformed `when` that a downstream stage could mistake for a
        // checked decision plan.
        if !arms_ok {
            return None;
        }
        let fallback = match else_body {
            Some(body) => hir::WhenFallback::Else(body),
            None => hir::WhenFallback::Impossible(
                self.prove_exhaustiveness(when.span, subject.ty, &arms)?,
            ),
        };
        Some(hir::StatementKind::When(hir::When {
            subject,
            arms,
            fallback,
        }))
    }

    /// Pattern `when` in value position. Arms that need context are lowered
    /// after context-independent arms establish a provisional result type;
    /// source order is restored in the resulting HIR.
    pub(crate) fn lower_when_expression(
        &mut self,
        when: &ast::When,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        let mut subject_sink = Vec::new();
        let subject = self.lower_expr(&when.subject, &mut subject_sink, None)?;
        if !matches!(
            self.types[subject.ty],
            Type::Enum(..)
                | Type::ImportedEnum(_)
                | Type::Tuple(..)
                | Type::Struct(..)
                | Type::Integer(_)
        ) {
            let found = self.type_name(subject.ty);
            self.error(
                when.subject.span(),
                format!(
                    "`when` subject must be an enum, tuple, struct or fixed-width integer, found {found}"
                ),
            );
            return None;
        }

        let probes: Vec<_> = when
            .arms
            .iter()
            .map(|arm| self.probe_value_arm(arm, subject.ty))
            .collect();
        let facts: Vec<_> = probes.iter().map(|(_, _, facts)| *facts).collect();
        let (body_reachable, else_reachable) = value_arm_reachability(&facts);
        let plans: Vec<_> = probes
            .into_iter()
            .zip(body_reachable)
            .map(
                |((valid, requires_expected, _), body_reachable)| ValueArmPlan {
                    deferred: expected.is_none() && valid && requires_expected,
                    body_reachable,
                },
            )
            .collect();
        let defer_else = when
            .else_body
            .as_ref()
            .is_some_and(|body| expected.is_none() && self.value_block_requires_expected(body));
        let mut arms: Vec<Option<ValueArm>> = (0..when.arms.len()).map(|_| None).collect();
        for (index, arm) in when.arms.iter().enumerate() {
            if !plans[index].deferred {
                let branch_expected = if plans[index].body_reachable {
                    expected
                } else {
                    None
                };
                arms[index] = Some(self.lower_value_arm(arm, subject.ty, branch_expected)?);
            }
        }
        let mut else_value = match &when.else_body {
            Some(body) if !defer_else => {
                let branch_expected = if else_reachable { expected } else { None };
                Some(self.lower_value_block(body, branch_expected)?)
            }
            _ => None,
        };
        let has_hint = arms.iter().zip(&plans).any(|(arm, plan)| {
            plan.body_reachable
                && arm
                    .as_ref()
                    .is_some_and(|arm| arm.body.value.as_ref().is_some())
        }) || (else_reachable
            && else_value
                .as_ref()
                .is_some_and(|body| body.value.as_ref().is_some()));
        if !has_hint {
            let arm_seed = when
                .arms
                .iter()
                .enumerate()
                .filter(|(index, _)| {
                    plans[*index].body_reachable && arms[*index].is_none() && plans[*index].deferred
                })
                .filter_map(|(index, arm)| {
                    self.value_block_default_seed_rank(&arm.body)
                        .map(|rank| (rank, index, arm))
                })
                .max_by_key(|(rank, _, _)| *rank);
            let else_rank = (else_reachable && else_value.is_none())
                .then(|| {
                    when.else_body
                        .as_ref()
                        .and_then(|body| self.value_block_default_seed_rank(body))
                })
                .flatten();
            if let Some((rank, index, arm)) = arm_seed
                && Some(rank) >= else_rank
            {
                arms[index] = Some(self.lower_value_arm(arm, subject.ty, None)?);
            } else if else_rank.is_some() && defer_else {
                else_value = Some(self.lower_value_block(
                    when.else_body.as_ref().expect("deferred else exists"),
                    None,
                )?);
            }
        }
        let mut hint_types: Vec<_> = arms
            .iter()
            .zip(&plans)
            .filter(|(_, plan)| plan.body_reachable)
            .filter_map(|(arm, _)| {
                arm.as_ref()
                    .and_then(|arm| arm.body.value.as_ref().map(|value| value.ty))
            })
            .collect();
        if let Some(ty) = else_reachable
            .then(|| {
                else_value
                    .as_ref()
                    .and_then(|body| body.value.as_ref().map(|value| value.ty))
            })
            .flatten()
        {
            hint_types.push(ty);
        }
        let hint = (!hint_types.is_empty()).then(|| self.least_upper_bound(&hint_types));
        for (index, arm) in when.arms.iter().enumerate() {
            if arms[index].is_none() {
                let branch_expected = if plans[index].body_reachable {
                    hint
                } else {
                    None
                };
                arms[index] = Some(self.lower_value_arm(arm, subject.ty, branch_expected)?);
            }
        }
        if defer_else {
            let branch_expected = if else_reachable { hint } else { None };
            else_value = Some(self.lower_value_block(
                when.else_body.as_ref().expect("deferred else exists"),
                branch_expected,
            )?);
        }
        let mut arms: Vec<ValueArm> = arms
            .into_iter()
            .map(|arm| arm.expect("every when arm was lowered"))
            .collect();
        let mut block_refs: Vec<&mut ValueBlock> = arms
            .iter_mut()
            .zip(&plans)
            .filter(|(_, plan)| plan.body_reachable)
            .map(|(arm, _)| &mut arm.body)
            .collect();
        if else_reachable && let Some(else_value) = else_value.as_mut() {
            block_refs.push(else_value);
        }
        let result =
            self.finish_control_value("when", when.span, expected, block_refs.as_mut_slice())?;
        drop(block_refs);
        for (arm, plan) in arms.iter_mut().zip(&plans) {
            if !plan.body_reachable {
                arm.body.discard_value();
            }
        }
        if !else_reachable && let Some(else_value) = else_value.as_mut() {
            else_value.discard_value();
        }
        let arms: Vec<_> = arms
            .into_iter()
            .map(|arm| hir::WhenArm {
                pattern: arm.pattern,
                guard: arm.guard,
                body: arm.body.statements,
                span: arm.span,
            })
            .collect();
        let fallback = match else_value {
            Some(body) => hir::WhenFallback::Else(body.statements),
            None => hir::WhenFallback::Impossible(
                self.prove_exhaustiveness(when.span, subject.ty, &arms)?,
            ),
        };
        sink.extend(subject_sink);
        sink.push(hir::Statement {
            span: when.span,
            kind: hir::StatementKind::When(hir::When {
                subject,
                arms,
                fallback,
            }),
        });
        Some(result)
    }

    fn probe_value_arm(
        &self,
        arm: &ast::WhenArm,
        subject_ty: TypeId,
    ) -> (bool, bool, ValueArmFlowFacts) {
        let mut probe = self.clone();
        probe.push_scope();
        let head = probe.lower_arm_head(arm, subject_ty);
        let requires_expected = head.is_some() && probe.value_block_requires_expected(&arm.body);
        probe.pop_scope();
        match head {
            Some((pattern, guard)) => {
                let guard_setup_falls_through = guard
                    .as_ref()
                    .map(|guard| statements_control_outcomes(&guard.setup).can_fall_through());
                (
                    true,
                    requires_expected,
                    ValueArmFlowFacts {
                        pattern_irrefutable: crate::patterns::is_irrefutable(&pattern),
                        guard_setup_falls_through,
                    },
                )
            }
            None => (
                false,
                false,
                ValueArmFlowFacts {
                    // Main lowering reports the invalid head. Until then,
                    // keep later branches reachable and avoid deriving facts
                    // from rejected syntax.
                    pattern_irrefutable: false,
                    guard_setup_falls_through: arm.guard.as_ref().map(|_| true),
                },
            ),
        }
    }

    /// One `when` arm: pattern (its bindings are in scope), optional
    /// guard (must be `Boolean`), body block.
    fn lower_arm(&mut self, arm: &ast::WhenArm, subject_ty: TypeId) -> Option<hir::WhenArm> {
        let (pattern, guard) = self.lower_arm_head(arm, subject_ty)?;
        let body = self.lower_block(&arm.body);
        Some(hir::WhenArm {
            pattern,
            guard,
            body,
            span: arm.span,
        })
    }

    fn lower_value_arm(
        &mut self,
        arm: &ast::WhenArm,
        subject_ty: TypeId,
        expected: Option<TypeId>,
    ) -> Option<ValueArm> {
        self.push_scope();
        let lowered = (|| {
            let (pattern, guard) = self.lower_arm_head(arm, subject_ty)?;
            let body = self.lower_value_block(&arm.body, expected)?;
            Some(ValueArm {
                pattern,
                guard,
                body,
                span: arm.span,
            })
        })();
        self.pop_scope();
        lowered
    }

    fn lower_arm_head(
        &mut self,
        arm: &ast::WhenArm,
        subject_ty: TypeId,
    ) -> Option<(hir::Pattern, Option<hir::WhenGuard>)> {
        let pattern = self.lower_pattern(
            &arm.pattern,
            subject_ty,
            PatternCtx {
                mutable: false,
                in_when: true,
            },
        )?;
        let guard = match &arm.guard {
            Some(guard) => {
                let mut sink = Vec::new();
                let guard_expr = self.lower_expr(guard, &mut sink, None)?;
                if guard_expr.ty != self.boolean {
                    let found = self.type_name(guard_expr.ty);
                    self.error(
                        guard.span(),
                        format!("when guard must be Boolean, found {found}"),
                    );
                    return None;
                }
                Some(hir::WhenGuard {
                    // Setup executes after the pattern binds its locals and
                    // before the condition, once for this arm attempt.
                    setup: sink,
                    condition: guard_expr,
                })
            }
            None => None,
        };
        Some((pattern, guard))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts(
        pattern_irrefutable: bool,
        guard_setup_falls_through: Option<bool>,
    ) -> ValueArmFlowFacts {
        ValueArmFlowFacts {
            pattern_irrefutable,
            guard_setup_falls_through,
        }
    }

    #[test]
    fn value_arm_reachability_respects_pattern_and_guard_setup() {
        assert_eq!(
            value_arm_reachability(&[facts(false, Some(false))]),
            (vec![false], true),
            "a refutable pattern can proceed after an abrupt matched guard"
        );
        assert_eq!(
            value_arm_reachability(&[facts(true, Some(false))]),
            (vec![false], false),
            "an irrefutable pattern cannot bypass an abrupt guard setup"
        );
        assert_eq!(
            value_arm_reachability(&[facts(true, Some(true))]),
            (vec![true], true),
            "a completing guard may enter the body or reject the arm"
        );
        assert_eq!(
            value_arm_reachability(&[facts(true, None), facts(false, None)]),
            (vec![true, false], false),
            "an unguarded irrefutable arm cuts off every later branch"
        );
    }
}
