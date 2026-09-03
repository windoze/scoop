use super::*;

/// Layout of an aggregate value (struct / tuple / Unit): fields in
/// declaration order at their natural alignment. The recursive scan
/// program preserves references nested in aggregates and tagged enums.
pub(crate) fn aggregate_layout(
    module: &mir::Module,
    enums: &Arena<lir::EnumDef>,
    name: String,
    fields: &[mir::Type],
) -> lir::Layout {
    let enum_shape = |id: mir::EnumId| repr_shape(&enums[enum_def_id(id)].repr);
    let (offsets, size, align) = aggregate_shape(module, &enum_shape, fields);
    let scan = scan_fields(module, enums, fields, &offsets, 0);
    lir::Layout {
        name,
        size,
        align,
        fields: offsets
            .iter()
            .zip(fields)
            .map(|(&offset, field)| {
                let (_, access_align) = size_align(module, &enum_shape, field);
                lir::FieldLayout {
                    offset,
                    access_align,
                }
            })
            .collect(),
        c_layout: None,
        interior_mutable: false,
        kind: lir::LayoutKind::Plain { scan },
    }
}

pub(crate) fn struct_layout(
    module: &mir::Module,
    enums: &Arena<lir::EnumDef>,
    definition: &mir::StructDef,
) -> lir::Layout {
    if let mir::StructRepresentation::Intrinsic(representation) = &definition.representation {
        let (size, align, representation) = match representation {
            mir::IntrinsicTypeRepresentation::Int => (8, 8, lir::IntrinsicTypeRepresentation::Int),
            mir::IntrinsicTypeRepresentation::UInt => {
                (8, 8, lir::IntrinsicTypeRepresentation::UInt)
            }
            mir::IntrinsicTypeRepresentation::Boolean => {
                (1, 1, lir::IntrinsicTypeRepresentation::Boolean)
            }
            mir::IntrinsicTypeRepresentation::String
            | mir::IntrinsicTypeRepresentation::Array { .. }
            | mir::IntrinsicTypeRepresentation::MutableArray { .. } => {
                unreachable!("the registry fixes intrinsic declaration targets")
            }
        };
        return lir::Layout {
            name: definition.name.clone(),
            size,
            align,
            fields: Vec::new(),
            c_layout: None,
            interior_mutable: false,
            kind: lir::LayoutKind::Intrinsic(representation),
        };
    }
    let enum_shape = |id: mir::EnumId| repr_shape(&enums[enum_def_id(id)].repr);
    let (fields, size, align) = struct_shape(module, &enum_shape, definition);
    let mir::StructRepresentation::Declared {
        c_layout,
        interior_mutable,
        fields: definition_fields,
    } = &definition.representation
    else {
        unreachable!()
    };
    let field_types: Vec<_> = definition_fields
        .iter()
        .map(|field| field.ty.clone())
        .collect();
    let offsets: Vec<_> = fields.iter().map(|field| field.offset).collect();
    let scan = scan_fields(module, enums, &field_types, &offsets, 0);
    lir::Layout {
        name: definition.name.clone(),
        size,
        align,
        fields,
        c_layout: c_layout.map(|layout| lir::CLayout {
            aligned: layout.aligned,
            packed: layout.packed,
        }),
        interior_mutable: *interior_mutable,
        kind: lir::LayoutKind::Plain { scan },
    }
}

/// Layout of an enum value. Tagged enums expose one unconditional scan
/// over their disjoint ref-bearing slots; the runtime never reads tag.
pub(crate) fn enum_layout(
    _module: &mir::Module,
    enums: &Arena<lir::EnumDef>,
    id: mir::EnumId,
    def: &mir::EnumDef,
) -> lir::Layout {
    match &enums[enum_def_id(id)].repr {
        lir::EnumRepr::Niche { .. } => lir::Layout {
            name: def.name.clone(),
            size: 8,
            align: 8,
            fields: Vec::new(),
            c_layout: None,
            interior_mutable: false,
            kind: lir::LayoutKind::Enum {
                scan: enums[enum_def_id(id)].scan.clone(),
            },
        },
        lir::EnumRepr::Tagged { .. } => {
            let (size, align) = repr_shape(&enums[enum_def_id(id)].repr);
            lir::Layout {
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
