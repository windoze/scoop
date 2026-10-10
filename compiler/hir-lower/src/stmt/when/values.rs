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
        let subject = self.lower_when_subject(when, &mut subject_sink)?;
        let (heads, else_reachable) = self.prepare_when_arms(when, subject.as_ref())?;
        let defer_else = when
            .else_body
            .as_ref()
            .is_some_and(|body| expected.is_none() && self.value_block_requires_expected(body));
        let mut arms: Vec<Option<ValueArm>> = (0..when.arms.len()).map(|_| None).collect();
        for (index, arm) in when.arms.iter().enumerate() {
            if !(expected.is_none() && heads[index].requires_expected) {
                let branch_expected = if heads[index].body_reachable {
                    expected
                } else {
                    None
                };
                arms[index] = Some(self.lower_value_arm(arm, &heads[index], branch_expected)?);
            }
        }
        let mut else_value = match &when.else_body {
            Some(body) if !defer_else => {
                let branch_expected = if else_reachable { expected } else { None };
                Some(self.lower_value_block(body, branch_expected)?)
            }
            _ => None,
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
            let arm_seed = when
                .arms
                .iter()
                .enumerate()
                .filter(|(index, _)| {
                    heads[*index].body_reachable
                        && arms[*index].is_none()
                        && (expected.is_none() && heads[*index].requires_expected)
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
                arms[index] = Some(self.lower_value_arm(arm, &heads[index], None)?);
            } else if else_rank.is_some() && defer_else {
                else_value = Some(self.lower_value_block(
                    when.else_body.as_ref().expect("deferred else exists"),
                    None,
                )?);
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
                arms[index] = Some(self.lower_value_arm(arm, &heads[index], branch_expected)?);
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
        let fallback = self.when_fallback(
            when,
            subject.as_ref(),
            &heads,
            else_value.map(|body| body.statements),
            true,
        )?;
        let arms = arms
            .into_iter()
            .zip(heads)
            .zip(&when.arms)
            .map(|((arm, head), source)| hir::WhenArm {
                condition: head.condition,
                guard: head.guard,
                body: arm.body.statements,
                span: source.span,
            })
            .collect();
        sink.extend(subject_sink);
        sink.push(hir::Statement {
            span: when.span,
            kind: hir::StatementKind::When(hir::When {
                subject: subject.map(|subject| subject.value),
                arms,
                fallback,
            }),
        });
        Some(result)
    }

    fn lower_value_arm(
        &mut self,
        arm: &ast::WhenArm,
        head: &ArmHead,
        expected: Option<TypeId>,
    ) -> Option<ValueArm> {
        self.in_when_arm(head, |this| {
            this.lower_value_block(&arm.body, expected)
                .map(|body| ValueArm { body })
        })
    }
}
