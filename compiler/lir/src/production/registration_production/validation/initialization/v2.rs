//! Full registration replay; dependency selection is committed by the section.

use super::*;
use crate::{
    StrongInitializationDefinitionCatalogV2, StrongInitializationDependencyKindV2,
    StrongInitializationUnitDefinitionRefV2,
};
use scoop_wire::encode_canonical_temporary;

/// Reconstructs all 28 fields and retains the exact physical definition for
/// each dependency. The artifact reader must additionally join foreign edges
/// to its explicit selected initialization-use closure before committing it.
#[allow(clippy::too_many_arguments)]
pub fn validate_initialization_registration_constituents_v2(
    decoded: Vec<DecodedStrongInitializationUnitRegistrationPlanV1>,
    target: LirTargetProfile,
    foundation: &ConeLirFoundation,
    identities: &StrongRegistrationIdentitySurfaceV1,
    static_storages: StrongStaticStorageSemanticPlanSetV1,
    definitions: &StrongInitializationDefinitionCatalogV2,
    digests: &StrongDigestFinalizationPlanV1,
) -> Result<
    crate::StrongInitializationUnitSemanticPlanSetV2,
    StrongRegistrationProductionValidationError,
> {
    if definitions.producer() != foundation.producer()
        || static_storages.producer() != foundation.producer()
    {
        return Err(semantic_error(
            RegistrationProductionTableV1::InitializationUnit,
            0,
            "producer",
        ));
    }
    require_length(
        RegistrationProductionTableV1::InitializationUnit,
        decoded.len(),
        identities.initialization_units().len(),
    )?;
    let path = WirePath::root();
    let mut actual = Vec::new();
    scoop_wire::allocation::try_reserve(&mut actual, decoded.len(), &path)?;
    for record in &decoded {
        actual.push(encode_canonical_temporary(record, &path)?);
    }

    let semantics = replay_units(
        decoded,
        target,
        foundation,
        identities,
        static_storages,
        |unit, ids, index| {
            let resolved = definitions.resolve(unit, &ids)?;
            for reference in resolved.references() {
                if let StrongInitializationDependencyKindV2::LocalUnit(definition) =
                    reference.kind()
                {
                    let current = StrongInitializationUnitDefinitionRefV2::from_foundation(
                        definition.unit(),
                        foundation,
                        identities,
                        digests,
                    )?;
                    if current != *definition {
                        return Err(semantic_error(
                            RegistrationProductionTableV1::InitializationUnit,
                            index,
                            "dependency_definition",
                        ));
                    }
                }
            }
            Ok(resolved.into_references())
        },
    )?;
    let expected = crate::StrongInitializationUnitRegistrationPlanSetV2::new(
        foundation, identities, &semantics, digests,
    )
    .map_err(|error| {
        StrongRegistrationProductionValidationError::Expected(Box::new(
            StrongRegistrationProductionBuildError::InitializationUnits(error),
        ))
    })?;
    for (index, (actual, expected)) in actual.iter().zip(expected.registrations()).enumerate() {
        if actual != &encode_canonical_temporary(expected, &path)? {
            return Err(StrongRegistrationProductionValidationError::EntryMismatch {
                table: RegistrationProductionTableV1::InitializationUnit,
                index,
            });
        }
    }
    Ok(semantics)
}
