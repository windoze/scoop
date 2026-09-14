//! Validation of untrusted complete registration-production carriers.

use std::fmt;

use scoop_identity::{
    DecodedPersistentId, LinkageClass, PersistentId, PersistentStaticStorageId,
    PersistentSymbolKey, PersistentSymbolRequest, RepresentationRole, ScanRole, StaticStorageKey,
};
use scoop_wire::{WireEncode, encode};

use super::wire::{
    DecodedImmortalObjectTypeRegistrationRefV1, DecodedRefScan,
    DecodedStaticImmortalRelocationPlanV1, DecodedStrongInitializationSchedulePlanV1,
    DecodedStrongStaticStorageInitialStatePlanV1,
};
use super::{
    DecodedStrongImmortalObjectRegistrationPlanV1,
    DecodedStrongInitializationUnitRegistrationPlanV1,
    DecodedStrongRegistrationProductionSurfaceV1, DecodedStrongSafepointRegistrationPlanV1,
    DecodedStrongStaticStorageRegistrationPlanV1, StrongRegistrationProductionBuildError,
    StrongRegistrationProductionSurfaceV1,
};
use crate::{
    BackendScalarKind, ImmortalObjectTypeRegistrationRefV1, LirTargetProfile, OdrFreeLirFoundation,
    PointerKind, RefScan, StaticImmortalRelocationPlanV1, StaticStorageScanKindV1,
    StrongDigestFinalizationPlanV1, StrongExternalLirBridgeSurfaceV1, StrongExternalLirBridgeV1,
    StrongImmortalObjectSemanticPlanSetV1, StrongImmortalObjectSemanticPlanV1,
    StrongInitializationSchedulePlanV1, StrongInitializationUnitSemanticPlanSetV1,
    StrongInitializationUnitSemanticPlanV1, StrongRegistrationIdentitySurfaceV1,
    StrongRegistrationIdentityValidationError, StrongSafepointSemanticPlanSetV1,
    StrongSafepointSemanticPlanV1, StrongStaticStorageInitialStatePlanV1,
    StrongStaticStorageSemanticPlanSetV1, StrongStaticStorageSemanticPlanV1, generated_unit_body,
    startup_gateway_body,
};

impl DecodedStrongRegistrationProductionSurfaceV1 {
    pub fn validate(
        self,
        target: LirTargetProfile,
        foundation: &OdrFreeLirFoundation,
        digests: &StrongDigestFinalizationPlanV1,
        external_bridges: &StrongExternalLirBridgeSurfaceV1,
    ) -> Result<StrongRegistrationProductionSurfaceV1, StrongRegistrationProductionValidationError>
    {
        let actual = encode(&self).map_err(StrongRegistrationProductionValidationError::Encode)?;
        let identities = self
            .identities
            .validate(foundation, digests)
            .map_err(StrongRegistrationProductionValidationError::Identities)?;
        let safepoint_semantics = validate_safepoints(self.safepoints, foundation, &identities)?;
        validate_derived_table(
            RegistrationProductionTableV1::Callable,
            &self.callables,
            crate::StrongCallableRegistrationPlanSetV1::new(foundation, &identities, digests)
                .map_err(|error| {
                    StrongRegistrationProductionValidationError::Expected(
                        StrongRegistrationProductionBuildError::Callables(error),
                    )
                })?
                .registrations(),
        )?;
        validate_derived_table(
            RegistrationProductionTableV1::Type,
            &self.types,
            crate::StrongTypeRegistrationPlanSetV1::new(target, foundation, &identities, digests)
                .map_err(|error| {
                    StrongRegistrationProductionValidationError::Expected(
                        StrongRegistrationProductionBuildError::Types(error),
                    )
                })?
                .registrations(),
        )?;
        let immortal_semantics = validate_immortal_objects(
            self.immortal_objects,
            target,
            foundation,
            &identities,
            external_bridges,
        )?;
        let static_semantics =
            validate_static_storages(self.static_storages, target, foundation, &identities)?;
        let initialization_semantics = validate_initialization_units(
            self.initialization_units,
            target,
            foundation,
            &identities,
            static_semantics,
        )?;
        let expected = StrongRegistrationProductionSurfaceV1::from_semantics(
            target,
            foundation,
            digests,
            identities,
            safepoint_semantics,
            immortal_semantics,
            initialization_semantics,
        )
        .map_err(StrongRegistrationProductionValidationError::Expected)?;
        let expected_bytes =
            encode(&expected).map_err(StrongRegistrationProductionValidationError::Encode)?;
        if actual != expected_bytes {
            return Err(StrongRegistrationProductionValidationError::SurfaceMismatch);
        }
        Ok(expected)
    }
}

