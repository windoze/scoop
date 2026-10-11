use super::super::{BodyNode, DefaultBodyReferenceVisitorV1, ReferenceWalker, ScheduledWork};
use crate::{
    DefaultBodyProviderTypeSiteV1, DefaultCatchV1, DefaultTryV1, DefaultWhenArmRefV1,
    DefaultWhenConditionV1, DefaultWhenFallbackViewV1, DefaultWhenGuardV1, DefaultWhenV1,
    ExportDefinitionSourceV1, OptionalDefaultStatementListViewV1,
};

impl<'body, V: DefaultBodyReferenceVisitorV1<'body>> ReferenceWalker<'_, 'body, V> {
    pub(in super::super) fn process_when(
        &mut self,
        value: &'body DefaultWhenV1,
        origin: &'body ExportDefinitionSourceV1,
        pending: &mut Vec<ScheduledWork<'body>>,
    ) -> Result<(), V::Error> {
        self.push_child(
            pending,
            BodyNode::WhenFallback {
                fallback: value.fallback(),
                origin,
            },
        )?;
        for arm in value.arms().rev() {
            self.push_child(pending, BodyNode::WhenArm(arm))?;
        }
        if let Some(subject) = value.subject() {
            self.push_child(pending, BodyNode::Expression(subject))?;
        }
        Ok(())
    }

    pub(in super::super) fn process_when_arm(
        &mut self,
        arm: DefaultWhenArmRefV1<'body>,
        pending: &mut Vec<ScheduledWork<'body>>,
    ) -> Result<(), V::Error> {
        self.push_statements(pending, arm.body())?;
        if let Some(guard) = arm.guard().as_ref() {
            self.push_child(pending, BodyNode::WhenGuard(guard))?;
        }
        match arm.condition() {
            DefaultWhenConditionV1::Case(pattern) => self.push_child(
                pending,
                BodyNode::Pattern {
                    pattern,
                    origin: arm.definition_origin(),
                },
            ),
            DefaultWhenConditionV1::Predicate(predicate) => {
                self.push_child(pending, BodyNode::WhenGuard(predicate))
            }
            DefaultWhenConditionV1::Always => Ok(()),
        }
    }

    pub(in super::super) fn process_when_guard(
        &mut self,
        guard: &'body DefaultWhenGuardV1,
        pending: &mut Vec<ScheduledWork<'body>>,
    ) -> Result<(), V::Error> {
        self.push_child(pending, BodyNode::Expression(guard.condition()))?;
        self.push_statements(pending, guard.setup())
    }

    pub(in super::super) fn process_when_fallback(
        &mut self,
        fallback: DefaultWhenFallbackViewV1<'body>,
        origin: &'body ExportDefinitionSourceV1,
        pending: &mut Vec<ScheduledWork<'body>>,
    ) -> Result<(), V::Error> {
        match fallback {
            DefaultWhenFallbackViewV1::Fallthrough => Ok(()),
            DefaultWhenFallbackViewV1::Else(statements) => {
                self.push_statements(pending, statements)
            }
            DefaultWhenFallbackViewV1::IrrefutableArm { subject_type }
            | DefaultWhenFallbackViewV1::PatternMatrix { subject_type } => self.push_type(
                pending,
                subject_type,
                origin,
                DefaultBodyProviderTypeSiteV1::WhenFallbackSubject,
            ),
            DefaultWhenFallbackViewV1::EnumPatternMatrix {
                subject_type,
                owner_type,
            } => {
                self.push_type(
                    pending,
                    owner_type,
                    origin,
                    DefaultBodyProviderTypeSiteV1::WhenFallbackEnumOwner,
                )?;
                self.push_type(
                    pending,
                    subject_type,
                    origin,
                    DefaultBodyProviderTypeSiteV1::WhenFallbackSubject,
                )
            }
        }
    }

    pub(in super::super) fn process_try(
        &mut self,
        value: &'body DefaultTryV1,
        pending: &mut Vec<ScheduledWork<'body>>,
    ) -> Result<(), V::Error> {
        if let OptionalDefaultStatementListViewV1::Present(statements) = value.finally_body().view()
        {
            self.push_statements(pending, statements)?;
        }
        for catch in value.catches().iter().rev() {
            self.push_child(pending, BodyNode::Catch(catch))?;
        }
        self.push_statements(pending, value.body())
    }

    pub(in super::super) fn process_catch(
        &mut self,
        catch: &'body DefaultCatchV1,
        pending: &mut Vec<ScheduledWork<'body>>,
    ) -> Result<(), V::Error> {
        self.push_statements(pending, catch.body())?;
        self.push_type(
            pending,
            catch.value_type(),
            catch.definition_origin(),
            DefaultBodyProviderTypeSiteV1::CatchValue,
        )
    }
}
