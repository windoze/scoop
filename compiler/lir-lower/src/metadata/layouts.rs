use super::*;

/// The meta layouts (DESIGN 2.4 / 3.4): the runtime `String` object
/// header, the `Int` / `Boolean` scalars, every struct in declaration
/// order, every enum in declaration order (with fixed reference
/// offsets), every class in declaration order (M6: header + fields),
/// and every tuple type that appears in the module. Concrete arrays have a
/// separate, typed metadata arena rather than a second layout identity.
pub(crate) fn layouts(
    module: &mir::Module,
    enums: &Arena<lir::EnumDef>,
    from_code: &[mir::Type],
) -> (Arena<lir::Layout>, lir::WellKnownLayouts) {
    // Tuple types reachable from struct / enum / class declarations
    // appear even when no code value mentions them directly.
    let mut types = Vec::new();
    for (_, def) in module.structs.iter() {
        match &def.representation {
            mir::StructRepresentation::Declared { fields, .. } => {
                for field in fields {
                    record_layout_types(&field.ty, &mut types);
                }
            }
            mir::StructRepresentation::Intrinsic(_) => {}
        }
    }
    for (_, def) in module.enums.iter() {
        for variant in &def.variants {
            for field in &variant.fields {
                record_layout_types(&field.ty, &mut types);
            }
        }
    }
    for (_, def) in module.classes.iter() {
        match &def.representation {
            mir::ClassRepresentation::Declared { fields, .. } => {
                for field in fields {
                    record_layout_types(&field.ty, &mut types);
                }
            }
            mir::ClassRepresentation::Intrinsic(
                mir::IntrinsicTypeRepresentation::Array { element }
                | mir::IntrinsicTypeRepresentation::MutableArray { element },
            ) => record_layout_types(element, &mut types),
            mir::ClassRepresentation::Intrinsic(_) => {}
        }
    }
    for (_, def) in module.closure_classes.iter() {
        for field in &def.captures {
            record_layout_types(&field.ty, &mut types);
        }
    }
    for ty in from_code {
        record_layout_types(ty, &mut types);
    }

    let mut layouts = Arena::new();
    for (_, def) in module.structs.iter() {
        layouts.alloc(struct_layout(module, enums, def));
    }
    for (id, def) in module.enums.iter() {
        layouts.alloc(enum_layout(module, enums, id, def));
    }
    let mut string = None;
    for (_, def) in module.classes.iter() {
        match def.representation {
            mir::ClassRepresentation::Intrinsic(mir::IntrinsicTypeRepresentation::String) => {
                let layout = class_definition_layout(module, enums, def);
                assert!(
                    string.replace(layouts.alloc(layout)).is_none(),
                    "one typed String representation"
                );
            }
            mir::ClassRepresentation::Intrinsic(
                mir::IntrinsicTypeRepresentation::Array { .. }
                | mir::IntrinsicTypeRepresentation::MutableArray { .. },
            ) => {}
            _ => {
                layouts.alloc(class_definition_layout(module, enums, def));
            }
        }
    }
    for (_, def) in module.closure_classes.iter() {
        let (_, size, align, scan) = closure_shape(module, enums, def);
        layouts.alloc(lir::Layout {
            name: def.name.clone(),
            size,
            align,
            fields: Vec::new(),
            c_layout: None,
            interior_mutable: false,
            kind: lir::LayoutKind::Plain { scan },
        });
    }
    // Tuple layouts keep their first-appearance order.
    for ty in &types {
        if let mir::Type::Tuple(elements) = ty {
            layouts.alloc(aggregate_layout(
                module,
                enums,
                mir::type_name(module, ty),
                elements,
            ));
        }
    }
    (
        layouts,
        lir::WellKnownLayouts {
            string: string
                .expect("LocalConcreteHir supplies the typed intrinsic String representation"),
        },
    )
}

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

/// Layout of a class object (M6, runtime spec 2.1/2.2): the 16-byte
/// object header (M9: TD pointer + GC word) followed by the fields
/// — mir-lower already flattened the base-class prefix into
/// `ClassDef::fields`. Returns size, align, and the reference offsets
/// relative to the object start (the header itself is not a scanned
/// reference). Boxed value types use the same shape: header + the
/// inline payload field.
pub(crate) fn class_layout(
    module: &mir::Module,
    enums: &Arena<lir::EnumDef>,
    def: &mir::ClassDef,
) -> (u64, u64, lir::RefScan) {
    match &def.representation {
        mir::ClassRepresentation::Intrinsic(mir::IntrinsicTypeRepresentation::String) => {
            return (24, 8, lir::RefScan::None);
        }
        mir::ClassRepresentation::Intrinsic(
            mir::IntrinsicTypeRepresentation::Array { element }
            | mir::IntrinsicTypeRepresentation::MutableArray { element },
        ) => {
            let enum_shape = |id: mir::EnumId| repr_shape(&enums[enum_def_id(id)].repr);
            let (size, align) = size_align(module, &enum_shape, element);
            return (
                size.next_multiple_of(align),
                align,
                ref_scan(module, enums, element, 0),
            );
        }
        mir::ClassRepresentation::Intrinsic(_) => {
            unreachable!("the registry fixes intrinsic declaration targets")
        }
        mir::ClassRepresentation::Declared { .. } => {}
    }
    let (offsets, size, align) = class_shape(module, enums, def);
    let fields: Vec<mir::Type> = def
        .declared_fields()
        .iter()
        .map(|field| field.ty.clone())
        .collect();
    let scan = scan_fields(module, enums, &fields, &offsets, 0);
    (size, align, scan)
}

