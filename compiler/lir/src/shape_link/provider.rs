use scoop_identity::ConeIdentity;

use super::{ExternalShapeLinkImportV1, ShapeLinkContractV1, ShapeLinkError};
use crate::*;

pub(super) mod contracts;
mod support;
mod types;

pub struct ShapeLinkProviderPartsV1<'a> {
    pub foundation: &'a OdrFreeLirFoundation,
    pub production: &'a StrongProductionSectionV2,
    pub ordinary: &'a CrossConeLirBridgeSectionV1,
    pub layouts: &'a CanonicalExactLayoutExportsV1,
    pub callables: &'a CanonicalExactCallableAbiExportsV1,
    pub descriptors: &'a CanonicalExactDescriptorExportsV1,
    pub dispatch: &'a CanonicalExactDispatchExportsV1,
}

/// Borrowed, same-provider inputs. Export and actual-use closure are checked
/// by the containing section; this view validates the physical contract.
pub struct ShapeLinkProviderV1<'a> {
    parts: ShapeLinkProviderPartsV1<'a>,
}

impl<'a> ShapeLinkProviderV1<'a> {
    pub fn try_new(parts: ShapeLinkProviderPartsV1<'a>) -> Result<Self, ShapeLinkError> {
        let provider = parts.foundation.producer();
        if [
            parts.ordinary.artifact(),
            parts.layouts.provider(),
            parts.callables.provider(),
            parts.descriptors.provider(),
            parts.dispatch.provider(),
            parts.production.type_registrations().producer(),
            parts.production.callable_registrations().producer(),
            parts.production.static_storage_registrations().producer(),
            parts.production.initialization_registrations().producer(),
        ]
        .into_iter()
        .any(|actual| actual != provider)
        {
            return Err(ShapeLinkError::Provider);
        }
        let target = parts.layouts.target();
        if parts.callables.target() != target
            || parts.descriptors.target() != target
            || parts.dispatch.target() != target
            || parts.production.type_registrations().target() != &target.wire_id()
        {
            return Err(ShapeLinkError::Target);
        }
        Ok(Self { parts })
    }

    pub fn provider(&self) -> ConeIdentity {
        self.parts.foundation.producer()
    }

    pub fn target_profile(&self) -> LirTargetProfile {
        self.parts.layouts.target()
    }

    pub(crate) fn semantic_target(
        &self,
        subject: ExternalStrongShapeSubjectV1,
    ) -> Result<Option<LayoutAbiSemanticTargetV1>, ShapeLinkError> {
        super::import::semantic_target(subject, self.parts.layouts)
    }

    /// The complete local Strong definition surface used by import replay.
    /// Closure validators use this exact surface for the consumer-side
    /// foreign-definition exclusion; it is not reconstructed from symbols.
    pub fn canonical_definitions(&self) -> &'a StrongObjectSymbolSurfaceV1 {
        self.parts.production.canonical_definitions()
    }

    pub(super) fn import(
        &self,
        subject: ExternalStrongShapeSubjectV1,
        consumer: ConeIdentity,
    ) -> Result<ExternalShapeLinkImportV1, ShapeLinkError> {
        if consumer == self.provider() {
            return Err(ShapeLinkError::LocalImport);
        }
        let physical = StrongShapeDefinitionRefV1::from_foundation(subject, self.parts.foundation)?;

        let plan = self
            .parts
            .production
            .canonical_definitions()
            .plan(physical.definition())
            .ok_or(ShapeLinkError::DefinitionRelation(subject))?;
        if plan.primary_atom() != physical.primary() || plan.primary_symbol() != physical.symbol() {
            return Err(ShapeLinkError::DefinitionRelation(subject));
        }

        let contract = self.contract(subject, physical)?;
        if !contract.matches_subject(subject) {
            return Err(ShapeLinkError::Contract);
        }
        let import = ExternalShapeLinkImportV1 {
            provider: self.provider(),
            subject,
            expected_symbol: physical.symbol(),
            required_definition: physical.definition(),
            contract,
        };
        Ok(import)
    }
}
