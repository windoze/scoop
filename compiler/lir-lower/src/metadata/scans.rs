use super::*;

/// Normalize through the shared runtime scan implementation before emission.
pub(crate) fn sequence(
    parts: impl IntoIterator<Item = lir::RefScan>,
) -> StorageResult<lir::RefScan> {
    Ok(
        lir::CheckedRefScanV1::normalize(lir::RefScan::Sequence(parts.into_iter().collect()))?
            .into_ref_scan(),
    )
}

/// Scan program for `fields` laid out at `offsets`, shifted by `base`.
pub(crate) fn scan_fields(
    context: &LoweringContext,
    module: &mir::Module,
    enums: &lir::EnumDefs,
    fields: &[mir::Type],
    offsets: &[u64],
    base: u64,
) -> StorageResult<lir::RefScan> {
    if fields.len() != offsets.len() {
        return Err(StorageLoweringError::InvalidRepresentation(
            "field and offset counts differ",
        ));
    }
    let scans = fields
        .iter()
        .zip(offsets)
        .map(|(field, offset)| {
            let offset = base
                .checked_add(*offset)
                .ok_or(lir::RefScanValidationError::OffsetOverflow)?;
            ref_scan(context, module, enums, field, offset)
        })
        .collect::<StorageResult<Vec<_>>>()?;
    sequence(scans)
}

/// Recursive scan program for one inline value at `base`.
pub(crate) fn ref_scan(
    context: &LoweringContext,
    module: &mir::Module,
    enums: &lir::EnumDefs,
    ty: &mir::Type,
    base: u64,
) -> StorageResult<lir::RefScan> {
    Ok(match ty {
        mir::Type::Context(storage) => {
            if storage.role == mir::ContextStorageRole::Mark {
                lir::RefScan::References(vec![
                    base,
                    base + context.pointer_layout(lir::PointerKind::Managed).size,
                ])
            } else {
                lir::RefScan::References(vec![base])
            }
        }
        mir::Type::String
        | mir::Type::Class(_)
        | mir::Type::Interface(_)
        | mir::Type::Function(_)
        | mir::Type::Any => lir::RefScan::References(vec![base]),
        mir::Type::Struct(id)
            if matches!(
                module.structs[*id].representation,
                mir::StructRepresentation::Intrinsic(mir::IntrinsicTypeRepresentation::Char)
            ) =>
        {
            lir::RefScan::None
        }
        mir::Type::Struct(id) => {
            let fields: Vec<mir::Type> = module.structs[*id]
                .declared_fields()
                .iter()
                .map(|field| field.ty.clone())
                .collect();
            let enum_shape =
                |id: mir::EnumId| Ok(repr_shape(context, &enums[enum_def_id(id)].repr));
            let (field_layouts, _, _) =
                struct_shape(context, module, &enum_shape, &module.structs[*id])?;
            let offsets: Vec<_> = field_layouts.iter().map(|field| field.offset).collect();
            scan_fields(context, module, enums, &fields, &offsets, base)?
        }
        mir::Type::Tuple(fields) => {
            let enum_shape =
                |id: mir::EnumId| Ok(repr_shape(context, &enums[enum_def_id(id)].repr));
            let (offsets, _, _) = aggregate_shape(context, module, &enum_shape, fields)?;
            scan_fields(context, module, enums, fields, &offsets, base)?
        }
        mir::Type::Enum(id, _) => match &enums[enum_def_id(*id)].repr {
            lir::EnumRepr::Niche {
                kind: lir::NichePointerKind::Managed,
                ..
            } => lir::RefScan::References(vec![base]),
            lir::EnumRepr::Niche {
                kind: lir::NichePointerKind::Raw | lir::NichePointerKind::Code,
                ..
            } => lir::RefScan::None,
            lir::EnumRepr::Tagged { variants, .. } => sequence(
                module.enums[*id]
                    .variants
                    .iter()
                    .zip(variants)
                    .filter(|(_, repr)| !repr.gc_free)
                    .map(|(variant, repr)| {
                        let fields: Vec<mir::Type> = variant
                            .fields
                            .iter()
                            .map(|field| field.ty.clone())
                            .collect();
                        let offsets: Vec<u64> =
                            repr.fields.iter().map(|field| field.offset).collect();
                        scan_fields(context, module, enums, &fields, &offsets, base)
                    })
                    .collect::<StorageResult<Vec<_>>>()?,
            )?,
        },
        mir::Type::Unit
        | mir::Type::Integer(_)
        | mir::Type::MachineScalar(_)
        | mir::Type::Boolean
        | mir::Type::Ptr(_)
        | mir::Type::FunPtr(_) => lir::RefScan::None,
    })
}

/// Build inline storage only after the scan is proven against its complete
/// extent. In particular, zero-sized storage cannot silently discard roots.
pub(crate) fn value_storage(
    context: &LoweringContext,
    size: u64,
    alignment: u64,
    scan: lir::RefScan,
) -> StorageResult<lir::ValueStorageLayoutV1> {
    lir::StorageGeometryV1::new(context.target_profile(), size, alignment)?;
    let scan = lir::CheckedRefScanV1::normalize(scan)?;
    scan.validate_extent(size, alignment)?;
    Ok(if size == 0 {
        lir::ValueStorageLayoutV1::zero_sized(alignment)?
    } else {
        lir::ValueStorageLayoutV1::inline(size, alignment, scan.into_ref_scan())?
    })
}