fn validate_safepoints(
    decoded: Vec<DecodedStrongSafepointRegistrationPlanV1>,
    foundation: &OdrFreeLirFoundation,
    identities: &StrongRegistrationIdentitySurfaceV1,
) -> Result<StrongSafepointSemanticPlanSetV1, StrongRegistrationProductionValidationError> {
    require_length(
        RegistrationProductionTableV1::Safepoint,
        decoded.len(),
        identities.safepoints().len(),
    )?;
    let mut sites = Vec::with_capacity(decoded.len());
    for (index, (decoded, identity)) in decoded.into_iter().zip(identities.safepoints()).enumerate()
    {
        let site = verify_expected(
            decoded.site,
            identity.semantic_id(),
            RegistrationProductionTableV1::Safepoint,
            index,
            "site",
        )?;
        let site_record = foundation
            .safepoint_sites()
            .iter()
            .find(|record| record.id() == site)
            .ok_or_else(|| {
                semantic_error(RegistrationProductionTableV1::Safepoint, index, "site")
            })?;
        let owner = verify_expected(
            decoded.owner,
            site_record.key().owner(),
            RegistrationProductionTableV1::Safepoint,
            index,
            "owner",
        )?;
        let role = site_record.key().role();
        if decoded.role != role.tag() {
            return Err(semantic_error(
                RegistrationProductionTableV1::Safepoint,
                index,
                "role",
            ));
        }
        let mapping = foundation
            .safepoint_mappings()
            .iter()
            .find(|mapping| mapping.site() == site)
            .ok_or_else(|| {
                semantic_error(RegistrationProductionTableV1::Safepoint, index, "safepoint")
            })?;
        if decoded.safepoint != mapping.safepoint().get() {
            return Err(semantic_error(
                RegistrationProductionTableV1::Safepoint,
                index,
                "safepoint",
            ));
        }
        sites.push(StrongSafepointSemanticPlanV1::from_artifact(
            site,
            mapping.safepoint(),
            owner,
            role,
            decoded.root_pair_count,
        ));
    }
    Ok(StrongSafepointSemanticPlanSetV1::from_artifact(
        foundation.producer(),
        sites,
    ))
}

fn validate_immortal_objects(
    decoded: Vec<DecodedStrongImmortalObjectRegistrationPlanV1>,
    target: LirTargetProfile,
    foundation: &OdrFreeLirFoundation,
    identities: &StrongRegistrationIdentitySurfaceV1,
    external_bridges: &StrongExternalLirBridgeSurfaceV1,
) -> Result<StrongImmortalObjectSemanticPlanSetV1, StrongRegistrationProductionValidationError> {
    require_length(
        RegistrationProductionTableV1::ImmortalObject,
        decoded.len(),
        identities.immortal_objects().len(),
    )?;
    let alignment = target.metadata_pointer_layout().alignment_bytes().max(
        target
            .scalar_layout(BackendScalarKind::I64)
            .alignment_bytes(),
    );
    let minimum_size = target
        .metadata_pointer_layout()
        .size_bytes()
        .checked_add(
            target
                .scalar_layout(BackendScalarKind::I64)
                .size_bytes()
                .saturating_mul(2),
        )
        .ok_or_else(|| {
            semantic_error(
                RegistrationProductionTableV1::ImmortalObject,
                0,
                "object_size",
            )
        })?;
    let mut objects = Vec::with_capacity(decoded.len());
    for (index, (decoded, identity)) in decoded
        .into_iter()
        .zip(identities.immortal_objects())
        .enumerate()
    {
        let object = verify_expected(
            decoded.object,
            identity.semantic_id(),
            RegistrationProductionTableV1::ImmortalObject,
            index,
            "object",
        )?;
        if decoded.required_alignment != alignment
            || decoded.object_size < minimum_size
            || decoded.object_size % alignment != 0
            || decoded.object_size > target.contract().maximum_managed_object_size()
        {
            return Err(semantic_error(
                RegistrationProductionTableV1::ImmortalObject,
                index,
                "object_shape",
            ));
        }
        let type_registration = match decoded.type_registration {
            DecodedImmortalObjectTypeRegistrationRefV1::Local(exact_type) => {
                let exact_type = resolve_known(
                    exact_type,
                    identities
                        .type_registrations()
                        .iter()
                        .map(|identity| identity.semantic_id()),
                    RegistrationProductionTableV1::ImmortalObject,
                    index,
                    "local_type_registration",
                )?;
                ImmortalObjectTypeRegistrationRefV1::Local(exact_type)
            }
            DecodedImmortalObjectTypeRegistrationRefV1::CoreExternal(exact_type) => {
                let exact_type = resolve_known(
                    exact_type,
                    external_bridges
                        .bridges()
                        .iter()
                        .filter_map(|bridge| match bridge {
                            StrongExternalLirBridgeV1::TypeDescriptor(bridge) => {
                                Some(bridge.target())
                            }
                            StrongExternalLirBridgeV1::Callable(_) => None,
                        }),
                    RegistrationProductionTableV1::ImmortalObject,
                    index,
                    "core_type_registration",
                )?;
                ImmortalObjectTypeRegistrationRefV1::CoreExternal(exact_type)
            }
        };
        let symbol = PersistentSymbolRequest::new(
            PersistentSymbolKey::ImmortalObject(object),
            LinkageClass::ConeStrong,
        )
        .map_err(|_| {
            semantic_error(
                RegistrationProductionTableV1::ImmortalObject,
                index,
                "object_symbol",
            )
        })?;
        objects.push(StrongImmortalObjectSemanticPlanV1::from_artifact(
            object,
            symbol,
            decoded.object_size,
            decoded.required_alignment,
            type_registration,
        ));
    }
    Ok(StrongImmortalObjectSemanticPlanSetV1::from_artifact(
        foundation.producer(),
        objects,
    ))
}

