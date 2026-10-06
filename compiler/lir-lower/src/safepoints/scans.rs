use super::*;

/// Build an inline scan before translating it to the caller's storage base.
pub(crate) fn root_scan(
    context: &LoweringContext,
    ty: &lir::LirType,
    structs: &lir::StructDefs,
    enums: &lir::EnumDefs,
    base: u64,
) -> StorageResult<lir::RefScan> {
    scan_value(context, ty, structs, enums, base, &mut HashSet::new())
}

fn scan_value(
    context: &LoweringContext,
    ty: &lir::LirType,
    structs: &lir::StructDefs,
    enums: &lir::EnumDefs,
    base: u64,
    visiting: &mut HashSet<lir::StructDefId>,
) -> StorageResult<lir::RefScan> {
    let (size, alignment) = lir_size_align(context, ty, structs, enums)?;
    let scan = match ty {
        lir::LirType::Ptr(lir::PointerKind::Managed) => lir::RefScan::References(vec![0]),
        lir::LirType::Aggregate(fields) => {
            let (offsets, _, _) = layout::lir_aggregate_shape(context, fields, structs, enums)?;
            sequence(
                fields
                    .iter()
                    .zip(offsets)
                    .map(|(field, offset)| {
                        scan_value(context, field, structs, enums, offset, visiting)
                    })
                    .collect::<StorageResult<Vec<_>>>()?,
            )?
        }
        lir::LirType::Struct(id) => {
            if !visiting.insert(*id) {
                return Err(lir::RefScanValidationError::Cycle.into());
            }
            let definition = &structs[*id];
            let scan = sequence(
                (0..definition.field_count())
                    .map(|index| {
                        let ty = definition.field_storage_type(index).ok_or(
                            StorageLoweringError::InvalidRepresentation(
                                "missing struct field storage",
                            ),
                        )?;
                        let offset = definition
                            .field_layout(index)
                            .ok_or(StorageLoweringError::InvalidRepresentation(
                                "missing struct field placement",
                            ))?
                            .offset;
                        scan_value(context, &ty, structs, enums, offset, visiting)
                    })
                    .collect::<StorageResult<Vec<_>>>()?,
            )?;
            visiting.remove(id);
            scan
        }
        lir::LirType::Enum(id) => enums[*id].scan.clone(),
        lir::LirType::Void
        | lir::LirType::I1
        | lir::LirType::I8
        | lir::LirType::I16
        | lir::LirType::I32
        | lir::LirType::F32
        | lir::LirType::F64
        | lir::LirType::I64
        | lir::LirType::MachineScalar(_)
        | lir::LirType::Ptr(_)
        | lir::LirType::ExceptionRecord => lir::RefScan::None,
    };
    let checked = lir::CheckedRefScanV1::normalize(scan)?;
    checked.validate_extent(size, alignment)?;
    require_inline(checked.as_ref_scan())?;
    Ok(checked.translated(base)?.into_ref_scan())
}

fn require_inline(scan: &lir::RefScan) -> StorageResult<()> {
    match scan {
        lir::RefScan::None | lir::RefScan::References(_) => Ok(()),
        lir::RefScan::Sequence(parts) => {
            for part in parts {
                require_inline(part)?;
            }
            Ok(())
        }
        lir::RefScan::Array { .. } => Err(StorageLoweringError::InvalidRepresentation(
            "a safepoint inline value cannot contain a variable object scan",
        )),
    }
}

pub(super) fn flatten_scan(scan: &lir::RefScan, offsets: &mut Vec<u64>) -> StorageResult<()> {
    match scan {
        lir::RefScan::None => {}
        lir::RefScan::References(references) => offsets.extend(references),
        lir::RefScan::Sequence(parts) => {
            for part in parts {
                flatten_scan(part, offsets)?;
            }
        }
        lir::RefScan::Array { .. } => {
            return Err(StorageLoweringError::InvalidRepresentation(
                "a statepoint leaf set cannot contain a variable object scan",
            ));
        }
    }
    Ok(())
}
