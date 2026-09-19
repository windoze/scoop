//! Ten-field V2 replay keeps publication gated by source and selected joins.

use super::*;
use scoop_wire::{BudgetMeter, WirePath, encode_canonical_temporary_with_meter};

mod budget;
mod layout_join;
mod view;
pub use layout_join::{StrongProductionLayoutJoinError, ValidatedStrongProductionSectionV2};
pub use view::ReplayedStrongProductionSectionV2;

impl DecodedStrongProductionSectionV2 {
    /// Replays the whole section using independently reconstructed digest and
    /// core-bridge plans. Their identities and foundation relations remain
    /// checked by the existing strong plan constructors. The result must be
    /// joined with the complete layout/ABI source and selected closure before
    /// it becomes a production section; this API publishes no such authority.
    #[allow(clippy::too_many_arguments)]
    pub fn replay(
        self,
        coordinate: ConeCoordinate,
        target: crate::LirTargetProfile,
        foundation: &OdrFreeLirFoundation,
        expected_external_bridges: StrongExternalLirBridgeSurfaceV1,
        expected_digests: StrongDigestFinalizationPlanV1,
        entry_source: EntryProductionSourceV1,
        core_shape_sources: &[SourceDeclarationKey],
        expected_core_bridge: CoreLirBridgeBranchV1,
        type_definitions: &crate::StrongTypeReferenceDefinitionsV2,
        initialization_definitions: &crate::StrongInitializationDefinitionCatalogV2,
        meter: &mut BudgetMeter,
    ) -> Result<ReplayedStrongProductionSectionV2, StrongProductionSectionValidationError> {
        let path = WirePath::root();
        budget::charge_replay(
            foundation,
            &expected_digests,
            core_shape_sources,
            &expected_core_bridge,
            meter,
        )?;
        let actual = encode_canonical_temporary_with_meter(&self, meter, &path)?;
        let registrations = self
            .registration_production
            .replay(
                target,
                foundation,
                &expected_digests,
                &expected_external_bridges,
                type_definitions,
                initialization_definitions,
                meter,
            )
            .map_err(StrongProductionSectionValidationError::Registrations)?;
        let section = StrongProductionSectionV2::from_parts(
            coordinate,
            foundation,
            expected_external_bridges,
            expected_digests,
            registrations.surface,
            entry_source,
            core_shape_sources,
            expected_core_bridge,
        )
        .map_err(StrongProductionSectionValidationError::Expected)?;
        if actual != encode_canonical_temporary_with_meter(&section, meter, &path)? {
            return Err(StrongProductionSectionValidationError::SectionMismatch);
        }
        Ok(ReplayedStrongProductionSectionV2 { section })
    }
}

impl From<WireError> for StrongProductionSectionValidationError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