fn validate_static_storages(
    decoded: Vec<DecodedStrongStaticStorageRegistrationPlanV1>,
    target: LirTargetProfile,
    foundation: &OdrFreeLirFoundation,
    identities: &StrongRegistrationIdentitySurfaceV1,
) -> Result<StrongStaticStorageSemanticPlanSetV1, StrongRegistrationProductionValidationError> {
    require_length(
        RegistrationProductionTableV1::StaticStorage,
        decoded.len(),
        identities.static_storages().len(),
    )?;
    let mut storages = Vec::with_capacity(decoded.len());
    for (index, (decoded, identity)) in decoded
        .into_iter()
        .zip(identities.static_storages())
        .enumerate()
    {
        let storage = verify_expected(
            decoded.storage,
            identity.semantic_id(),
            RegistrationProductionTableV1::StaticStorage,
            index,
            "storage",
        )?;
        let layout = resolve_known(
            decoded.layout,
            foundation.layouts().iter().map(|record| record.id()),
            RegistrationProductionTableV1::StaticStorage,
            index,
            "layout",
        )?;
        let layout_record = foundation
            .layouts()
            .iter()
            .find(|record| record.id() == layout)
            .expect("the layout was resolved from this table");
        if layout_record.key().target_profile() != &target.wire_id()
            || layout_record.key().representation() == RepresentationRole::ManagedObject
        {
            return Err(semantic_error(
                RegistrationProductionTableV1::StaticStorage,
                index,
                "layout_relation",
            ));
        }
        let scan = resolve_known(
            decoded.scan,
            foundation.scans().iter().map(|record| record.id()),
            RegistrationProductionTableV1::StaticStorage,
            index,
            "scan",
        )?;
        let scan_record = foundation
            .scans()
            .iter()
            .find(|record| record.id() == scan)
            .expect("the scan was resolved from this table");
        if scan_record.key().layout() != layout || scan_record.key().role() != ScanRole::InlineValue
        {
            return Err(semantic_error(
                RegistrationProductionTableV1::StaticStorage,
                index,
                "scan_relation",
            ));
        }
        if decoded.required_alignment == 0 || !decoded.required_alignment.is_power_of_two() {
            return Err(semantic_error(
                RegistrationProductionTableV1::StaticStorage,
                index,
                "required_alignment",
            ));
        }
        if decoded.allocation_extent != decoded.byte_size.max(1) {
            return Err(semantic_error(
                RegistrationProductionTableV1::StaticStorage,
                index,
                "allocation_extent",
            ));
        }
        let scan_program = validate_scan_program(
            decoded.scan_program,
            storage,
            decoded.byte_size,
            target,
            index,
        )?;
        let scan_kind = match decoded.scan_kind {
            0 if scan_program == RefScan::None => StaticStorageScanKindV1::None,
            1 if scan_program.contains_reference() => StaticStorageScanKindV1::Recursive,
            _ => {
                return Err(semantic_error(
                    RegistrationProductionTableV1::StaticStorage,
                    index,
                    "scan_kind",
                ));
            }
        };
        let initial_state = validate_initial_state(
            decoded.initial_state,
            target,
            identities,
            decoded.allocation_extent,
            index,
        )?;
        let symbol = PersistentSymbolRequest::new(
            PersistentSymbolKey::StaticStorage(storage),
            LinkageClass::ConeStrong,
        )
        .map_err(|_| {
            semantic_error(
                RegistrationProductionTableV1::StaticStorage,
                index,
                "storage_symbol",
            )
        })?;
        storages.push(StrongStaticStorageSemanticPlanV1::from_artifact(
            storage,
            symbol,
            layout,
            scan,
            scan_program,
            scan_kind,
            decoded.byte_size,
            decoded.allocation_extent,
            decoded.required_alignment,
            initial_state,
        ));
    }
    Ok(StrongStaticStorageSemanticPlanSetV1::from_artifact(
        foundation.producer(),
        storages,
    ))
}

