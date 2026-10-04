use super::*;

pub(crate) fn struct_layout(
    context: &LoweringContext,
    module: &mir::Module,
    enums: &lir::EnumDefs,
    identity: lir::LayoutIdentity,
    definition: &mir::StructDef,
) -> StorageResult<lir::Layout> {
    if let mir::StructRepresentation::Intrinsic(representation) = &definition.representation {
        let (size, align, representation) = match representation {
            mir::IntrinsicTypeRepresentation::Integer(kind) => {
                let kind = integer_kind(*kind);
                let layout = context.integer_layout(kind);
                (
                    layout.size,
                    layout.align,
                    lir::IntrinsicTypeRepresentation::Integer(kind),
                )
            }
            mir::IntrinsicTypeRepresentation::Char => {
                let layout = context.scalar_layout(lir::BackendScalarKind::I32);
                (
                    layout.size,
                    layout.align,
                    lir::IntrinsicTypeRepresentation::Char,
                )
            }
            mir::IntrinsicTypeRepresentation::Boolean => {
                let layout = context.scalar_layout(lir::BackendScalarKind::I1);
                (
                    layout.size,
                    layout.align,
                    lir::IntrinsicTypeRepresentation::Boolean,
                )
            }
            mir::IntrinsicTypeRepresentation::Ptr { pointee } => {
                let layout = context.pointer_layout(lir::PointerKind::Raw);
                (
                    layout.size,
                    layout.align,
                    lir::IntrinsicTypeRepresentation::Ptr {
                        pointee: compiler_data_pointee(module, pointee),
                    },
                )
            }
            mir::IntrinsicTypeRepresentation::FunPtr { signature } => {
                let layout = context.pointer_layout(lir::PointerKind::Code);
                (
                    layout.size,
                    layout.align,
                    lir::IntrinsicTypeRepresentation::FunPtr {
                        signature: compiler_function_type(module, *signature),
                    },
                )
            }
            mir::IntrinsicTypeRepresentation::String
            | mir::IntrinsicTypeRepresentation::Array { .. }
            | mir::IntrinsicTypeRepresentation::MutableArray { .. } => {
                unreachable!("the registry fixes intrinsic declaration targets")
            }
        };
        return Ok(lir::Layout {
            identity,
            name: definition.name.clone(),
            size,
            align,
            fields: Vec::new(),
            c_layout: None,
            interior_mutable: false,
            kind: lir::LayoutKind::Intrinsic(representation),
        });
    }
    let enum_shape = |id: mir::EnumId| Ok(repr_shape(context, &enums[enum_def_id(id)].repr));
    let (fields, size, align) = struct_shape(context, module, &enum_shape, definition)?;
    let mir::StructRepresentation::Declared {
        c_layout,
        interior_mutable,
        fields: definition_fields,
        ..
    } = &definition.representation
    else {
        unreachable!()
    };
    let field_types: Vec<_> = definition_fields
        .iter()
        .map(|field| field.ty.clone())
        .collect();
    let offsets: Vec<_> = fields.iter().map(|field| field.offset).collect();
    let scan = scan_fields(context, module, enums, &field_types, &offsets, 0)?;
    Ok(lir::Layout {
        identity,
        name: definition.name.clone(),
        size,
        align,
        fields,
        c_layout: c_layout.map(lower_c_layout),
        interior_mutable: *interior_mutable,
        kind: lir::LayoutKind::Plain { scan },
    })
}

/// Layout of an enum value. Tagged enums expose one unconditional scan
/// over their disjoint ref-bearing slots; the runtime never reads tag.
pub(crate) fn enum_layout(
    context: &LoweringContext,
    enums: &lir::EnumDefs,
    id: mir::EnumId,
    identity: lir::LayoutIdentity,
    def: &mir::EnumDef,
) -> lir::Layout {
    match &enums[enum_def_id(id)].repr {
        lir::EnumRepr::Niche { .. } => {
            let (size, align) = repr_shape(context, &enums[enum_def_id(id)].repr);
            lir::Layout {
                identity,
                name: def.name.clone(),
                size,
                align,
                fields: Vec::new(),
                c_layout: None,
                interior_mutable: false,
                kind: lir::LayoutKind::Enum {
                    scan: enums[enum_def_id(id)].scan.clone(),
                },
            }
        }
        lir::EnumRepr::Tagged { .. } => {
            let (size, align) = repr_shape(context, &enums[enum_def_id(id)].repr);
            lir::Layout {
                identity,
                name: def.name.clone(),
                size,
                align,
                fields: Vec::new(),
                c_layout: None,
                interior_mutable: false,
                kind: lir::LayoutKind::Enum {
                    scan: enums[enum_def_id(id)].scan.clone(),
                },
            }
        }
    }
}
