//! Initialization unit semantic validation.

use super::*;
use scoop_identity::PersistentInitializationUnitId;
use scoop_wire::WirePath;

mod v2;
pub use v2::validate_initialization_registration_constituents_v2;

pub(super) fn validate_initialization_units(
    decoded: Vec<DecodedStrongInitializationUnitRegistrationPlanV1>,
    target: LirTargetProfile,
    foundation: &ConeLirFoundation,
    identities: &RegistrationIdentitySurfaceV1,
    static_storages: StrongStaticStorageSemanticPlanSetV1,
) -> Result<StrongInitializationUnitSemanticPlanSetV1, StrongRegistrationProductionValidationError>
{
    replay_units(
        decoded,
        target,
        foundation,
        identities,
        static_storages,
        |_, ids, index| {
            let mut resolved = Vec::new();
            scoop_wire::allocation::try_reserve(&mut resolved, ids.len(), &WirePath::root())?;
            for id in ids {
                resolved.push(resolve_known(
                    id,
                    identities
                        .initialization_units()
                        .iter()
                        .map(|identity| identity.semantic_id()),
                    RegistrationProductionTableV1::InitializationUnit,
                    index,
                    "dependency",
                )?);
            }
            Ok(resolved)
        },
    )
}

fn replay_units<D: crate::StrongInitializationDependencyReference>(
    decoded: Vec<DecodedStrongInitializationUnitRegistrationPlanV1>,
    target: LirTargetProfile,
    foundation: &ConeLirFoundation,
    identities: &RegistrationIdentitySurfaceV1,
    static_storages: StrongStaticStorageSemanticPlanSetV1,
    mut resolve: impl FnMut(
        PersistentInitializationUnitId,
        Vec<DecodedPersistentId<PersistentInitializationUnitId>>,
        usize,
    ) -> Result<Vec<D>, StrongRegistrationProductionValidationError>,
) -> Result<
    crate::StrongInitializationUnitSemanticPlanSet<D>,
    StrongRegistrationProductionValidationError,
> {
    require_length(
        RegistrationProductionTableV1::InitializationUnit,
        decoded.len(),
        identities.initialization_units().len(),
    )?;
    let path = WirePath::root();

    let mut units = Vec::new();
    scoop_wire::allocation::try_reserve(&mut units, decoded.len(), &path)?;
    for (index, (decoded, identity)) in decoded
        .into_iter()
        .zip(identities.initialization_units())
        .enumerate()
    {
        let decoded = decoded.semantic;
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
        let initializer = resolve_unit_body(
            decoded.initializer_id,
            unit,
            scoop_identity::InitializationCallableRole::Initializer,
            identity.owner(),
            foundation,
            index,
            "initializer",
        )?;
        let ensure = resolve_unit_body(
            decoded.ensure_id,
            unit,
            scoop_identity::InitializationCallableRole::Ensure,
            identity.owner(),
            foundation,
            index,
            "ensure",
        )?;
        if initializer == ensure {
            return Err(semantic_error(
                RegistrationProductionTableV1::InitializationUnit,
                index,
                "aliased_callable",
            ));
        }
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
        let dependencies = resolve(unit, decoded.dependencies, index)?;
        if dependencies.iter().any(|dependency| {
            dependency.unit_id() == unit
                || !dependency.has_valid_provider_role(foundation.producer())
        }) || dependencies
            .windows(2)
            .any(|pair| pair[0].unit_id() >= pair[1].unit_id())
        {
            return Err(semantic_error(
                RegistrationProductionTableV1::InitializationUnit,
                index,
                "dependencies",
            ));
        }
        units.push(crate::StrongInitializationUnitSemanticPlan::from_artifact(
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
    Ok(crate::StrongInitializationUnitSemanticPlanSet::from_artifact(static_storages, units))
}

fn resolve_unit_body(
    decoded: DecodedPersistentId<scoop_identity::PersistentCallableBodyId>,
    unit: PersistentInitializationUnitId,
    role: scoop_identity::InitializationCallableRole,
    owner: crate::RegistrationDefinitionOwner,
    foundation: &ConeLirFoundation,
    index: usize,
    field: &'static str,
) -> Result<scoop_identity::PersistentCallableBodyId, StrongRegistrationProductionValidationError> {
    let table = RegistrationProductionTableV1::InitializationUnit;
    match owner {
        crate::RegistrationDefinitionOwner::Strong => {
            let expected = generated_unit_body(unit, role, None)
                .map_err(|_| semantic_error(table, index, field))?;
            verify_expected(decoded, expected, table, index, field)
        }
        crate::RegistrationDefinitionOwner::Odr { group, .. } => {
            // The shared identity graph checks the declaration role and unit
            // application; this projection joins the actual emitted member.
            resolve_known(decoded, foundation.callable_bodies().iter().filter_map(|body| {
                matches!(body.key().kind(), scoop_identity::CallableBodyKeyKind::Odr(member)
                    if member.group() == group && member.role() == scoop_identity::OdrMemberRole::CallableBody)
                    .then_some(body.id())
            }), table, index, field)
        }
    }
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