fn validate_scan_program(
    decoded: DecodedRefScan,
    _storage: PersistentStaticStorageId,
    byte_size: u64,
    target: LirTargetProfile,
    index: usize,
) -> Result<RefScan, StrongRegistrationProductionValidationError> {
    let scan = match decoded {
        DecodedRefScan::None => RefScan::None,
        DecodedRefScan::References(offsets) => RefScan::References(offsets),
        DecodedRefScan::Sequence(_) => {
            return Err(semantic_error(
                RegistrationProductionTableV1::StaticStorage,
                index,
                "non_canonical_scan",
            ));
        }
    };
    let RefScan::References(offsets) = &scan else {
        return Ok(scan);
    };
    if offsets.is_empty() || offsets.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(semantic_error(
            RegistrationProductionTableV1::StaticStorage,
            index,
            "scan_offsets",
        ));
    }
    let pointer = target.pointer_layout(PointerKind::Managed);
    for offset in offsets {
        let Some(end) = offset.checked_add(pointer.size_bytes()) else {
            return Err(semantic_error(
                RegistrationProductionTableV1::StaticStorage,
                index,
                "scan_offset",
            ));
        };
        if offset % pointer.alignment_bytes() != 0 || end > byte_size {
            return Err(semantic_error(
                RegistrationProductionTableV1::StaticStorage,
                index,
                "scan_offset",
            ));
        }
    }
    Ok(scan)
}

fn validate_initial_state(
    decoded: DecodedStrongStaticStorageInitialStatePlanV1,
    target: LirTargetProfile,
    identities: &StrongRegistrationIdentitySurfaceV1,
    allocation_extent: u64,
    index: usize,
) -> Result<StrongStaticStorageInitialStatePlanV1, StrongRegistrationProductionValidationError> {
    match decoded {
        DecodedStrongStaticStorageInitialStatePlanV1::ZeroedForRuntimeUnit => {
            Ok(StrongStaticStorageInitialStatePlanV1::ZeroedForRuntimeUnit)
        }
        DecodedStrongStaticStorageInitialStatePlanV1::EncodedStaticValue {
            initial_template,
            immortal_relocations,
        } => {
            if u64::try_from(initial_template.len()).ok() != Some(allocation_extent) {
                return Err(semantic_error(
                    RegistrationProductionTableV1::StaticStorage,
                    index,
                    "initial_template",
                ));
            }
            let mut relocations = Vec::with_capacity(immortal_relocations.len());
            let mut previous = None;
            for DecodedStaticImmortalRelocationPlanV1 {
                pointer_offset,
                target: decoded_target,
            } in immortal_relocations
            {
                if previous.is_some_and(|previous| previous >= pointer_offset) {
                    return Err(semantic_error(
                        RegistrationProductionTableV1::StaticStorage,
                        index,
                        "relocation_order",
                    ));
                }
                previous = Some(pointer_offset);
                let target_id = resolve_known(
                    decoded_target,
                    identities
                        .immortal_objects()
                        .iter()
                        .map(|identity| identity.semantic_id()),
                    RegistrationProductionTableV1::StaticStorage,
                    index,
                    "relocation_target",
                )?;
                let pointer = target.pointer_layout(PointerKind::Managed);
                let Some(end) = pointer_offset.checked_add(pointer.size_bytes()) else {
                    return Err(semantic_error(
                        RegistrationProductionTableV1::StaticStorage,
                        index,
                        "relocation_offset",
                    ));
                };
                if pointer_offset % pointer.alignment_bytes() != 0 || end > allocation_extent {
                    return Err(semantic_error(
                        RegistrationProductionTableV1::StaticStorage,
                        index,
                        "relocation_offset",
                    ));
                }
                let start = usize::try_from(pointer_offset).map_err(|_| {
                    semantic_error(
                        RegistrationProductionTableV1::StaticStorage,
                        index,
                        "relocation_offset",
                    )
                })?;
                let end = usize::try_from(end).map_err(|_| {
                    semantic_error(
                        RegistrationProductionTableV1::StaticStorage,
                        index,
                        "relocation_offset",
                    )
                })?;
                if initial_template[start..end].iter().any(|byte| *byte != 0) {
                    return Err(semantic_error(
                        RegistrationProductionTableV1::StaticStorage,
                        index,
                        "relocation_template",
                    ));
                }
                relocations.push(StaticImmortalRelocationPlanV1::from_artifact(
                    pointer_offset,
                    target_id,
                ));
            }
            Ok(StrongStaticStorageInitialStatePlanV1::EncodedStaticValue {
                initial_template,
                immortal_relocations: relocations,
            })
        }
    }
}

