use scoop_identity::ConeIdentity;
use scoop_wire::{BudgetMeter, WirePath};

use super::{
    ExternalShapeLinkImportV1, ShapeLinkContractV1, ShapeLinkError, ShapeLinkSupportAuthorityV1,
};
use crate::*;

mod production;
pub use production::ShapeLinkProductionV1;
pub(super) mod contracts;
mod legacy;
mod support;
mod types;

pub struct ShapeLinkProviderPartsV1<'a> {
    pub foundation: &'a OdrFreeLirFoundation,
    pub production: ShapeLinkProductionV1<'a>,
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
    pub fn try_new(
        parts: ShapeLinkProviderPartsV1<'a>,
        meter: &mut BudgetMeter,
    ) -> Result<Self, ShapeLinkError> {
        meter.charge_work(10, &WirePath::root())?;
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
            || parts.production.core().core().is_some() != (provider == ConeIdentity::CORE)
            || parts.production.core_shapes().core().is_some() != (provider == ConeIdentity::CORE)
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
        if let ShapeLinkProductionV1::Reader(production) = parts.production
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

    pub(super) fn import(
        &self,
        subject: ExternalStrongShapeSubjectV1,
        consumer: ConeIdentity,
        consumer_definitions: &StrongObjectSymbolSurfaceV1,
        support: &dyn ShapeLinkSupportAuthorityV1<'a>,
        meter: &mut BudgetMeter,
    ) -> Result<ExternalShapeLinkImportV1<'a>, ShapeLinkError> {
        if consumer == self.provider() {
            return Err(ShapeLinkError::LocalImport);
        }
        self.reject_legacy(subject, meter)?;
        let physical =
            StrongShapeDefinitionRefV1::from_foundation(subject, self.parts.foundation, meter)?;
        let path = WirePath::root();
        meter.charge_work(
            self.parts.production.definitions().plans().len() as u64,
            &path,
        )?;
        let plan = self
            .parts
            .production
            .definitions()
            .plan(physical.definition())
            .ok_or(ShapeLinkError::DefinitionRelation(subject))?;
        if plan.primary_atom() != physical.primary() || plan.primary_symbol() != physical.symbol() {
            return Err(ShapeLinkError::DefinitionRelation(subject));
        }
        meter.charge_work(consumer_definitions.plans().len() as u64, &path)?;
        for definition in consumer_definitions.plans() {
            meter.charge_work(definition.atom_boundaries().len() as u64, &path)?;
            if definition.primary_symbol() == physical.symbol()
                || definition.atom_boundaries().iter().any(|boundary| {
                    boundary.start() == physical.symbol() || boundary.end() == physical.symbol()
                })
            {
                return Err(ShapeLinkError::ConsumerDefinition(physical.symbol()));
            }
        }
        let contract = self.contract(subject, physical, support, meter)?;
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
            meter,
        )?;
        Ok(import)
    }
}
