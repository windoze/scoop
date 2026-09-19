//! Static storage geometry, scans, and initial state validation.

use super::*;

pub(super) fn validate_static_storages(
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
        let decoded = decoded.semantic;
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
        DecodedRefScan::Sequence(_) | DecodedRefScan::Array { .. } => {
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
