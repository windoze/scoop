use crate::{
    DefaultCatchV1, DefaultTryV1, DefaultWhenArmV1, DefaultWhenFallbackV1,
    DefaultWhenFallbackViewV1, DefaultWhenGuardV1, DefaultWhenV1, ExportDefinitionSourceV1,
    OptionalDefaultStatementListViewV1,
};

use super::super::{BodyEnvelopeAuthority, BodyNode, Validator, WorkItem};
use crate::{
    DefaultBodyOriginSiteV1, DefaultBodyProviderEnvelopeSemanticValidationError,
    DefaultBodyProviderTypeSiteV1,
};

impl<A, E> Validator<'_, A, E>
where
    A: BodyEnvelopeAuthority<E>,
{
    pub(in super::super) fn process_when<'body>(
        &mut self,
        value: &'body DefaultWhenV1,
        definition_origin: &'body ExportDefinitionSourceV1,
        depth: u64,
        pending: &mut Vec<WorkItem<'body>>,
    ) -> Result<(), DefaultBodyProviderEnvelopeSemanticValidationError<E>> {
        self.push_child(
            pending,
            depth,
            BodyNode::WhenFallback {
                fallback: value.fallback(),
                definition_origin,
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
    ) -> Result<(), DefaultBodyProviderEnvelopeSemanticValidationError<E>> {
        self.push_statements(pending, depth, arm.body())?;
        if let Some(guard) = arm.guard().as_ref() {
            self.push_child(
                pending,
                depth,
                BodyNode::WhenGuard {
                    guard,
                    definition_origin: arm.definition_origin(),
                },
            )?;
        }
        self.push_child(
            pending,
            depth,
            BodyNode::Pattern {
                pattern: arm.pattern(),
                definition_origin: arm.definition_origin(),
            },
        )?;
        self.push_child(
            pending,
            depth,
            BodyNode::Origin {
                source: arm.definition_origin(),
                site: DefaultBodyOriginSiteV1::WhenArm,
            },
        )
    }

    pub(in super::super) fn process_when_guard<'body>(
        &mut self,
        guard: &'body DefaultWhenGuardV1,
        _definition_origin: &'body ExportDefinitionSourceV1,
        depth: u64,
        pending: &mut Vec<WorkItem<'body>>,
    ) -> Result<(), DefaultBodyProviderEnvelopeSemanticValidationError<E>> {
        self.push_child(pending, depth, BodyNode::Expression(guard.condition()))?;
        self.push_statements(pending, depth, guard.setup())
    }

    pub(in super::super) fn process_when_fallback<'body>(
        &mut self,
        fallback: &'body DefaultWhenFallbackV1,
        definition_origin: &'body ExportDefinitionSourceV1,
        depth: u64,
        pending: &mut Vec<WorkItem<'body>>,
    ) -> Result<(), DefaultBodyProviderEnvelopeSemanticValidationError<E>> {
        match fallback.view() {
            DefaultWhenFallbackViewV1::Else(statements) => {
                self.push_statements(pending, depth, statements)
            }
            DefaultWhenFallbackViewV1::IrrefutableArm { subject_type }
            | DefaultWhenFallbackViewV1::PatternMatrix { subject_type } => self.push_type(
                pending,
                subject_type,
                DefaultBodyProviderTypeSiteV1::WhenFallbackSubject,
                definition_origin,
            ),
            DefaultWhenFallbackViewV1::EnumPatternMatrix {
                subject_type,
                owner_type,
            } => {
                self.push_type(
                    pending,
                    owner_type,
                    DefaultBodyProviderTypeSiteV1::WhenFallbackEnumOwner,
                    definition_origin,
                )?;
                self.push_type(
                    pending,
                    subject_type,
                    DefaultBodyProviderTypeSiteV1::WhenFallbackSubject,
                    definition_origin,
                )
            }
        }
    }

    pub(in super::super) fn process_try<'body>(
        &mut self,
        value: &'body DefaultTryV1,
        _definition_origin: &'body ExportDefinitionSourceV1,
        depth: u64,
        pending: &mut Vec<WorkItem<'body>>,
    ) -> Result<(), DefaultBodyProviderEnvelopeSemanticValidationError<E>> {
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
    ) -> Result<(), DefaultBodyProviderEnvelopeSemanticValidationError<E>> {
        self.push_statements(pending, depth, catch.body())?;
        self.push_type(
            pending,
            catch.value_type(),
            DefaultBodyProviderTypeSiteV1::CatchValue,
            catch.definition_origin(),
        )?;
        self.push_child(
            pending,
            depth,
            BodyNode::Origin {
                source: catch.definition_origin(),
                site: DefaultBodyOriginSiteV1::Catch,
            },
        )
    }
}
