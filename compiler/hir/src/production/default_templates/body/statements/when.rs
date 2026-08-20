use super::super::super::DefaultBodyProjectionError as Error;
use super::*;

impl BodyProjection<'_, '_> {
    pub(super) fn when(&mut self, when: &crate::When) -> Result<DefaultWhenV1, Error> {
        match when {
            crate::When::Subject {
                subject,
                arms,
                fallback,
            } => {
                let subject = self.expression(subject)?;
                let arms = arms
                    .iter()
                    .map(|arm| {
                        self.when_arm(arm, |this, condition| {
                            Ok(match condition {
                                crate::WhenCondition::Case(pattern) => {
                                    DefaultWhenConditionV1::Case(this.pattern(pattern)?)
                                }
                                crate::WhenCondition::Predicate(predicate) => {
                                    DefaultWhenConditionV1::Predicate(Box::new(
                                        this.when_guard(predicate)?,
                                    ))
                                }
                                crate::WhenCondition::Always => DefaultWhenConditionV1::Always,
                            })
                        })
                    })
                    .collect::<Result<_, Error>>()?;
                let fallback = self.when_fallback(fallback)?;
                DefaultWhenV1::try_subject(subject, arms, fallback).map_err(Error::ControlFlow)
            }
            crate::When::Conditions { arms, fallback } => {
                let arms = arms
                    .iter()
                    .map(|arm| {
                        self.when_arm(arm, |this, condition| {
                            Ok(match condition {
                                crate::WhenPredicate::Predicate(predicate) => {
                                    DefaultWhenConditionV1::Predicate(Box::new(
                                        this.when_guard(predicate)?,
                                    ))
                                }
                                crate::WhenPredicate::Always => DefaultWhenConditionV1::Always,
                            })
                        })
                    })
                    .collect::<Result<_, Error>>()?;
                let fallback = self.when_fallback(fallback)?;
                DefaultWhenV1::try_conditions(arms, fallback).map_err(Error::ControlFlow)
            }
        }
    }

    fn when_arm<C, P>(
        &mut self,
        arm: &crate::WhenArm<C>,
        condition: impl FnOnce(&mut Self, &C) -> Result<DefaultWhenConditionV1<P>, Error>,
    ) -> Result<DefaultWhenArmV1<P>, Error> {
        let guard = match &arm.guard {
            Some(guard) => OptionalDefaultWhenGuardV1::present(self.when_guard(guard)?),
            None => OptionalDefaultWhenGuardV1::Absent,
        };
        let condition = condition(self, &arm.condition)?;
        DefaultWhenArmV1::try_new(
            condition,
            guard,
            self.statements(&arm.body)?,
            self.span_origin(arm.span)?,
        )
        .map_err(Error::ControlFlow)
    }

    fn when_guard(&mut self, guard: &crate::WhenGuard) -> Result<DefaultWhenGuardV1, Error> {
        DefaultWhenGuardV1::try_new(
            self.statements(&guard.setup)?,
            self.expression(&guard.condition)?,
        )
        .map_err(Error::ControlFlow)
    }

    fn when_fallback<P: Copy>(
        &mut self,
        fallback: &WhenFallback<P>,
    ) -> Result<DefaultWhenFallbackV1<P>, Error> {
        Ok(match fallback {
            WhenFallback::Fallthrough => DefaultWhenFallbackV1::Fallthrough,
            WhenFallback::Else(body) => DefaultWhenFallbackV1::try_else(self.statements(body)?)
                .map_err(Error::ControlFlow)?,
            WhenFallback::Impossible(proof) => DefaultWhenFallbackV1::Impossible(*proof),
        })
    }
}
