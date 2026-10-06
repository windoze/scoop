use super::*;

/// Layout of a class object (M6, runtime spec 2.1/2.2): the 16-byte
/// object header (M9: TD pointer + GC word) followed by the complete
/// base instance and this class's own fields. MIR lists fields in
/// base-first order. Returns size, align, and the reference offsets
/// relative to the object start (the header itself is not a scanned
/// reference). Boxed value types use the same shape: header + the
/// inline payload field.
pub(crate) fn class_layout(
    context: &LoweringContext,
    module: &mir::Module,
    enums: &lir::EnumDefs,
    def: &mir::ClassDef,
) -> StorageResult<(u64, u64, lir::RefScan)> {
    match &def.representation {
        mir::ClassRepresentation::Intrinsic(mir::IntrinsicTypeRepresentation::String) => {
            let layout = context.string_layout();
            return Ok((layout.size, layout.align, lir::RefScan::None));
        }
        mir::ClassRepresentation::Intrinsic(
            mir::IntrinsicTypeRepresentation::Array { element }
            | mir::IntrinsicTypeRepresentation::MutableArray { element },
        ) => {
            let enum_shape =
                |id: mir::EnumId| Ok(repr_shape(context, &enums[enum_def_id(id)].repr));
            let (size, align) = size_align(context, module, &enum_shape, element)?;
            return Ok((size, align, ref_scan(context, module, enums, element, 0)?));
        }
        mir::ClassRepresentation::Intrinsic(_) => {
            unreachable!("the registry fixes intrinsic declaration targets")
        }
        mir::ClassRepresentation::Declared { .. } => {}
    }
    let (offsets, size, align) = class_shape(context, module, enums, def)?;
    let fields: Vec<mir::Type> = def
        .declared_fields()
        .iter()
        .map(|field| field.ty.clone())
        .collect();
    let scan = scan_fields(context, module, enums, &fields, &offsets, 0)?;
    Ok((size, align, scan))
}

/// A derived instance preserves the complete base prefix, including tail
/// padding. Only newly declared fields are appended. Zero-sized fields
/// have canonical offset zero and do not advance the allocation cursor.
pub(crate) fn class_shape(
    context: &LoweringContext,
    module: &mir::Module,
    enums: &lir::EnumDefs,
    def: &mir::ClassDef,
) -> StorageResult<(Vec<u64>, u64, u64)> {
    let mut chain = vec![def];
    let mut current = def;
    let mut seen = std::collections::HashSet::new();
    while let Some(base) = current.base_class() {
        if !seen.insert(base) {
            return Err(StorageLoweringError::InvalidRepresentation(
                "class base cycle",
            ));
        }
        current = &module.classes[base];
        chain.push(current);
    }
    let enum_shape = |id: mir::EnumId| Ok(repr_shape(context, &enums[enum_def_id(id)].repr));
    let header = context.object_header_layout();
    let mut prefix =
        lir::StorageGeometryV1::new(context.target_profile(), header.size, header.align)?;
    let mut offsets = Vec::new();
    let mut inherited_fields: &[mir::Field] = &[];
    for definition in chain.into_iter().rev() {
        let fields = definition.declared_fields();
        let inherited_count = offsets.len();
        if inherited_count > fields.len()
            || fields
                .iter()
                .zip(inherited_fields)
                .any(|(field, inherited)| field.name != inherited.name || field.ty != inherited.ty)
        {
            return Err(StorageLoweringError::InvalidRepresentation(
                "class fields do not preserve the complete base prefix",
            ));
        }
        let mut cursor = lir::StorageLayoutCursorV1::with_prefix(prefix);
        for field in &fields[inherited_count..] {
            let (size, align) = size_align(context, module, &enum_shape, &field.ty)?;
            let geometry = lir::StorageGeometryV1::new(context.target_profile(), size, align)?;
            offsets.push(cursor.push(geometry)?.offset());
        }
        prefix = cursor.finish()?;
        inherited_fields = fields;
    }
    Ok((offsets, prefix.size(), prefix.alignment().get()))
}