/// Natural object layout for one flattened class: the header occupies
/// bytes 0..16 and each base/derived field starts at the next address
/// satisfying its own alignment. LIR heap operations consume these
/// byte offsets directly, so sub-word fields are not rounded to slots.
pub(crate) fn class_shape(
    module: &mir::Module,
    enums: &Arena<lir::EnumDef>,
    def: &mir::ClassDef,
) -> (Vec<u64>, u64, u64) {
    let fields = def.declared_fields();
    let enum_shape = |id: mir::EnumId| repr_shape(&enums[enum_def_id(id)].repr);
    let mut offsets = Vec::with_capacity(fields.len());
    let mut size = 16u64;
    let mut align = 8u64;
    for field in fields {
        let (field_size, field_align) = size_align(module, &enum_shape, &field.ty);
        let offset = size.next_multiple_of(field_align);
        offsets.push(offset);
        size = offset + field_size;
        align = align.max(field_align);
    }
    (offsets, size.next_multiple_of(align), align)
}

pub(crate) fn class_definition_layout(
    module: &mir::Module,
    enums: &Arena<lir::EnumDef>,
    def: &mir::ClassDef,
) -> lir::Layout {
    match &def.representation {
        mir::ClassRepresentation::Declared { fields, .. } => {
            let (size, align, scan) = class_layout(module, enums, def);
            let offsets = class_shape(module, enums, def).0;
            lir::Layout {
                name: def.name.clone(),
                size,
                align,
                fields: fields
                    .iter()
                    .zip(offsets)
                    .map(|(field, offset)| {
                        let enum_shape = |id: mir::EnumId| repr_shape(&enums[enum_def_id(id)].repr);
                        let (_, access_align) = size_align(module, &enum_shape, &field.ty);
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
        mir::ClassRepresentation::Intrinsic(representation) => {
            let (size, align, kind) = match representation {
                mir::IntrinsicTypeRepresentation::String => {
                    (24, 8, lir::IntrinsicTypeRepresentation::String)
                }
                mir::IntrinsicTypeRepresentation::Array { .. }
                | mir::IntrinsicTypeRepresentation::MutableArray { .. } => {
                    unreachable!("intrinsic arrays use the typed ArrayType metadata arena")
                }
                mir::IntrinsicTypeRepresentation::Int
                | mir::IntrinsicTypeRepresentation::UInt
                | mir::IntrinsicTypeRepresentation::Boolean => {
                    unreachable!("the registry fixes intrinsic declaration targets")
                }
            };
            lir::Layout {
                name: def.name.clone(),
                size,
                align,
                fields: Vec::new(),
                c_layout: None,
                interior_mutable: false,
                kind: lir::LayoutKind::Intrinsic(kind),
            }
        }
    }
}

/// Closure object layout: the 16-byte managed header, one non-scanned code
/// pointer at offset 16, then naturally aligned inline capture fields.
pub(crate) fn closure_shape(
    module: &mir::Module,
    enums: &Arena<lir::EnumDef>,
    def: &mir::ClosureClass,
) -> (Vec<u64>, u64, u64, lir::RefScan) {
    let enum_shape = |id: mir::EnumId| repr_shape(&enums[enum_def_id(id)].repr);
    let mut offsets = Vec::with_capacity(def.captures.len());
    let mut size = 24u64;
    let mut align = 8u64;
    for capture in &def.captures {
        let (capture_size, capture_align) = size_align(module, &enum_shape, &capture.ty);
        let offset = size.next_multiple_of(capture_align);
        offsets.push(offset);
        size = offset + capture_size;
        align = align.max(capture_align);
    }
    let fields: Vec<_> = def
        .captures
        .iter()
        .map(|capture| capture.ty.clone())
        .collect();
    let scan = scan_fields(module, enums, &fields, &offsets, 0);
    (offsets, size.next_multiple_of(align), align, scan)
}

/// Field offsets plus total size and alignment of an aggregate with
/// the given field types: each field sits at the next offset aligned
/// to its own alignment, and the size is rounded up to the aggregate
/// alignment (natural layout, as for LLVM literal structs). Enum
/// shapes come from `enum_shape`, so the same code serves both the
/// representation-computation phase and completed modules.
pub(crate) fn aggregate_shape(
    module: &mir::Module,
    enum_shape: &dyn Fn(mir::EnumId) -> (u64, u64),
    fields: &[mir::Type],
) -> (Vec<u64>, u64, u64) {
    let mut offsets = Vec::with_capacity(fields.len());
    let mut size = 0u64;
    let mut align = 1u64;
    for field in fields {
        let (field_size, field_align) = size_align(module, enum_shape, field);
        let offset = size.next_multiple_of(field_align);
        offsets.push(offset);
        size = offset + field_size;
        align = align.max(field_align);
    }
    (offsets, size.next_multiple_of(align), align)
}

/// Exact layout of one named struct. Ordinary structs use natural field
/// alignment. `@CLayout(packed = N)` caps each field's access alignment at
/// `N`; `aligned = N` raises (but never lowers) the aggregate alignment.
pub(crate) fn struct_shape(
    module: &mir::Module,
    enum_shape: &dyn Fn(mir::EnumId) -> (u64, u64),
    definition: &mir::StructDef,
) -> (Vec<lir::FieldLayout>, u64, u64) {
    let mir::StructRepresentation::Declared {
        c_layout, fields, ..
    } = &definition.representation
    else {
        return match definition.representation {
            mir::StructRepresentation::Intrinsic(mir::IntrinsicTypeRepresentation::Int)
            | mir::StructRepresentation::Intrinsic(mir::IntrinsicTypeRepresentation::UInt) => {
                (Vec::new(), 8, 8)
            }
            mir::StructRepresentation::Intrinsic(mir::IntrinsicTypeRepresentation::Boolean) => {
                (Vec::new(), 1, 1)
            }
            mir::StructRepresentation::Intrinsic(_) => {
                unreachable!("the registry fixes intrinsic declaration targets")
            }
            mir::StructRepresentation::Declared { .. } => unreachable!(),
        };
    };
    let packed = c_layout.map(|layout| u64::from(layout.packed)).unwrap_or(0);
    let explicit_align = c_layout
        .map(|layout| u64::from(layout.aligned))
        .unwrap_or(0);
    let mut layouts = Vec::with_capacity(fields.len());
    let mut size = 0u64;
    let mut align = explicit_align.max(1);
    for field in fields {
        let (field_size, natural_align) = size_align(module, enum_shape, &field.ty);
        let access_align = if packed == 0 {
            natural_align
        } else {
            natural_align.min(packed)
        };
        let offset = size.next_multiple_of(access_align);
        layouts.push(lir::FieldLayout {
            offset,
            access_align,
        });
        size = offset + field_size;
        align = align.max(access_align);
    }
    (layouts, size.next_multiple_of(align), align)
}

/// Size and alignment of a value of type `ty`. `String` and the M6
/// reference types are pointers (pointer-sized); aggregates recurse;
/// enums take their representation's shape.
pub(crate) fn size_align(
    module: &mir::Module,
    enum_shape: &dyn Fn(mir::EnumId) -> (u64, u64),
    ty: &mir::Type,
) -> (u64, u64) {
    match ty {
        mir::Type::Unit => (0, 1),
        // UInt shares Int's machine word (M9, spec 11.2).
        mir::Type::Int | mir::Type::UInt => (8, 8),
        mir::Type::Boolean => (1, 1),
        mir::Type::String
        | mir::Type::Class(_)
        | mir::Type::Interface(_)
        | mir::Type::Function(_)
        | mir::Type::Ptr(_)
        | mir::Type::FunPtr(_)
        | mir::Type::Any => (8, 8),
        mir::Type::Struct(id) => {
            let (_, size, align) = struct_shape(module, enum_shape, &module.structs[*id]);
            (size, align)
        }
        mir::Type::Tuple(elements) => {
            let (_, size, align) = aggregate_shape(module, enum_shape, elements);
            (size, align)
        }
        mir::Type::Enum(id, _) => enum_shape(*id),
    }
}

/// Record every tuple type reachable from `ty`
/// (first-appearance order, duplicates skipped) so each gets a meta
/// layout. Structs and enums are covered by their own
/// declaration-driven layout sections.
pub(crate) fn record_layout_types(ty: &mir::Type, types: &mut Vec<mir::Type>) {
    if let mir::Type::Tuple(elements) = ty {
        if !types.contains(ty) {
            types.push(ty.clone());
        }
        for element in elements {
            record_layout_types(element, types);
        }
    }
}
