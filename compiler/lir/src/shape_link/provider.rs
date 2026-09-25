use scoop_identity::ConeIdentity;

use super::{
    ExternalShapeLinkImportV1, ShapeLinkContractV1, ShapeLinkError, ShapeLinkSupportLookupV1,
};
use crate::*;

mod production;
use production::PhysicalProduction;
pub use production::ShapeLinkProductionV1;
pub(super) mod contracts;
mod legacy;
mod support;
mod types;

pub struct ShapeLinkProviderPartsV1<'a, P = ShapeLinkProductionV1<'a>> {
    pub foundation: &'a OdrFreeLirFoundation,
    pub production: P,
    pub ordinary: &'a CrossConeLirBridgeSectionV1,
    pub layouts: &'a CanonicalExactLayoutExportsV1,
    pub callables: &'a CanonicalExactCallableAbiExportsV1,
    pub descriptors: &'a CanonicalExactDescriptorExportsV1,
    pub dispatch: &'a CanonicalExactDispatchExportsV1,
}

/// Borrowed, same-provider inputs. Export and actual-use closure are checked
/// by the containing section; this view validates the physical contract.
pub struct ShapeLinkProviderV1<'a> {
    parts: ShapeLinkProviderPartsV1<'a, PhysicalProduction<'a>>,
}

impl<'a> ShapeLinkProviderV1<'a> {
    pub fn try_new(parts: ShapeLinkProviderPartsV1<'a>) -> Result<Self, ShapeLinkError> {
        let production = PhysicalProduction::Complete(parts.production);
        Self::validate(parts.with_production(production))
    }

    /// A contract-only view. Its replayed production cannot be passed to the
    /// complete Link terminal constructor, which accepts the separate enum.
    pub fn from_replayed(
        foundation: &'a OdrFreeLirFoundation,
        ordinary: &'a CrossConeLirBridgeSectionV1,
        view: ReplayedStrongLayoutExportsV2<'a>,
    ) -> Result<Self, ShapeLinkError> {
        Self::validate(ShapeLinkProviderPartsV1 {
            foundation,
            ordinary,
            production: PhysicalProduction::Replayed(view),
            layouts: view.exports.layouts(),
            callables: view.exports.callables(),
            descriptors: view.exports.descriptors(),
            dispatch: view.exports.dispatch(),
        })
    }

    fn validate(
        parts: ShapeLinkProviderPartsV1<'a, PhysicalProduction<'a>>,
    ) -> Result<Self, ShapeLinkError> {
        let provider = parts.foundation.producer();
        if [
            parts.ordinary.artifact(),
            parts.layouts.provider(),
            parts.callables.provider(),
            parts.descriptors.provider(),
            parts.dispatch.provider(),
            parts.production.types().producer(),
            parts.production.callables().producer(),
            parts.production.storages().producer(),
            parts.production.units().producer(),
        ]
        .into_iter()
        .any(|actual| actual != provider)
            || parts
                .production
                .initialization_abi()
                .is_some_and(|abi| abi.link_contract(provider).is_err())
        {
            return Err(ShapeLinkError::Provider);
        }
        let target = parts.layouts.target();
        if parts.callables.target() != target
            || parts.descriptors.target() != target
            || parts.dispatch.target() != target
            || parts.production.types().target() != &target.wire_id()
        {
            return Err(ShapeLinkError::Target);
        }
        if let PhysicalProduction::Complete(ShapeLinkProductionV1::Reader(production)) =
            parts.production
            && (parts.layouts != production.layouts()
                || parts.callables != production.callables()
                || parts.descriptors != production.descriptors()
                || parts.dispatch != production.dispatch())
        {
            return Err(ShapeLinkError::Provider);
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
        self.parts.production.definitions()
    }

    pub(super) fn import(
        &self,
        subject: ExternalStrongShapeSubjectV1,
        consumer: ConeIdentity,
        consumer_definitions: &StrongObjectSymbolSurfaceV1,
        support: &dyn ShapeLinkSupportLookupV1<'a>,
    ) -> Result<ExternalShapeLinkImportV1<'a>, ShapeLinkError> {
        if consumer == self.provider() {
            return Err(ShapeLinkError::LocalImport);
        }
        self.reject_legacy(subject)?;
        let physical = StrongShapeDefinitionRefV1::from_foundation(subject, self.parts.foundation)?;

        let plan = self
            .parts
            .production
            .definitions()
            .plan(physical.definition())
            .ok_or(ShapeLinkError::DefinitionRelation(subject))?;
        if plan.primary_atom() != physical.primary() || plan.primary_symbol() != physical.symbol() {
            return Err(ShapeLinkError::DefinitionRelation(subject));
        }

        for definition in consumer_definitions.plans() {
            if definition.primary_symbol() == physical.symbol()
                || definition.atom_boundaries().iter().any(|boundary| {
                    boundary.start() == physical.symbol() || boundary.end() == physical.symbol()
                })
            {
                return Err(ShapeLinkError::ConsumerDefinition(physical.symbol()));
            }
        }
        let contract = self.contract(subject, physical, support)?;
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
        import.validate_semantic_against(
            self.parts.layouts,
            self.parts.callables,
            self.parts.descriptors,
            self.parts.dispatch,
        )?;
        Ok(import)
    }
}

impl<'a, P> ShapeLinkProviderPartsV1<'a, P> {
    fn with_production<Q>(self, production: Q) -> ShapeLinkProviderPartsV1<'a, Q> {
        ShapeLinkProviderPartsV1 {
            foundation: self.foundation,
            production,
            ordinary: self.ordinary,
            layouts: self.layouts,
            callables: self.callables,
            descriptors: self.descriptors,
            dispatch: self.dispatch,
        }
    }
}
