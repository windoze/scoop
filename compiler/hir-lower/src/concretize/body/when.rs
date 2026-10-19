use super::*;

impl Concretizer<'_> {
    pub(super) fn lower_when(
        &mut self,
        source: &export::When,
        substitution: &[concrete::TypeId],
        locals: &[concrete::LocalId],
        loops: &mut LoopRemap,
        storage: &mut Arena<concrete::Local>,
    ) -> concrete::When {
        match source {
            export::When::Subject {
                subject,
                arms,
                fallback,
            } => {
                let subject = self.lower_expr(subject, substitution, locals);
                let arms = arms
                    .iter()
                    .map(|arm| {
                        let condition = match &arm.condition {
                            export::WhenCondition::Case(pattern) => concrete::WhenCondition::Case(
                                self.lower_pattern(pattern, subject.ty, substitution, locals),
                            ),
                            export::WhenCondition::Predicate(predicate) => {
                                concrete::WhenCondition::Predicate(self.lower_when_guard(
                                    predicate,
                                    substitution,
                                    locals,
                                    loops,
                                    storage,
                                ))
                            }
                            export::WhenCondition::Always => concrete::WhenCondition::Always,
                        };
                        self.lower_when_arm(arm, condition, substitution, locals, loops, storage)
                    })
                    .collect();
                let fallback =
                    self.lower_when_fallback(fallback, substitution, locals, loops, storage);
                concrete::When::Subject {
                    subject,
                    arms,
                    fallback,
                }
            }
            export::When::Conditions { arms, fallback } => {
                let arms = arms
                    .iter()
                    .map(|arm| {
                        let condition = match &arm.condition {
                            export::WhenPredicate::Predicate(predicate) => {
                                concrete::WhenPredicate::Predicate(self.lower_when_guard(
                                    predicate,
                                    substitution,
                                    locals,
                                    loops,
                                    storage,
                                ))
                            }
                            export::WhenPredicate::Always => concrete::WhenPredicate::Always,
                        };
                        self.lower_when_arm(arm, condition, substitution, locals, loops, storage)
                    })
                    .collect();
                let fallback =
                    self.lower_when_fallback(fallback, substitution, locals, loops, storage);
                concrete::When::Conditions { arms, fallback }
            }
        }
    }

    fn lower_when_arm<C, D>(
        &mut self,
        arm: &export::WhenArm<C>,
        condition: D,
        substitution: &[concrete::TypeId],
        locals: &[concrete::LocalId],
        loops: &mut LoopRemap,
        storage: &mut Arena<concrete::Local>,
    ) -> concrete::WhenArm<D> {
        concrete::WhenArm {
            condition,
            guard: arm
                .guard
                .as_ref()
                .map(|guard| self.lower_when_guard(guard, substitution, locals, loops, storage)),
            body: self.lower_statements(&arm.body, substitution, locals, loops, storage),
            span: arm.span,
        }
    }

    fn lower_when_guard(
        &mut self,
        guard: &export::WhenGuard,
        substitution: &[concrete::TypeId],
        locals: &[concrete::LocalId],
        loops: &mut LoopRemap,
        storage: &mut Arena<concrete::Local>,
    ) -> concrete::WhenGuard {
        concrete::WhenGuard {
            setup: self.lower_statements(&guard.setup, substitution, locals, loops, storage),
            condition: self.lower_expr(&guard.condition, substitution, locals),
        }
    }

    fn lower_when_fallback<P: Copy>(
        &mut self,
        fallback: &export::WhenFallback<P>,
        substitution: &[concrete::TypeId],
        locals: &[concrete::LocalId],
        loops: &mut LoopRemap,
        storage: &mut Arena<concrete::Local>,
    ) -> concrete::WhenFallback<P> {
        match fallback {
            export::WhenFallback::Else(body) => concrete::WhenFallback::Else(
                self.lower_statements(body, substitution, locals, loops, storage),
            ),
            export::WhenFallback::Fallthrough => concrete::WhenFallback::Fallthrough,
            export::WhenFallback::Impossible(proof) => concrete::WhenFallback::Impossible(*proof),
        }
    }
}
