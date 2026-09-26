//! Decode Strong V2 records against their actual foundation and registrations.

use super::*;
use scoop_wire::{WirePath, encode_canonical_temporary};

mod layout_join;
pub use layout_join::StrongProductionLayoutJoinError;

impl DecodedStrongProductionSectionV2 {
    /// Checks the canonical section against its foundation and registration
    /// records. Layout and external-reference joins use the returned data.
    #[allow(clippy::too_many_arguments)]
    pub fn replay(
        self,
        coordinate: ConeCoordinate,
        direct_dependencies: &[ConeIdentity],
        target: crate::LirTargetProfile,
        foundation: &OdrFreeLirFoundation,
        entry_source: EntryProductionSourceV1,
        shape_sources: &[SourceDeclarationKey],
        type_definitions: &crate::StrongTypeReferenceDefinitionsV2,
        initialization_definitions: &crate::StrongInitializationDefinitionCatalogV2,
    ) -> Result<StrongProductionSectionV2, StrongProductionSectionValidationError> {
        let path = WirePath::root();
        let actual = encode_canonical_temporary(&self, &path)?;
        let digests = self
            .digest_finalization_plan
            .resolve_foundation(foundation)
            .map_err(|source| {
                StrongProductionSectionValidationError::DigestReplay(Box::new(source))
            })?;

        let registrations = self
            .registration_production
            .replay(
                target,
                foundation,
                &digests,
                type_definitions,
                initialization_definitions,
            )
            .map_err(StrongProductionSectionValidationError::Registrations)?;
        let expected_digests = crate::replay_strong_digest_finalization_plan_v2(
            foundation,
            &registrations,
            &entry_source,
        )
        .map_err(|source| {
            StrongProductionSectionValidationError::DigestProjection(Box::new(source))
        })?;
        let original = encode_canonical_temporary(&digests, &path)?;
        let canonical = encode_canonical_temporary(&expected_digests, &path)?;

        if original != canonical {
            return Err(StrongProductionSectionValidationError::DigestMismatch);
        }
        let section = StrongProductionSectionV2::from_parts(
            coordinate,
            direct_dependencies,
            foundation,
            expected_digests,
            registrations,
            entry_source,
            shape_sources,
        )
        .map_err(StrongProductionSectionValidationError::Expected)?;
        let canonical = encode_canonical_temporary(&section, &path)?;

        if actual != canonical {
            return Err(StrongProductionSectionValidationError::SectionMismatch);
        }
        Ok(section)
    }
}

impl From<WireError> for StrongProductionSectionValidationError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