fn validate_initialization_units(
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

fn validate_derived_table<A: WireEncode, E: WireEncode>(
    table: RegistrationProductionTableV1,
    actual: &[A],
    expected: &[E],
) -> Result<(), StrongRegistrationProductionValidationError> {
    require_length(table, actual.len(), expected.len())?;
    for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
        let actual = encode(actual).map_err(StrongRegistrationProductionValidationError::Encode)?;
        let expected =
            encode(expected).map_err(StrongRegistrationProductionValidationError::Encode)?;
        if actual != expected {
            return Err(StrongRegistrationProductionValidationError::EntryMismatch {
                table,
                index,
            });
        }
    }
    Ok(())
}

fn require_length(
    table: RegistrationProductionTableV1,
    actual: usize,
    expected: usize,
) -> Result<(), StrongRegistrationProductionValidationError> {
    if actual == expected {
        Ok(())
    } else {
        Err(StrongRegistrationProductionValidationError::TableLength {
            table,
            expected,
            actual,
        })
    }
}

fn verify_expected<I: PersistentId>(
    decoded: DecodedPersistentId<I>,
    expected: I,
    table: RegistrationProductionTableV1,
    index: usize,
    field: &'static str,
) -> Result<I, StrongRegistrationProductionValidationError> {
    decoded
        .verify(expected)
        .map_err(|_| semantic_error(table, index, field))
}

fn resolve_known<I: PersistentId>(
    decoded: DecodedPersistentId<I>,
    candidates: impl IntoIterator<Item = I>,
    table: RegistrationProductionTableV1,
    index: usize,
    field: &'static str,
) -> Result<I, StrongRegistrationProductionValidationError> {
    candidates
        .into_iter()
        .find(|candidate| candidate.as_array() == decoded.as_array())
        .ok_or_else(|| semantic_error(table, index, field))
}

fn semantic_error(
    table: RegistrationProductionTableV1,
    index: usize,
    field: &'static str,
) -> StrongRegistrationProductionValidationError {
    StrongRegistrationProductionValidationError::Semantic {
        table,
        index,
        field,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RegistrationProductionTableV1 {
    Safepoint,
    Callable,
    Type,
    ImmortalObject,
    StaticStorage,
    InitializationUnit,
}

#[derive(Debug)]
pub enum StrongRegistrationProductionValidationError {
    Encode(scoop_wire::cbor::EncodeError),
    Identities(StrongRegistrationIdentityValidationError),
    TableLength {
        table: RegistrationProductionTableV1,
        expected: usize,
        actual: usize,
    },
    EntryMismatch {
        table: RegistrationProductionTableV1,
        index: usize,
    },
    Semantic {
        table: RegistrationProductionTableV1,
        index: usize,
        field: &'static str,
    },
    Expected(StrongRegistrationProductionBuildError),
    SurfaceMismatch,
}

impl fmt::Display for StrongRegistrationProductionValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid decoded strong registration production surface: {self:?}"
        )
    }
}

impl std::error::Error for StrongRegistrationProductionValidationError {}
