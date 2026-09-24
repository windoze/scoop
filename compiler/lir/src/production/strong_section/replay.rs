//! Ten-field V2 replay keeps publication gated by source and selected joins.

use super::*;
use scoop_wire::{BudgetMeter, WirePath, encode_canonical_temporary_with_meter};

mod budget;
mod layout_join;
mod view;
pub use layout_join::{StrongProductionLayoutJoinError, ValidatedStrongProductionSectionV2};
pub use view::ReplayedStrongProductionSectionV2;

impl<I: WireEncode>
    DecodedStrongProductionSection<crate::DecodedStrongRegistrationProductionSurfaceV2, I>
{
    /// Replays the whole section using the actual foundation and registration
    /// semantics. Candidate digests are resolved only for the registration
    /// relation checks, then replaced by the shared canonical projection. The result must be
    /// joined with the complete layout/ABI source and selected closure before
    /// it becomes a production section; this API publishes no such authority.
    #[allow(clippy::too_many_arguments)]
    pub fn replay(
        self,
        coordinate: ConeCoordinate,
        direct_dependencies: &[ConeIdentity],
        target: crate::LirTargetProfile,
        foundation: &OdrFreeLirFoundation,
        expected_external_bridges: StrongExternalLirBridgeSurfaceV1,
        entry_source: EntryProductionSourceV1,
        shape_sources: &[SourceDeclarationKey],
        expected_initialization_abi: Option<Box<CallableAbiRecordV1>>,
        type_definitions: &crate::StrongTypeReferenceDefinitionsV2,
        initialization_definitions: &crate::StrongInitializationDefinitionCatalogV2,
        meter: &mut BudgetMeter,
    ) -> Result<ReplayedStrongProductionSectionV2, StrongProductionSectionValidationError> {
        let path = WirePath::root();
        let actual = encode_canonical_temporary_with_meter(&self, meter, &path)?;
        let digests = self
            .digest_finalization_plan
            .resolve_foundation(foundation, meter)
            .map_err(|source| {
                StrongProductionSectionValidationError::DigestReplay(Box::new(source))
            })?;
        budget::charge_replay(
            foundation,
            direct_dependencies,
            &digests,
            shape_sources,
            expected_initialization_abi.as_deref(),
            meter,
        )?;
        let registrations = self
            .registration_production
            .replay(
                target,
                foundation,
                &digests,
                &expected_external_bridges,
                type_definitions,
                initialization_definitions,
                meter,
            )
            .map_err(StrongProductionSectionValidationError::Registrations)?;
        let expected_digests = crate::replay_strong_digest_finalization_plan_v2(
            foundation,
            &registrations.surface,
            &entry_source,
            meter,
        )
        .map_err(|source| {
            StrongProductionSectionValidationError::DigestProjection(Box::new(source))
        })?;
        let original = encode_canonical_temporary_with_meter(&digests, meter, &path)?;
        let canonical = encode_canonical_temporary_with_meter(&expected_digests, meter, &path)?;
        meter.charge_work(original.len().min(canonical.len()) as u64, &path)?;
        if original != canonical {
            return Err(StrongProductionSectionValidationError::DigestMismatch);
        }
        let section = StrongProductionSectionV2::from_parts(
            coordinate,
            direct_dependencies,
            foundation,
            expected_external_bridges,
            expected_digests,
            registrations.surface,
            entry_source,
            shape_sources,
            expected_initialization_abi,
        )
        .map_err(StrongProductionSectionValidationError::Expected)?;
        let canonical = encode_canonical_temporary_with_meter(&section, meter, &path)?;
        meter.charge_work(actual.len().min(canonical.len()) as u64, &path)?;
        if actual != canonical {
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
