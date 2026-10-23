use super::*;
use crate::stmt::when::ArmPrimary;

impl<F: Fn() -> bool> PrefixQuery<'_, F> {
    pub(super) fn when(
        &mut self,
        lowerer: &mut Lowerer,
        when: &ast::RecoveredWhen,
        available: bool,
    ) -> Result<(), EditingCancelled> {
        check_cancelled(self.is_cancelled)?;
        lowerer.in_when_scope(|lowerer| {
            let has_conditions = when.arms.iter().any(|arm| match arm {
                ast::RecoveredWhenArm::Head { condition, .. }
                | ast::RecoveredWhenArm::Guard { condition, .. } => {
                    matches!(condition, ast::WhenArmCondition::Conditions(_))
                }
                ast::RecoveredWhenArm::Condition { .. } | ast::RecoveredWhenArm::Unavailable(_) => {
                    true
                }
            });
            let subject = if available {
                let before = lowerer.diagnostics.len();
                lowerer
                    .lower_when_subject_input(&when.subject, has_conditions, &mut Vec::new())
                    .filter(|_| lowerer.diagnostics.len() == before)
            } else {
                None
            };
            match subject {
                Some(Some(subject)) => {
                    self.when_arms(lowerer, when, true, |lowerer, condition, _| {
                        lowerer.subject_primary(condition, &subject)
                    })
                }
                subject => self.when_arms(
                    lowerer,
                    when,
                    subject.is_some(),
                    |lowerer, condition, span| lowerer.conditional_primary(condition, span),
                ),
            }
        })
    }

    fn when_arms<C>(
        &mut self,
        lowerer: &mut Lowerer,
        when: &ast::RecoveredWhen,
        mut available: bool,
        mut prepare: impl FnMut(
            &mut Lowerer,
            &ast::WhenArmCondition,
            ast::Span,
        ) -> Option<ArmPrimary<C>>,
    ) -> Result<(), EditingCancelled> {
        let mut reachable = true;
        for arm in &when.arms {
            check_cancelled(self.is_cancelled)?;
            match arm {
                ast::RecoveredWhenArm::Head {
                    condition,
                    guard,
                    body,
                    span,
                } => {
                    lowerer.push_scope();
                    let before = lowerer.diagnostics.len();
                    let head = available
                        .then(|| {
                            lowerer.prepare_when_head(guard.as_ref(), &mut reachable, |lowerer| {
                                prepare(lowerer, condition, *span)
                            })
                        })
                        .flatten()
                        .filter(|_| lowerer.diagnostics.len() == before);
                    lowerer.pop_scope();
                    available = head.is_some();
                    if let Some(body) = body {
                        if let Some(head) = head {
                            lowerer.in_when_arm(&head, |lowerer| {
                                self.control_body(lowerer, when.block, *body, true)
                            })?;
                        } else {
                            self.control_body(lowerer, when.block, *body, false)?;
                        }
                    }
                }
                ast::RecoveredWhenArm::Guard {
                    condition,
                    expression_start,
                    span,
                } => {
                    return self.when_operand(
                        lowerer,
                        when.block,
                        *expression_start,
                        available,
                        |lowerer| {
                            let incoming = lowerer.smart_casts.clone();
                            let primary = prepare(lowerer, condition, *span)?;
                            Some(primary.paths.map_or(incoming, |paths| paths.0))
                        },
                    );
                }
                ast::RecoveredWhenArm::Condition {
                    previous,
                    expression_start,
                    span,
                } => {
                    return self.when_operand(
                        lowerer,
                        when.block,
                        *expression_start,
                        available,
                        |lowerer| {
                            let Some((first, rest)) = previous.split_first() else {
                                return Some(lowerer.smart_casts.clone());
                            };
                            let condition = ast::WhenArmCondition::Conditions(
                                ast::NonEmptyVec::new(first.clone(), rest.to_vec()),
                            );
                            let primary = prepare(lowerer, &condition, *span)?;
                            Some(
                                primary
                                    .paths
                                    .map_or_else(|| lowerer.smart_casts.clone(), |paths| paths.1),
                            )
                        },
                    );
                }
                ast::RecoveredWhenArm::Unavailable(_) => return Ok(()),
            }
        }
        Ok(())
    }

    fn when_operand(
        &mut self,
        lowerer: &mut Lowerer,
        block: Option<ast::BodyBlockId>,
        start: u32,
        available: bool,
        prepare: impl FnOnce(&mut Lowerer) -> Option<crate::expr::SmartCastFacts>,
    ) -> Result<(), EditingCancelled> {
        lowerer.push_scope();
        let before = lowerer.diagnostics.len();
        let facts = available
            .then(|| prepare(lowerer))
            .flatten()
            .filter(|_| lowerer.diagnostics.len() == before);
        let available = facts.is_some();
        let result = lowerer.with_smart_casts(facts.unwrap_or_default(), |lowerer| {
            self.expression(lowerer, block, start, available)
        });
        lowerer.pop_scope();
        result
    }
}
