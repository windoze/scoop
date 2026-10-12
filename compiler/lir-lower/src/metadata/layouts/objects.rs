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
        mir::ClassRepresentation::Intrinsic(mir::IntrinsicClassRepresentation::Atomic(storage)) => {
            let layout = atomic_object_layout(context, storage.kind())?;
            Ok((layout.size, layout.align, layout.scan))
        }
        mir::ClassRepresentation::Intrinsic(
            mir::IntrinsicClassRepresentation::Any | mir::IntrinsicClassRepresentation::Nothing,
        ) => Ok((0, 1, lir::RefScan::None)),
        mir::ClassRepresentation::Intrinsic(mir::IntrinsicClassRepresentation::String) => {
            let layout = context.string_layout();
            Ok((layout.size, layout.align, lir::RefScan::None))
        }
        mir::ClassRepresentation::Intrinsic(
            mir::IntrinsicClassRepresentation::Array { element }
            | mir::IntrinsicClassRepresentation::MutableArray { element },
        ) => {
            let enum_shape =
                |id: mir::EnumId| Ok(repr_shape(context, &enums[enum_def_id(id)].repr));
            let (size, align) = size_align(context, module, &enum_shape, element)?;
            Ok((size, align, ref_scan(context, module, enums, element, 0)?))
        }
        mir::ClassRepresentation::Declared(declared) => {
            let (_, size, align, scan) = declared_class_layout(context, module, enums, declared)?;
            Ok((size, align, scan))
        }
    }
}

/// A derived instance preserves the complete base prefix, including tail
/// padding. Only newly declared fields are appended. Zero-sized fields
/// have canonical offset zero and do not advance the allocation cursor.
pub(crate) fn class_shape(
    context: &LoweringContext,
    module: &mir::Module,
    enums: &lir::EnumDefs,
    def: &mir::DeclaredClass,
) -> StorageResult<(Vec<u64>, u64, u64)> {
    let mut chain = vec![def];
    let mut current = def;
    let mut seen = std::collections::HashSet::new();
    while let Some(base) = current.base_class {
        // Integer subtype boxes share one scalar payload with the parent box.
        // Their runtime parent relation does not append inherited fields.
        if module
            .meta
            .boxed_types
            .iter()
            .any(|boxed| boxed.class() == base)
        {
            break;
        }
        if !seen.insert(base) {
            return Err(StorageLoweringError::InvalidRepresentation(
                "class base cycle",
            ));
        }
        current = match &module.classes[base].representation {
            mir::ClassRepresentation::Declared(base) => base,
            mir::ClassRepresentation::Intrinsic(_) => {
                return Err(StorageLoweringError::InvalidRepresentation(
                    "a declared class requires a declared base",
                ));
            }
        };
        chain.push(current);
    }
    let enum_shape = |id: mir::EnumId| Ok(repr_shape(context, &enums[enum_def_id(id)].repr));
    let header = context.object_header_layout();
    let mut prefix =
        lir::StorageGeometryV1::new(context.target_profile(), header.size, header.align)?;
    let mut offsets = Vec::new();
    let mut inherited_fields: &[mir::Field] = &[];
    for definition in chain.into_iter().rev() {
        let fields = definition.fields.as_slice();
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

fn declared_class_layout(
    context: &LoweringContext,
    module: &mir::Module,
    enums: &lir::EnumDefs,
    declared: &mir::DeclaredClass,
) -> StorageResult<(Vec<u64>, u64, u64, lir::RefScan)> {
    let (offsets, size, align) = class_shape(context, module, enums, declared)?;
    let fields: Vec<_> = declared
        .fields
        .iter()
        .map(|field| field.ty.clone())
        .collect();
    let scan = scan_fields(context, module, enums, &fields, &offsets, 0)?;
    Ok((offsets, size, align, scan))
}

pub(crate) fn class_definition_layout(
    context: &LoweringContext,
    module: &mir::Module,
    enums: &lir::EnumDefs,
    identity: lir::LayoutIdentity,
    def: &mir::ClassDef,
) -> StorageResult<lir::Layout> {
    Ok(match &def.representation {
        mir::ClassRepresentation::Declared(declared) => {
            let (offsets, size, align, scan) =
                declared_class_layout(context, module, enums, declared)?;
            lir::Layout {
                identity,
                name: def.name.clone(),
                size,
                align,
                fields: declared
                    .fields
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
                mir::IntrinsicClassRepresentation::Atomic(storage) => {
                    let layout = atomic_object_layout(context, storage.kind())?;
                    return Ok(lir::Layout {
                        identity,
                        name: def.name.clone(),
                        size: layout.size,
                        align: layout.align,
                        fields: Vec::new(),
                        c_layout: None,
                        interior_mutable: false,
                        kind: lir::LayoutKind::Plain { scan: layout.scan },
                    });
                }
                mir::IntrinsicClassRepresentation::Any => {
                    (0, 1, lir::IntrinsicTypeRepresentation::Any)
                }
                mir::IntrinsicClassRepresentation::Nothing => {
                    (0, 1, lir::IntrinsicTypeRepresentation::Nothing)
                }
                mir::IntrinsicClassRepresentation::String => {
                    let layout = context.string_layout();
                    (
                        layout.size,
                        layout.align,
                        lir::IntrinsicTypeRepresentation::String,
                    )
                }
                mir::IntrinsicClassRepresentation::Array { .. }
                | mir::IntrinsicClassRepresentation::MutableArray { .. } => {
                    unreachable!("intrinsic arrays use the typed ArrayType metadata arena")
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
