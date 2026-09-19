//! Full registration replay; dependency selection is committed by the section.

use super::*;
use crate::{
    StrongInitializationDefinitionCatalogV2, StrongInitializationDependencyKindV2,
    StrongInitializationUnitDefinitionRefV2,
};
use scoop_wire::encode_canonical_temporary_with_meter;

/// Reconstructs all 28 fields and retains the exact physical definition for
/// each dependency. The artifact reader must additionally join foreign edges
/// to its explicit selected initialization-use closure before committing it.
#[allow(clippy::too_many_arguments)]
pub fn validate_initialization_registration_constituents_v2(
    decoded: Vec<DecodedStrongInitializationUnitRegistrationPlanV1>,
    target: LirTargetProfile,
    foundation: &OdrFreeLirFoundation,
    identities: &StrongRegistrationIdentitySurfaceV1,
    static_storages: StrongStaticStorageSemanticPlanSetV1,
    definitions: &StrongInitializationDefinitionCatalogV2,
    digests: &StrongDigestFinalizationPlanV1,
    meter: &mut BudgetMeter,
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
    meter.try_reserve_collection_slots(&mut actual, decoded.len(), &path)?;
    for record in &decoded {
        meter.charge_owned_bytes(record.diagnostic_path.len() as u64, &path)?;
        meter.charge_collection_slots(record.dependencies.len() as u64, &path)?;
        actual.push(encode_canonical_temporary_with_meter(record, meter, &path)?);
    }
    charge_definition_replay(decoded.len(), foundation, digests, meter)?;
    let semantics = replay_units(
        decoded,
        target,
        foundation,
        identities,
        static_storages,
        |unit, ids, index, meter| {
            let resolved = definitions.resolve(unit, &ids, meter)?;
            for reference in resolved.references() {
                if let StrongInitializationDependencyKindV2::LocalUnit(definition) =
                    reference.kind()
                {
                    let current = StrongInitializationUnitDefinitionRefV2::from_foundation(
                        definition.unit(),
                        foundation,
                        identities,
                        digests,
                        meter,
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
        meter,
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
        if actual != &encode_canonical_temporary_with_meter(expected, meter, &path)? {
            return Err(StrongRegistrationProductionValidationError::EntryMismatch {
                table: RegistrationProductionTableV1::InitializationUnit,
                index,
            });
        }
    }
    Ok(semantics)
}

fn charge_definition_replay(
    count: usize,
    foundation: &OdrFreeLirFoundation,
    digests: &StrongDigestFinalizationPlanV1,
    meter: &mut BudgetMeter,
) -> Result<(), scoop_wire::WireError> {
    let path = WirePath::root();
    meter.charge_work(digests.nodes().len() as u64, &path)?;
    let mut entries = (foundation.definition_plans().len() as u64)
        .saturating_add(foundation.definition_atoms().len() as u64)
        .saturating_add(foundation.symbol_requests().len() as u64)
        .saturating_add(foundation.callable_bodies().len() as u64)
        .saturating_add(digests.nodes().len() as u64);
    for node in digests.nodes() {
        entries = entries
            .saturating_add(node.direct_inputs().len() as u64)
            .saturating_add(node.patch_intents().len() as u64);
    }
    meter.charge_work(
        (count as u64)
            .saturating_mul(entries.saturating_add(1))
            .saturating_mul(64),
        &path,
    )?;
    meter.charge_collection_slots(
        (count as u64).saturating_mul(entries.saturating_add(16)),
        &path,
    )
}
