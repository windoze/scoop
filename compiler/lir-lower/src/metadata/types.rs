use super::*;

/// The `lir::EnumDefId` of a MIR enum (the arenas are transposed 1:1).
pub(crate) fn enum_def_id(id: mir::EnumId) -> lir::EnumDefId {
    lir::EnumDefId::from_raw(id.into_raw())
}

pub(crate) fn struct_def_id(id: mir::StructId) -> lir::StructDefId {
    lir::StructDefId::from_raw(id.into_raw())
}

pub(crate) fn lower_structs(
    module: &mir::Module,
    enums: &Arena<lir::EnumDef>,
) -> Arena<lir::StructDef> {
    let enum_shape = |id: mir::EnumId| repr_shape(&enums[enum_def_id(id)].repr);
    let mut structs = Arena::new();
    for (_, definition) in module.structs.iter() {
        let (field_layouts, size, align) = struct_shape(module, &enum_shape, definition);
        let (fields, c_layout, interior_mutable) = match &definition.representation {
            mir::StructRepresentation::Declared {
                c_layout,
                interior_mutable,
                fields,
            } => (
                fields
                    .iter()
                    .zip(field_layouts)
                    .map(|(field, layout)| lir::StructField {
                        ty: lir_type(&field.ty),
                        layout,
                    })
                    .collect(),
                c_layout.map(|layout| lir::CLayout {
                    aligned: layout.aligned,
                    packed: layout.packed,
                }),
                *interior_mutable,
            ),
            mir::StructRepresentation::Intrinsic(_) => (Vec::new(), None, false),
        };
        structs.alloc(lir::StructDef {
            name: definition.name.clone(),
            fields,
            size,
            align,
            c_layout,
            interior_mutable,
        });
    }
    structs
}

/// Map a MIR type onto its LIR value type (DESIGN 2.4 / 3.4): Unit is
/// the empty aggregate, String and the M6 reference types pointers,
/// struct / tuple literal aggregates of their mapped fields, and
/// enums `LirType::Enum` — their representation lives in the
/// `EnumDef`, so the mapping is the identity on enum ids. Intrinsic arrays are
/// ordinary classes here and therefore managed pointers; their element layout
/// lives only in typed `ArrayType` metadata.
pub(crate) fn lir_type(ty: &mir::Type) -> lir::LirType {
    match ty {
        mir::Type::Unit => lir::LirType::Aggregate(Vec::new()),
        // UInt shares Int's machine word (M9, spec 11.2): the same
        // `i64` at LIR, so codegen needs no UInt-specific handling.
        mir::Type::Int | mir::Type::UInt => lir::LirType::I64,
        mir::Type::Boolean => lir::LirType::I1,
        mir::Type::String
        | mir::Type::Class(_)
        | mir::Type::Interface(_)
        | mir::Type::Function(_)
        | mir::Type::Any => lir::LirType::Ptr(lir::PointerKind::Managed),
        mir::Type::Ptr(_) => lir::LirType::Ptr(lir::PointerKind::Raw),
        mir::Type::FunPtr(_) => lir::LirType::Ptr(lir::PointerKind::Code),
        mir::Type::Struct(id) => lir::LirType::Struct(struct_def_id(*id)),
        mir::Type::Tuple(elements) => {
            lir::LirType::Aggregate(elements.iter().map(lir_type).collect())
        }
        mir::Type::Enum(id, _) => lir::LirType::Enum(enum_def_id(*id)),
    }
}

pub(crate) fn uses_indirect_result(enums: &Arena<lir::EnumDef>, ty: &lir::LirType) -> bool {
    match ty {
        lir::LirType::Aggregate(_) | lir::LirType::Struct(_) | lir::LirType::ExceptionRecord => {
            true
        }
        lir::LirType::Enum(id) => matches!(enums[*id].repr, lir::EnumRepr::Tagged { .. }),
        lir::LirType::Void | lir::LirType::I1 | lir::LirType::I64 | lir::LirType::Ptr(_) => false,
    }
}
