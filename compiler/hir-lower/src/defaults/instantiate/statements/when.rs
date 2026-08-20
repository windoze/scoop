use super::*;

impl Lowerer {
    pub(super) fn instantiate_default_when(
        &mut self,
        when: &hir::When,
        context: &mut InstantiationContext,
    ) -> hir::When {
        match when {
            hir::When::Subject {
                subject,
                arms,
                fallback,
            } => {
                let subject = self.instantiate_default_expr(subject, context);
                let arms = arms
                    .iter()
                    .map(|arm| {
                        let condition = match &arm.condition {
                            hir::WhenCondition::Case(pattern) => hir::WhenCondition::Case(
                                self.instantiate_default_pattern(pattern, context),
                            ),
                            hir::WhenCondition::Predicate(predicate) => {
                                hir::WhenCondition::Predicate(
                                    self.instantiate_when_guard(predicate, context),
                                )
                            }
                            hir::WhenCondition::Always => hir::WhenCondition::Always,
                        };
                        self.instantiate_when_arm(arm, condition, context)
                    })
                    .collect();
                let fallback = self.instantiate_when_fallback(fallback, context);
                hir::When::Subject {
                    subject,
                    arms,
                    fallback,
                }
            }
            hir::When::Conditions { arms, fallback } => {
                let arms = arms
                    .iter()
                    .map(|arm| {
                        let condition = match &arm.condition {
                            hir::WhenPredicate::Predicate(predicate) => {
                                hir::WhenPredicate::Predicate(
                                    self.instantiate_when_guard(predicate, context),
                                )
                            }
                            hir::WhenPredicate::Always => hir::WhenPredicate::Always,
                        };
                        self.instantiate_when_arm(arm, condition, context)
                    })
                    .collect();
                let fallback = self.instantiate_when_fallback(fallback, context);
                hir::When::Conditions { arms, fallback }
            }
        }
    }

    fn instantiate_when_arm<C, D>(
        &mut self,
        arm: &hir::WhenArm<C>,
        condition: D,
        context: &mut InstantiationContext,
    ) -> hir::WhenArm<D> {
        hir::WhenArm {
            condition,
            guard: arm
                .guard
                .as_ref()
                .map(|guard| self.instantiate_when_guard(guard, context)),
            body: arm
                .body
                .iter()
                .map(|statement| self.instantiate_default_statement(statement, context))
                .collect(),
            span: context.statement_span,
        }
    }

    fn instantiate_when_guard(
        &mut self,
        guard: &hir::WhenGuard,
        context: &mut InstantiationContext,
    ) -> hir::WhenGuard {
        hir::WhenGuard {
            setup: guard
                .setup
                .iter()
                .map(|statement| self.instantiate_default_statement(statement, context))
                .collect(),
            condition: self.instantiate_default_expr(&guard.condition, context),
        }
    }

    fn instantiate_when_fallback<P: Copy>(
        &mut self,
        fallback: &hir::WhenFallback<P>,
        context: &mut InstantiationContext,
    ) -> hir::WhenFallback<P> {
        match fallback {
            hir::WhenFallback::Else(body) => hir::WhenFallback::Else(
                body.iter()
                    .map(|statement| self.instantiate_default_statement(statement, context))
                    .collect(),
            ),
            hir::WhenFallback::Fallthrough => hir::WhenFallback::Fallthrough,
            hir::WhenFallback::Impossible(proof) => hir::WhenFallback::Impossible(*proof),
        }
    }
}