pub(crate) fn class_definition_layout(
    context: &LoweringContext,
    module: &mir::Module,
    enums: &lir::EnumDefs,
    identity: lir::LayoutIdentity,
    def: &mir::ClassDef,
) -> StorageResult<lir::Layout> {
    Ok(match &def.representation {
        mir::ClassRepresentation::Declared { fields, .. } => {
            let (size, align, scan) = class_layout(context, module, enums, def)?;
            let offsets = class_shape(context, module, enums, def)?.0;
            lir::Layout {
                identity,
                name: def.name.clone(),
                size,
                align,
                fields: fields
                    .iter()
                    .zip(offsets)
                    .map(|(field, offset)| {
                        let enum_shape =
                            |id: mir::EnumId| Ok(repr_shape(context, &enums[enum_def_id(id)].repr));
                        let (_, access_align) =
                            size_align(context, module, &enum_shape, &field.ty)?;
                        Ok(lir::FieldLayout {
                            offset,
                            access_align,
                        })
                    })
                    .collect::<StorageResult<Vec<_>>>()?,
                c_layout: None,
                interior_mutable: false,
                kind: lir::LayoutKind::Plain { scan },
            }
        }
        mir::ClassRepresentation::Intrinsic(representation) => {
            let (size, align, kind) = match representation {
                mir::IntrinsicTypeRepresentation::String => {
                    let layout = context.string_layout();
                    (
                        layout.size,
                        layout.align,
                        lir::IntrinsicTypeRepresentation::String,
                    )
                }
                mir::IntrinsicTypeRepresentation::Array { .. }
                | mir::IntrinsicTypeRepresentation::MutableArray { .. } => {
                    unreachable!("intrinsic arrays use the typed ArrayType metadata arena")
                }
                mir::IntrinsicTypeRepresentation::Unit
                | mir::IntrinsicTypeRepresentation::Integer(_)
                | mir::IntrinsicTypeRepresentation::Float(_)
                | mir::IntrinsicTypeRepresentation::Char
                | mir::IntrinsicTypeRepresentation::Boolean => {
                    unreachable!("the registry fixes intrinsic declaration targets")
                }
                mir::IntrinsicTypeRepresentation::Ptr { .. }
                | mir::IntrinsicTypeRepresentation::FunPtr { .. } => {
                    unreachable!("compiler pointer declarations are value-type shells")
                }
            };
            lir::Layout {
                identity,
                name: def.name.clone(),
                size,
                align,
                fields: Vec::new(),
                c_layout: None,
                interior_mutable: false,
                kind: lir::LayoutKind::Intrinsic(kind),
            }
        }
    })
}

/// Closure object layout: the 16-byte managed header, one non-scanned code
/// pointer at offset 16, then naturally aligned inline capture fields.
pub(crate) fn closure_shape(
    context: &LoweringContext,
    module: &mir::Module,
    enums: &lir::EnumDefs,
    def: &mir::ClosureClass,
) -> StorageResult<(Vec<u64>, u64, u64, lir::RefScan)> {
    let enum_shape = |id: mir::EnumId| Ok(repr_shape(context, &enums[enum_def_id(id)].repr));
    let mut offsets = Vec::with_capacity(def.captures.len());
    let (_, prefix) = context.closure_prefix();
    let prefix = lir::StorageGeometryV1::new(context.target_profile(), prefix.size, prefix.align)?;
    let mut cursor = lir::StorageLayoutCursorV1::with_prefix(prefix);
    for capture in &def.captures {
        let (size, align) = size_align(context, module, &enum_shape, &capture.ty)?;
        offsets.push(
            cursor
                .push(lir::StorageGeometryV1::new(
                    context.target_profile(),
                    size,
                    align,
                )?)?
                .offset(),
        );
    }
    let fields: Vec<_> = def
        .captures
        .iter()
        .map(|capture| capture.ty.clone())
        .collect();
    let scan = scan_fields(context, module, enums, &fields, &offsets, 0)?;
    let geometry = cursor.finish()?;
    Ok((offsets, geometry.size(), geometry.alignment().get(), scan))
}
