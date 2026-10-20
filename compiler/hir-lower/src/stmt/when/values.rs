use super::*;

struct ValueArm {
    body: ValueBlock,
}

impl Lowerer {
    pub(super) fn lower_when_value(
        &mut self,
        when: &ast::When,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        let mut subject_sink = Vec::new();
        let lowered = match self.lower_when_subject(when, &mut subject_sink)? {
            Some(subject) => {
                let (heads, reachable) = self.prepare_when_arms(when, |this, arm, reachable| {
                    this.prepare_when_head(arm, reachable, |this| {
                        this.subject_primary(arm, &subject)
                    })
                })?;
                self.lower_when_value_arms(
                    when,
                    heads,
                    reachable,
                    expected,
                    |this, heads, arms, else_body| {
                        let fallback =
                            this.subject_when_fallback(when, &subject, &heads, else_body, true)?;
                        Some(hir::When::Subject {
                            subject: subject.value,
                            arms: attach_bodies(when, heads, arms),
                            fallback,
                        })
                    },
                )?
            }
            None => {
                let (heads, reachable) = self.prepare_when_arms(when, |this, arm, reachable| {
                    this.prepare_when_head(arm, reachable, |this| this.conditional_primary(arm))
                })?;
                self.lower_when_value_arms(
                    when,
                    heads,
                    reachable,
                    expected,
                    |this, heads, arms, else_body| {
                        let fallback = this.conditional_when_fallback(when, else_body, true)?;
                        Some(hir::When::Conditions {
                            arms: attach_bodies(when, heads, arms),
                            fallback,
                        })
                    },
                )?
            }
        };
        sink.extend(subject_sink);
        sink.push(hir::Statement {
            span: when.span,
            kind: hir::StatementKind::When(lowered.0),
        });
        Some(lowered.1)
    }

    fn lower_when_value_arms<C>(
        &mut self,
        when: &ast::When,
        heads: Vec<ArmHead<C>>,
        else_reachable: bool,
        expected: Option<TypeId>,
        finish: impl FnOnce(
            &mut Self,
            Vec<ArmHead<C>>,
            Vec<ValueArm>,
            Option<Vec<hir::Statement>>,
        ) -> Option<hir::When>,
    ) -> Option<(hir::When, hir::Expr)> {
        let mut arms: Vec<Option<ValueArm>> = (0..when.arms.len()).map(|_| None).collect();
        let mut failures = (0..when.arms.len()).map(|_| None).collect::<Vec<_>>();
        let mut else_failure = None;
        for (index, arm) in when.arms.iter().enumerate() {
            let branch_expected = if heads[index].body_reachable {
                expected
            } else {
                None
            };
            arms[index] = self.probe_contextual_value(
                heads[index].context_requirement,
                branch_expected,
                &mut failures[index],
                |this, expected| this.lower_value_arm(arm, &heads[index], expected),
            )?;
        }
        let mut else_value = match &when.else_body {
            Some(body) => {
                let branch_expected = if else_reachable { expected } else { None };
                self.probe_contextual_value(
                    self.value_block_context_requirement(body),
                    branch_expected,
                    &mut else_failure,
                    |this, expected| this.lower_value_block(body, expected),
                )?
            }
            None => None,
        };
        let has_hint = arms.iter().zip(&heads).any(|(arm, plan)| {
            plan.body_reachable
                && arm
                    .as_ref()
                    .is_some_and(|arm| arm.body.value.as_ref().is_some())
        }) || (else_reachable
            && else_value
                .as_ref()
                .is_some_and(|body| body.value.as_ref().is_some()));
        if !has_hint {
            let mut seeds = when
                .arms
                .iter()
                .enumerate()
                .filter(|(index, _)| {
                    heads[*index].body_reachable
                        && arms[*index].is_none()
                        && failures[*index].is_none()
                })
                .filter_map(|(index, arm)| {
                    self.value_block_default_seed_rank(&arm.body)
                        .map(|rank| (rank, Some(index)))
                })
                .collect::<Vec<_>>();
            let else_rank = (else_reachable && else_value.is_none() && else_failure.is_none())
                .then(|| {
                    when.else_body
                        .as_ref()
                        .and_then(|body| self.value_block_default_seed_rank(body))
                })
                .flatten();
            seeds.extend(else_rank.map(|rank| (rank, None)));
            seeds.sort_by_key(|seed| std::cmp::Reverse(*seed));
            for (_, index) in seeds {
                let ready = if let Some(index) = index {
                    arms[index] = self.probe_contextual_value(
                        ExpressionContextRequirement::Unresolved,
                        None,
                        &mut failures[index],
                        |this, expected| {
                            this.lower_value_arm(&when.arms[index], &heads[index], expected)
                        },
                    )?;
                    arms[index].is_some()
                } else {
                    let body = when.else_body.as_ref().expect("an else seed has a body");
                    else_value = self.probe_contextual_value(
                        ExpressionContextRequirement::Unresolved,
                        None,
                        &mut else_failure,
                        |this, expected| this.lower_value_block(body, expected),
                    )?;
                    else_value.is_some()
                };
                if ready {
                    break;
                }
            }
        }
        let mut hint_types: Vec<_> = arms
            .iter()
            .zip(&heads)
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
                let branch_expected = if heads[index].body_reachable {
                    hint
                } else {
                    None
                };
                arms[index] = Some(self.finish_contextual_value(
                    branch_expected,
                    &mut failures[index],
                    |this, expected| this.lower_value_arm(arm, &heads[index], expected),
                )?);
            }
        }
        if else_value.is_none()
            && let Some(body) = &when.else_body
        {
            let branch_expected = if else_reachable { hint } else { None };
            else_value = Some(self.finish_contextual_value(
                branch_expected,
                &mut else_failure,
                |this, expected| this.lower_value_block(body, expected),
            )?);
        }
        let mut arms: Vec<ValueArm> = arms
            .into_iter()
            .map(|arm| arm.expect("every when arm was lowered"))
            .collect();
        let mut block_refs: Vec<&mut ValueBlock> = arms
            .iter_mut()
            .zip(&heads)
            .filter(|(_, plan)| plan.body_reachable)
            .map(|(arm, _)| &mut arm.body)
            .collect();
        if else_reachable && let Some(else_value) = else_value.as_mut() {
            block_refs.push(else_value);
        }
        let result =
            self.finish_control_value("when", when.span, expected, block_refs.as_mut_slice())?;
        drop(block_refs);
        for (arm, plan) in arms.iter_mut().zip(&heads) {
            if !plan.body_reachable {
                arm.body.discard_value();
            }
        }
        if !else_reachable && let Some(else_value) = else_value.as_mut() {
            else_value.discard_value();
        }
        let when = finish(self, heads, arms, else_value.map(|body| body.statements))?;
        Some((when, result))
    }

    fn lower_value_arm<C>(
        &mut self,
        arm: &ast::WhenArm,
        head: &ArmHead<C>,
        expected: Option<TypeId>,
    ) -> Option<ValueArm> {
        self.in_when_arm(head, |this| {
            this.lower_value_block(&arm.body, expected)
                .map(|body| ValueArm { body })
        })
    }
}

fn attach_bodies<C>(
    when: &ast::When,
    heads: Vec<ArmHead<C>>,
    arms: Vec<ValueArm>,
) -> Vec<hir::WhenArm<C>> {
    arms.into_iter()
        .zip(heads)
        .zip(&when.arms)
        .map(|((arm, head), source)| hir::WhenArm {
            condition: head.condition,
            guard: head.guard,
            body: arm.body.statements,
            span: source.span,
        })
        .collect()
}
