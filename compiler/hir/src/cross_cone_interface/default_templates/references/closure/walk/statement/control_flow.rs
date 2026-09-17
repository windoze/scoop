use super::super::super::ExportDefaultReferenceClosureValidationError;
use super::super::{BodyNode, Validator, WorkItem};
use crate::{
    DefaultBodyProviderTypeSiteV1, DefaultCatchV1, DefaultTryV1, DefaultWhenArmV1,
    DefaultWhenFallbackV1, DefaultWhenFallbackViewV1, DefaultWhenGuardV1, DefaultWhenV1,
    ExportDefinitionSourceV1, OptionalDefaultStatementListViewV1,
};

impl Validator<'_> {
    pub(in super::super) fn process_when<'body>(
        &mut self,
        value: &'body DefaultWhenV1,
        origin: &'body ExportDefinitionSourceV1,
        depth: u64,
        pending: &mut Vec<WorkItem<'body>>,
    ) -> Result<(), ExportDefaultReferenceClosureValidationError> {
        self.push_child(
            pending,
            depth,
            BodyNode::WhenFallback {
                fallback: value.fallback(),
                origin,
            },
        )?;
        for arm in value.arms().iter().rev() {
            self.push_child(pending, depth, BodyNode::WhenArm(arm))?;
        }
        self.push_child(pending, depth, BodyNode::Expression(value.subject()))
    }

    pub(in super::super) fn process_when_arm<'body>(
        &mut self,
        arm: &'body DefaultWhenArmV1,
        depth: u64,
        pending: &mut Vec<WorkItem<'body>>,
    ) -> Result<(), ExportDefaultReferenceClosureValidationError> {
        self.push_statements(pending, depth, arm.body())?;
        if let Some(guard) = arm.guard().as_ref() {
            self.push_child(pending, depth, BodyNode::WhenGuard(guard))?;
        }
        self.push_child(
            pending,
            depth,
            BodyNode::Pattern {
                pattern: arm.pattern(),
                origin: arm.definition_origin(),
            },
        )
    }

    pub(in super::super) fn process_when_guard<'body>(
        &mut self,
        guard: &'body DefaultWhenGuardV1,
        depth: u64,
        pending: &mut Vec<WorkItem<'body>>,
    ) -> Result<(), ExportDefaultReferenceClosureValidationError> {
        self.push_child(pending, depth, BodyNode::Expression(guard.condition()))?;
        self.push_statements(pending, depth, guard.setup())
    }

    pub(in super::super) fn process_when_fallback<'body>(
        &mut self,
        fallback: &'body DefaultWhenFallbackV1,
        origin: &'body ExportDefinitionSourceV1,
        depth: u64,
        pending: &mut Vec<WorkItem<'body>>,
    ) -> Result<(), ExportDefaultReferenceClosureValidationError> {
        match fallback.view() {
            DefaultWhenFallbackViewV1::Else(statements) => {
                self.push_statements(pending, depth, statements)
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

    pub(in super::super) fn process_try<'body>(
        &mut self,
        value: &'body DefaultTryV1,
        depth: u64,
        pending: &mut Vec<WorkItem<'body>>,
    ) -> Result<(), ExportDefaultReferenceClosureValidationError> {
        if let OptionalDefaultStatementListViewV1::Present(statements) = value.finally_body().view()
        {
            self.push_statements(pending, depth, statements)?;
        }
        for catch in value.catches().iter().rev() {
            self.push_child(pending, depth, BodyNode::Catch(catch))?;
        }
        self.push_statements(pending, depth, value.body())
    }

    pub(in super::super) fn process_catch<'body>(
        &mut self,
        catch: &'body DefaultCatchV1,
        depth: u64,
        pending: &mut Vec<WorkItem<'body>>,
    ) -> Result<(), ExportDefaultReferenceClosureValidationError> {
        self.push_statements(pending, depth, catch.body())?;
        self.push_type(
            pending,
            catch.value_type(),
            catch.definition_origin(),
            DefaultBodyProviderTypeSiteV1::CatchValue,
        )
    }
}
