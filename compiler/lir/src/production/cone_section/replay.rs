//! Decode Strong V2 records against their actual foundation and registrations.

use super::*;
use scoop_wire::{WirePath, encode_canonical_temporary};

mod layout_join;
pub use layout_join::StrongProductionLayoutJoinError;

impl DecodedConeProductionSectionV2 {
    /// Checks the canonical section against its foundation and registration
    /// records. Layout and external-reference joins use the returned data.
    #[allow(clippy::too_many_arguments)]
    pub fn replay(
        self,
        coordinate: ConeCoordinate,
        direct_dependencies: &[ConeIdentity],
        target: crate::LirTargetProfile,
        foundation: &ConeLirFoundation,
        entry_source: EntryProductionSourceV1,
        shape_sources: &[SourceDeclarationKey],
        type_definitions: &crate::StrongTypeReferenceDefinitionsV2,
        initialization_definitions: &crate::StrongInitializationDefinitionCatalogV2,
    ) -> Result<ConeProductionSectionV2, ConeProductionSectionValidationError> {
        let path = WirePath::root();
        let actual = encode_canonical_temporary(&self, &path)?;
        let digests = self
            .digest_finalization_plan
            .resolve_foundation(foundation, type_definitions)
            .map_err(|source| {
                ConeProductionSectionValidationError::DigestReplay(Box::new(source))
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
            .map_err(ConeProductionSectionValidationError::Registrations)?;
        let expected_digests =
            crate::replay_digest_finalization_plan_v2(foundation, &registrations, &entry_source)
                .map_err(|source| {
                    ConeProductionSectionValidationError::DigestProjection(Box::new(source))
                })?;
        let original = encode_canonical_temporary(&digests, &path)?;
        let canonical = encode_canonical_temporary(&expected_digests, &path)?;

        if original != canonical {
            return Err(ConeProductionSectionValidationError::DigestMismatch);
        }
        let canonical_callables = self
            .canonical_callables
            .validate(foundation)
            .map_err(ConeProductionSectionValidationError::CanonicalCallables)?;
        let canonical_shapes = self
            .canonical_shapes
            .validate(foundation)
            .map_err(ConeProductionSectionValidationError::CanonicalShapes)?;
        let section = ConeProductionSectionV2::from_parts(
            coordinate,
            direct_dependencies,
            foundation,
            expected_digests,
            registrations,
            entry_source,
            shape_sources,
            canonical_callables,
            canonical_shapes,
        )
        .map_err(ConeProductionSectionValidationError::Expected)?;
        let canonical = encode_canonical_temporary(&section, &path)?;

        if actual != canonical {
            return Err(ConeProductionSectionValidationError::SectionMismatch);
        }
        Ok(section)
    }
}

impl From<WireError> for ConeProductionSectionValidationError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
