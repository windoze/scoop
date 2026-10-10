use super::*;

type Error = ImportedDefaultMaterializationError;

impl Lowerer {
    pub(super) fn materialize_imported_default_when(
        &mut self,
        value: &hir::DefaultWhenV1,
        context: &mut ImportedDefaultContext<'_>,
    ) -> Result<hir::When, Error> {
        Ok(match value {
            hir::DefaultWhenV1::Subject {
                subject,
                arms,
                fallback,
            } => {
                let subject = self.materialize_imported_default_expression(subject, context)?;
                let arms = arms
                    .iter()
                    .map(|arm| {
                        let condition = match arm.condition() {
                            hir::DefaultWhenConditionV1::Case(pattern) => hir::WhenCondition::Case(
                                self.materialize_imported_default_pattern(pattern, context)?,
                            ),
                            hir::DefaultWhenConditionV1::Predicate(predicate) => {
                                hir::WhenCondition::Predicate(
                                    self.materialize_when_guard(predicate, context)?,
                                )
                            }
                            hir::DefaultWhenConditionV1::Always => hir::WhenCondition::Always,
                        };
                        self.materialize_when_arm(arm, condition, context)
                    })
                    .collect::<Result<_, Error>>()?;
                let fallback = self.materialize_when_fallback(fallback, context)?;
                hir::When::Subject {
                    subject,
                    arms,
                    fallback,
                }
            }
            hir::DefaultWhenV1::Conditions { arms, fallback } => {
                let arms = arms
                    .iter()
                    .map(|arm| {
                        let condition = match arm.condition() {
                            hir::DefaultWhenConditionV1::Case(never) => match *never {},
                            hir::DefaultWhenConditionV1::Predicate(predicate) => {
                                hir::WhenPredicate::Predicate(
                                    self.materialize_when_guard(predicate, context)?,
                                )
                            }
                            hir::DefaultWhenConditionV1::Always => hir::WhenPredicate::Always,
                        };
                        self.materialize_when_arm(arm, condition, context)
                    })
                    .collect::<Result<_, Error>>()?;
                let fallback = self.materialize_when_fallback(fallback, context)?;
                hir::When::Conditions { arms, fallback }
            }
        })
    }

    fn materialize_when_arm<P, C>(
        &mut self,
        arm: &hir::DefaultWhenArmV1<P>,
        condition: C,
        context: &mut ImportedDefaultContext<'_>,
    ) -> Result<hir::WhenArm<C>, Error> {
        let guard = arm
            .guard()
            .as_ref()
            .map(|guard| self.materialize_when_guard(guard, context))
            .transpose()?;
        let body = self.materialize_imported_default_statements(arm.body(), context)?;
        let span = self.imported_statement_span(arm.definition_origin(), context)?;
        Ok(hir::WhenArm {
            condition,
            guard,
            body,
            span,
        })
    }

    fn materialize_when_guard(
        &mut self,
        guard: &hir::DefaultWhenGuardV1,
        context: &mut ImportedDefaultContext<'_>,
    ) -> Result<hir::WhenGuard, Error> {
        Ok(hir::WhenGuard {
            setup: self.materialize_imported_default_statements(guard.setup(), context)?,
            condition: self.materialize_imported_default_expression(guard.condition(), context)?,
        })
    }

    fn materialize_when_fallback<P: Copy>(
        &mut self,
        fallback: &hir::DefaultWhenFallbackV1<P>,
        context: &mut ImportedDefaultContext<'_>,
    ) -> Result<hir::WhenFallback<P>, Error> {
        Ok(match fallback {
            hir::DefaultWhenFallbackV1::Fallthrough => hir::WhenFallback::Fallthrough,
            hir::DefaultWhenFallbackV1::Else(body) => hir::WhenFallback::Else(
                self.materialize_imported_default_statements(body, context)?,
            ),
            hir::DefaultWhenFallbackV1::Impossible(proof) => hir::WhenFallback::Impossible(*proof),
        })
    }
}
