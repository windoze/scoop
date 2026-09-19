//! Initialization unit semantic validation.

use super::*;

pub(super) fn validate_initialization_units(
    decoded: Vec<DecodedStrongInitializationUnitRegistrationPlanV1>,
    target: LirTargetProfile,
    foundation: &OdrFreeLirFoundation,
    identities: &StrongRegistrationIdentitySurfaceV1,
    static_storages: StrongStaticStorageSemanticPlanSetV1,
) -> Result<StrongInitializationUnitSemanticPlanSetV1, StrongRegistrationProductionValidationError>
{
    require_length(
        RegistrationProductionTableV1::InitializationUnit,
        decoded.len(),
        identities.initialization_units().len(),
    )?;
    let mut units = Vec::with_capacity(decoded.len());
    for (index, (decoded, identity)) in decoded
        .into_iter()
        .zip(identities.initialization_units())
        .enumerate()
    {
        let unit = verify_expected(
            decoded.unit,
            identity.semantic_id(),
            RegistrationProductionTableV1::InitializationUnit,
            index,
            "unit",
        )?;
        if decoded.diagnostic_path.is_empty() {
            return Err(semantic_error(
                RegistrationProductionTableV1::InitializationUnit,
                index,
                "diagnostic_path",
            ));
        }
        let storage = resolve_known(
            decoded.storage_id,
            identities
                .static_storages()
                .iter()
                .map(|identity| identity.semantic_id()),
            RegistrationProductionTableV1::InitializationUnit,
            index,
            "storage",
        )?;
        let failure_root = resolve_known(
            decoded.failure_root_id,
            identities
                .static_storages()
                .iter()
                .map(|identity| identity.semantic_id()),
            RegistrationProductionTableV1::InitializationUnit,
            index,
            "failure_root",
        )?;
        if storage == failure_root {
            return Err(semantic_error(
                RegistrationProductionTableV1::InitializationUnit,
                index,
                "aliased_storage",
            ));
        }
        validate_initialization_storage(
            static_storages
                .storages()
                .iter()
                .find(|candidate| candidate.storage() == storage)
                .ok_or_else(|| {
                    semantic_error(
                        RegistrationProductionTableV1::InitializationUnit,
                        index,
                        "storage",
                    )
                })?,
            false,
            target,
            index,
        )?;
        validate_initialization_storage(
            static_storages
                .storages()
                .iter()
                .find(|candidate| candidate.storage() == failure_root)
                .ok_or_else(|| {
                    semantic_error(
                        RegistrationProductionTableV1::InitializationUnit,
                        index,
                        "failure_root",
                    )
                })?,
            true,
            target,
            index,
        )?;
        let failure_record = foundation
            .static_storages()
            .iter()
            .find(|record| record.id() == failure_root)
            .ok_or_else(|| {
                semantic_error(
                    RegistrationProductionTableV1::InitializationUnit,
                    index,
                    "failure_root",
                )
            })?;
        if failure_record.key() != &StaticStorageKey::initialization_failure_root(unit) {
            return Err(semantic_error(
                RegistrationProductionTableV1::InitializationUnit,
                index,
                "failure_root_identity",
            ));
        }
        let expected_initializer = generated_unit_body(
            unit,
            scoop_identity::InitializationCallableRole::Initializer,
        )
        .map_err(|_| {
            semantic_error(
                RegistrationProductionTableV1::InitializationUnit,
                index,
                "initializer",
            )
        })?;
        let initializer = verify_expected(
            decoded.initializer_id,
            expected_initializer,
            RegistrationProductionTableV1::InitializationUnit,
            index,
            "initializer",
        )?;
        let expected_ensure =
            generated_unit_body(unit, scoop_identity::InitializationCallableRole::Ensure).map_err(
                |_| {
                    semantic_error(
                        RegistrationProductionTableV1::InitializationUnit,
                        index,
                        "ensure",
                    )
                },
            )?;
        let ensure = verify_expected(
            decoded.ensure_id,
            expected_ensure,
            RegistrationProductionTableV1::InitializationUnit,
            index,
            "ensure",
        )?;
        let schedule = match decoded.semantic_schedule {
            DecodedStrongInitializationSchedulePlanV1::EagerStartup(decoded_gateway) => {
                let gateway = startup_gateway_body(unit).map_err(|_| {
                    semantic_error(
                        RegistrationProductionTableV1::InitializationUnit,
                        index,
                        "gateway",
                    )
                })?;
                verify_expected(
                    decoded_gateway,
                    gateway,
                    RegistrationProductionTableV1::InitializationUnit,
                    index,
                    "gateway",
                )?;
                StrongInitializationSchedulePlanV1::EagerStartup { gateway }
            }
            DecodedStrongInitializationSchedulePlanV1::LazyAccess => {
                StrongInitializationSchedulePlanV1::LazyAccess
            }
        };
        let mut dependencies = Vec::with_capacity(decoded.dependencies.len());
        for dependency in decoded.dependencies {
            dependencies.push(resolve_known(
                dependency,
                identities
                    .initialization_units()
                    .iter()
                    .map(|identity| identity.semantic_id()),
                RegistrationProductionTableV1::InitializationUnit,
                index,
                "dependency",
            )?);
        }
        if dependencies.contains(&unit) || dependencies.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(semantic_error(
                RegistrationProductionTableV1::InitializationUnit,
                index,
                "dependencies",
            ));
        }
        units.push(StrongInitializationUnitSemanticPlanV1::from_artifact(
            unit,
            decoded.diagnostic_path,
            schedule,
            storage,
            failure_root,
            initializer,
            ensure,
            dependencies,
        ));
    }
    Ok(StrongInitializationUnitSemanticPlanSetV1::from_artifact(
        static_storages,
        units,
    ))
}

fn validate_initialization_storage(
    storage: &StrongStaticStorageSemanticPlanV1,
    failure_root: bool,
    target: LirTargetProfile,
    index: usize,
) -> Result<(), StrongRegistrationProductionValidationError> {
    if storage.initial_state() != &StrongStaticStorageInitialStatePlanV1::ZeroedForRuntimeUnit {
        return Err(semantic_error(
            RegistrationProductionTableV1::InitializationUnit,
            index,
            "storage_initial_state",
        ));
    }
    if failure_root {
        let pointer = target.pointer_layout(PointerKind::Managed);
        if storage.byte_size() != pointer.size_bytes()
            || storage.allocation_extent() != pointer.size_bytes()
            || storage.required_alignment() != pointer.alignment_bytes()
            || storage.scan_program() != &RefScan::References(vec![0])
        {
            return Err(semantic_error(
                RegistrationProductionTableV1::InitializationUnit,
                index,
                "failure_root_shape",
            ));
        }
    }
    Ok(())
}
