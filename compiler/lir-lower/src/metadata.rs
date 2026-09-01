use super::*;

pub(super) fn enum_def_id(id: mir::EnumId) -> lir::EnumDefId {
    lir::EnumDefId::from_raw(id.into_raw())
}

pub(super) fn struct_def_id(id: mir::StructId) -> lir::StructDefId {
    lir::StructDefId::from_raw(id.into_raw())
}

pub(super) fn lower_structs(
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

/// Fix the representation of every MIR enum definition (spec 7.4).
pub(super) fn lower_enums(module: &mir::Module) -> Arena<lir::EnumDef> {
    let mut reprs: Vec<Option<lir::EnumRepr>> = Vec::new();
    reprs.resize_with(module.enums.len(), || None);
    for (id, _) in module.enums.iter() {
        compute_repr(module, &mut reprs, id);
    }
    let mut enums = Arena::new();
    for ((_, def), repr) in module.enums.iter().zip(reprs) {
        enums.alloc(lir::EnumDef {
            name: def.name.clone(),
            repr: repr.expect("compute_repr fills every entry"),
            scan: lir::RefScan::None,
        });
    }
    for (id, _) in module.enums.iter() {
        let scan = ref_scan(module, &enums, &mir::Type::Enum(id, Vec::new()), 0);
        enums[enum_def_id(id)].scan = scan;
    }
    enums
}

/// Every enum type nested inside `ty` (through tuple elements and
/// struct fields), for representation sizing.
pub(super) fn nested_enums(module: &mir::Module, ty: &mir::Type, out: &mut Vec<mir::EnumId>) {
    match ty {
        mir::Type::Enum(id, _) => out.push(*id),
        mir::Type::Tuple(elements) => {
            for element in elements {
                nested_enums(module, element, out);
            }
        }
        mir::Type::Struct(id) => match &module.structs[*id].representation {
            mir::StructRepresentation::Declared { fields, .. } => {
                for field in fields {
                    nested_enums(module, &field.ty, out);
                }
            }
            mir::StructRepresentation::Intrinsic(_) => {}
        },
        // References hide whatever they point at behind a pointer.
        mir::Type::Unit
        | mir::Type::Int
        | mir::Type::UInt
        | mir::Type::Boolean
        | mir::Type::String
        | mir::Type::Class(_)
        | mir::Type::Interface(_)
        | mir::Type::Function(_)
        | mir::Type::Ptr(_)
        | mir::Type::FunPtr(_)
        | mir::Type::Any => {}
    }
}

pub(super) fn is_niche_payload(ty: &mir::Type) -> bool {
    matches!(
        ty,
        mir::Type::String
            | mir::Type::Class(_)
            | mir::Type::Interface(_)
            | mir::Type::Function(_)
            | mir::Type::Any
            | mir::Type::Ptr(_)
            | mir::Type::FunPtr(_)
    )
}

/// Compute (memoized) the representation of one enum. Niche layout is
/// restricted to the exact Option-isomorphic cases from spec 7.4.
/// Tagged layout gives all GC-free variants one shared payload region
/// and every non-GC-free variant its own disjoint slot. Nested enums are
/// computed first because variant sizing needs their shapes.
pub(super) fn compute_repr(
    module: &mir::Module,
    reprs: &mut Vec<Option<lir::EnumRepr>>,
    id: mir::EnumId,
) {
    let index = id.into_raw().into_u32() as usize;
    if reprs[index].is_some() {
        return;
    }
    let def = &module.enums[id];
    let mut nested = Vec::new();
    for variant in &def.variants {
        for field in &variant.fields {
            nested_enums(module, &field.ty, &mut nested);
        }
    }
    for nested_id in nested {
        // A by-value recursive enum is infinitely sized; hir-lower
        // rejects it before this stage.
        assert!(nested_id != id, "a by-value recursive enum is unsized");
        compute_repr(module, reprs, nested_id);
    }

    // This is a semantic whitelist, not merely an LLVM pointer-shape
    // check: only managed refs, Ptr and FunPtr qualify.
    if def.variants.len() == 2 {
        let has_unit = def.variants.iter().any(|variant| variant.fields.is_empty());
        let payload = def
            .variants
            .iter()
            .enumerate()
            .find(|(_, variant)| !variant.fields.is_empty());
        if let (true, Some((payload_index, payload_variant))) = (has_unit, payload) {
            if payload_variant.fields.len() == 1 && is_niche_payload(&payload_variant.fields[0].ty)
            {
                reprs[index] = Some(lir::EnumRepr::Niche {
                    payload_variant: payload_index as u32,
                });
                return;
            }
        }
    }

    // First compute each variant's natural field layout independent of
    // its eventual slot assignment.
    let enum_shape = |id: mir::EnumId| {
        repr_shape(
            reprs[id.into_raw().into_u32() as usize]
                .as_ref()
                .expect("nested enum representations are computed first"),
        )
    };
    struct PendingVariant {
        fields: Vec<lir::LirType>,
        field_offsets: Vec<u64>,
        size: u64,
        align: u64,
        gc_free: bool,
    }

    let mut pending = Vec::new();
    for variant in &def.variants {
        let fields: Vec<lir::LirType> = variant
            .fields
            .iter()
            .map(|field| lir_type(&field.ty))
            .collect();
        let field_types: Vec<mir::Type> = variant
            .fields
            .iter()
            .map(|field| field.ty.clone())
            .collect();
        let (field_offsets, size, align) = aggregate_shape(module, &enum_shape, &field_types);
        pending.push(PendingVariant {
            fields,
            field_offsets,
            size,
            align,
            gc_free: variant.gc_free,
        });
    }

    // Pure-value variants all reuse this one region. Empty variants need
    // no bytes but retain the same offset in the structural metadata.
    let pure_size = pending
        .iter()
        .filter(|variant| variant.gc_free)
        .map(|variant| variant.size)
        .max()
        .unwrap_or(0);
    let pure_align = pending
        .iter()
        .filter(|variant| variant.gc_free)
        .map(|variant| variant.align)
        .max()
        .unwrap_or(1);
    let pure_offset = 8u64.next_multiple_of(pure_align);
    let mut cursor = pure_offset + pure_size;
    let mut align = 8u64.max(pure_align);
    let mut variants = Vec::with_capacity(pending.len());
    for variant in pending {
        let slot_offset = if !variant.gc_free {
            cursor = cursor.next_multiple_of(variant.align);
            let offset = cursor;
            cursor += variant.size;
            offset
        } else {
            pure_offset
        };
        align = align.max(variant.align);
        variants.push(lir::EnumVariantRepr {
            fields: variant.fields,
            field_offsets: variant
                .field_offsets
                .into_iter()
                .map(|offset| slot_offset + offset)
                .collect(),
            slot_offset,
            slot_size: variant.size,
            slot_align: variant.align,
            gc_free: variant.gc_free,
        });
    }
    let size = cursor.next_multiple_of(align);
    reprs[index] = Some(lir::EnumRepr::Tagged {
        variants,
        size,
        align,
    });
}

/// Size and alignment of an enum value from its representation: the
/// niche form is a bare pointer; tagged size/alignment are fixed by its
/// shared pure-value region and disjoint ref-bearing slots.
pub(super) fn repr_shape(repr: &lir::EnumRepr) -> (u64, u64) {
    match repr {
        lir::EnumRepr::Niche { .. } => (8, 8),
        lir::EnumRepr::Tagged { size, align, .. } => (*size, *align),
    }
}

/// The meta layouts (DESIGN 2.4 / 3.4): the runtime `String` object
/// header, the `Int` / `Boolean` scalars, every struct in declaration
/// order, every enum in declaration order (with fixed reference
/// offsets), every class in declaration order (M6: header + fields),
/// and every tuple type that appears in the module. Concrete arrays have a
/// separate, typed metadata arena rather than a second layout identity.
pub(super) fn layouts(
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
pub(super) fn aggregate_layout(
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

pub(super) fn struct_layout(
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
pub(super) fn enum_layout(
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
pub(super) fn class_layout(
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
pub(super) fn class_shape(
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

pub(super) fn class_definition_layout(
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
pub(super) fn closure_shape(
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

/// Class ids ordered base-before-derived (single inheritance: depth
/// in the base chain; ties keep declaration order).
pub(super) fn class_order(module: &mir::Module) -> Vec<mir::ClassId> {
    fn depth(module: &mir::Module, id: mir::ClassId) -> usize {
        match module.classes[id].base_class() {
            Some(base) => depth(module, base) + 1,
            None => 0,
        }
    }
    let mut order: Vec<mir::ClassId> = module.classes.iter().map(|(id, _)| id).collect();
    order.sort_by_key(|&id| depth(module, id));
    order
}

/// The global symbol of a type's TypeDescriptor (`scoop_td_<name>`,
/// runtime spec 2.2).
pub(super) fn td_symbol(name: &str) -> String {
    format!("scoop_td_{name}")
}

#[derive(Default)]
pub(super) struct TypeDescriptorRefs {
    classes: HashMap<mir::ClassId, lir::TypeDescriptorRef>,
    interfaces: HashMap<mir::InterfaceId, lir::TypeDescriptorRef>,
    function_types: HashMap<mir::FunctionTypeId, lir::TypeDescriptorRef>,
    closures: HashMap<mir::ClosureClassId, lir::TypeDescriptorRef>,
    boxed: Vec<(mir::Type, lir::TypeDescriptorRef)>,
    string: Option<lir::TypeDescriptorRef>,
}

impl TypeDescriptorRefs {
    pub(super) fn for_closure(&self, id: mir::ClosureClassId) -> lir::TypeDescriptorRef {
        self.closures[&id]
    }

    pub(super) fn for_type(&self, ty: &mir::Type) -> lir::TypeDescriptorRef {
        match ty {
            mir::Type::Class(id) => self.classes[id],
            mir::Type::Interface(id) => self.interfaces[id],
            mir::Type::String => self.string.expect("typed String descriptor"),
            mir::Type::Function(id) => self.function_types[id],
            mir::Type::Struct(_)
            | mir::Type::Enum(..)
            | mir::Type::Tuple(_)
            | mir::Type::Int
            | mir::Type::UInt
            | mir::Type::Boolean
            | mir::Type::Unit
            | mir::Type::Ptr(_)
            | mir::Type::FunPtr(_) => self
                .boxed
                .iter()
                .find_map(|(payload, descriptor)| (payload == ty).then_some(*descriptor))
                .unwrap_or_else(|| panic!("MIR did not supply a boxed descriptor for {ty:?}")),
            mir::Type::Any => unreachable!("Any has no referenceable TypeDescriptor"),
        }
    }
}

pub(super) fn dispatch_entry(
    slot: &mir::TableSlot,
    local_functions: &HashMap<mir::FunctionId, lir::LocalFunctionId>,
) -> lir::DispatchEntry {
    lir::DispatchEntry {
        callable: match slot {
            mir::TableSlot::Function(id) => lir::CallableRef::Local(local_functions[id]),
            mir::TableSlot::Runtime(function) => {
                lir::CallableRef::Runtime(lower_runtime_function(*function))
            }
        },
    }
}

const FIRST_GENERATED_TD_TYPE_ID: u64 = 2;

/// Build the complete local TypeDescriptor graph before lowering any body.
/// Parent, interface, dispatch and operand references can therefore use typed
/// ids directly; symbols remain emission attributes only.
pub(super) fn type_descriptors(
    module: &mir::Module,
    enums: &Arena<lir::EnumDef>,
    local_functions: &HashMap<mir::FunctionId, lir::LocalFunctionId>,
) -> (
    Arena<lir::TypeDescriptor>,
    TypeDescriptorRefs,
    lir::WellKnownTypeDescriptors,
) {
    let mut descriptors = Arena::new();
    let mut refs = TypeDescriptorRefs::default();
    let mut next_type_id = FIRST_GENERATED_TD_TYPE_ID;
    for (interface, def) in module.interfaces.iter() {
        let id = descriptors.alloc(lir::TypeDescriptor {
            name: def.name.clone(),
            symbol: td_symbol(&def.name),
            runtime_type_id: next_type_id,
            size: 0,
            align: 0,
            scan: lir::TypeDescriptorScan::Fixed(lir::RefScan::None),
            parent: None,
            vtable: Vec::new(),
            itables: Vec::new(),
        });
        next_type_id += 1;
        refs.interfaces
            .insert(interface, lir::TypeDescriptorRef::Local(id));
    }
    for (id, _) in module.function_types.iter() {
        let name = format!(
            "function${}",
            mir::encode_type(module, &mir::Type::Function(id))
        );
        let descriptor = descriptors.alloc(lir::TypeDescriptor {
            symbol: td_symbol(&name),
            name,
            runtime_type_id: next_type_id,
            size: 0,
            align: 0,
            scan: lir::TypeDescriptorScan::Fixed(lir::RefScan::None),
            parent: None,
            vtable: Vec::new(),
            itables: Vec::new(),
        });
        next_type_id += 1;
        refs.function_types
            .insert(id, lir::TypeDescriptorRef::Local(descriptor));
    }
    let mut string = None;
    for id in class_order(module) {
        let def = &module.classes[id];
        let is_string = matches!(
            def.representation,
            mir::ClassRepresentation::Intrinsic(mir::IntrinsicTypeRepresentation::String)
        );
        let runtime_type_id = if is_string {
            1
        } else {
            let assigned = next_type_id;
            next_type_id += 1;
            assigned
        };
        let descriptor =
            class_type_descriptor(module, enums, id, runtime_type_id, &refs, local_functions);
        let descriptor = lir::TypeDescriptorRef::Local(descriptors.alloc(descriptor));
        assert!(refs.classes.insert(id, descriptor).is_none());
        if is_string {
            assert!(
                string.replace(descriptor).is_none(),
                "one typed String TypeDescriptor"
            );
            refs.string = Some(descriptor);
        }
    }
    for (closure, def) in module.closure_classes.iter() {
        let (_, size, align, scan) = closure_shape(module, enums, def);
        let descriptor = descriptors.alloc(lir::TypeDescriptor {
            name: def.name.clone(),
            symbol: td_symbol(&def.name),
            runtime_type_id: next_type_id,
            size,
            align,
            scan: lir::TypeDescriptorScan::Fixed(scan),
            parent: Some(refs.function_types[&def.function_type]),
            vtable: Vec::new(),
            itables: def
                .bridges
                .iter()
                .map(|bridge| lir::ItableRecord {
                    interface: refs.function_types[&bridge.target],
                    slots: vec![dispatch_entry(
                        &mir::TableSlot::Function(bridge.function),
                        local_functions,
                    )],
                })
                .collect(),
        });
        next_type_id += 1;
        refs.closures
            .insert(closure, lir::TypeDescriptorRef::Local(descriptor));
    }
    refs.boxed = module
        .meta
        .boxed_types
        .iter()
        .map(|boxed| (boxed.payload.clone(), refs.classes[&boxed.class]))
        .collect();
    let string = string.expect("LocalConcreteHir supplies the typed intrinsic String descriptor");
    (descriptors, refs, lir::WellKnownTypeDescriptors { string })
}

pub(super) fn class_type_descriptor(
    module: &mir::Module,
    enums: &Arena<lir::EnumDef>,
    id: mir::ClassId,
    runtime_type_id: u64,
    refs: &TypeDescriptorRefs,
    local_functions: &HashMap<mir::FunctionId, lir::LocalFunctionId>,
) -> lir::TypeDescriptor {
    let def = &module.classes[id];
    let (size, align, scan) = class_layout(module, enums, def);
    let scan = match &def.representation {
        mir::ClassRepresentation::Intrinsic(
            mir::IntrinsicTypeRepresentation::Array { .. }
            | mir::IntrinsicTypeRepresentation::MutableArray { .. },
        ) => lir::TypeDescriptorScan::ArrayElement { stride: size, scan },
        _ => lir::TypeDescriptorScan::Fixed(scan),
    };
    lir::TypeDescriptor {
        name: def.name.clone(),
        symbol: if matches!(
            def.representation,
            mir::ClassRepresentation::Intrinsic(mir::IntrinsicTypeRepresentation::String)
        ) {
            lir::STRING_TD_SYMBOL.to_string()
        } else {
            td_symbol(&def.name)
        },
        runtime_type_id,
        size,
        align,
        scan,
        parent: def.base_class().map(|base| refs.classes[&base]),
        vtable: def
            .vtable
            .iter()
            .map(|slot| dispatch_entry(slot, local_functions))
            .collect(),
        itables: def
            .itables
            .iter()
            .map(|record| lir::ItableRecord {
                interface: refs.interfaces[&record.interface],
                slots: record
                    .slots
                    .iter()
                    .map(|slot| dispatch_entry(slot, local_functions))
                    .collect(),
            })
            .collect(),
    }
}

/// Transpose every concrete intrinsic array class application into one typed
/// LIR metadata record. The MIR class id -> LIR array id map is complete before
/// function lowering starts, so no instruction discovers metadata on demand.
pub(super) fn array_types(
    module: &mir::Module,
    enums: &Arena<lir::EnumDef>,
    descriptors: &TypeDescriptorRefs,
) -> (
    Arena<lir::ArrayType>,
    HashMap<mir::ClassId, lir::ArrayTypeId>,
) {
    let mut arrays = Arena::new();
    let mut ids = HashMap::new();
    for (class_id, class) in module.classes.iter() {
        let (kind, element) = match &class.representation {
            mir::ClassRepresentation::Intrinsic(mir::IntrinsicTypeRepresentation::Array {
                element,
            }) => (lir::ArrayKind::Immutable, element),
            mir::ClassRepresentation::Intrinsic(
                mir::IntrinsicTypeRepresentation::MutableArray { element },
            ) => (lir::ArrayKind::Mutable, element),
            mir::ClassRepresentation::Declared { .. } | mir::ClassRepresentation::Intrinsic(_) => {
                continue;
            }
        };
        let (element_size, element_align, _) = class_layout(module, enums, class);
        let id = arrays.alloc(lir::ArrayType {
            kind,
            element: lir_type(element),
            element_size,
            element_align,
            type_descriptor: descriptors.classes[&class_id],
        });
        assert!(ids.insert(class_id, id).is_none());
    }
    (arrays, ids)
}

/// Field offsets plus total size and alignment of an aggregate with
/// the given field types: each field sits at the next offset aligned
/// to its own alignment, and the size is rounded up to the aggregate
/// alignment (natural layout, as for LLVM literal structs). Enum
/// shapes come from `enum_shape`, so the same code serves both the
/// representation-computation phase and completed modules.
pub(super) fn aggregate_shape(
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
pub(super) fn struct_shape(
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
pub(super) fn size_align(
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

/// Normalize a list of scans: remove empty parts, flatten sequences,
/// and merge plain reference lists.
pub(super) fn sequence(parts: impl IntoIterator<Item = lir::RefScan>) -> lir::RefScan {
    fn collect(scan: lir::RefScan, refs: &mut Vec<u64>) {
        match scan {
            lir::RefScan::None => {}
            lir::RefScan::References(offsets) => refs.extend(offsets),
            lir::RefScan::Sequence(parts) => {
                for part in parts {
                    collect(part, refs);
                }
            }
        }
    }

    let mut refs = Vec::new();
    for part in parts {
        collect(part, &mut refs);
    }
    if refs.is_empty() {
        lir::RefScan::None
    } else {
        lir::RefScan::References(refs)
    }
}

/// Scan program for `fields` laid out at `offsets`, shifted by `base`.
pub(super) fn scan_fields(
    module: &mir::Module,
    enums: &Arena<lir::EnumDef>,
    fields: &[mir::Type],
    offsets: &[u64],
    base: u64,
) -> lir::RefScan {
    sequence(
        fields
            .iter()
            .zip(offsets)
            .map(|(field, offset)| ref_scan(module, enums, field, base + offset)),
    )
}

/// Recursive scan program for one inline value at `base`.
pub(super) fn ref_scan(
    module: &mir::Module,
    enums: &Arena<lir::EnumDef>,
    ty: &mir::Type,
    base: u64,
) -> lir::RefScan {
    match ty {
        mir::Type::String
        | mir::Type::Class(_)
        | mir::Type::Interface(_)
        | mir::Type::Function(_)
        | mir::Type::Any => lir::RefScan::References(vec![base]),
        mir::Type::Struct(id) => {
            let fields: Vec<mir::Type> = module.structs[*id]
                .declared_fields()
                .iter()
                .map(|field| field.ty.clone())
                .collect();
            let enum_shape = |id: mir::EnumId| repr_shape(&enums[enum_def_id(id)].repr);
            let (field_layouts, _, _) = struct_shape(module, &enum_shape, &module.structs[*id]);
            let offsets: Vec<_> = field_layouts.iter().map(|field| field.offset).collect();
            scan_fields(module, enums, &fields, &offsets, base)
        }
        mir::Type::Tuple(fields) => {
            let enum_shape = |id: mir::EnumId| repr_shape(&enums[enum_def_id(id)].repr);
            let (offsets, _, _) = aggregate_shape(module, &enum_shape, fields);
            scan_fields(module, enums, fields, &offsets, base)
        }
        mir::Type::Enum(id, _) => match &enums[enum_def_id(*id)].repr {
            lir::EnumRepr::Niche { payload_variant } => {
                let variant = &module.enums[*id].variants[*payload_variant as usize];
                let [field] = variant.fields.as_slice() else {
                    unreachable!("a niche payload variant has exactly one pointer-like field")
                };
                ref_scan(module, enums, &field.ty, base)
            }
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
                        scan_fields(module, enums, &fields, &repr.field_offsets, base)
                    }),
            ),
        },
        mir::Type::Unit
        | mir::Type::Int
        | mir::Type::UInt
        | mir::Type::Boolean
        | mir::Type::Ptr(_)
        | mir::Type::FunPtr(_) => lir::RefScan::None,
    }
}

/// Record every tuple type reachable from `ty`
/// (first-appearance order, duplicates skipped) so each gets a meta
/// layout. Structs and enums are covered by their own
/// declaration-driven layout sections.
pub(super) fn record_layout_types(ty: &mir::Type, types: &mut Vec<mir::Type>) {
    if let mir::Type::Tuple(elements) = ty {
        if !types.contains(ty) {
            types.push(ty.clone());
        }
        for element in elements {
            record_layout_types(element, types);
        }
    }
}

/// Map a MIR type onto its LIR value type (DESIGN 2.4 / 3.4): Unit is
/// the empty aggregate, String and the M6 reference types pointers,
/// struct / tuple literal aggregates of their mapped fields, and
/// enums `LirType::Enum` — their representation lives in the
/// `EnumDef`, so the mapping is the identity on enum ids. Intrinsic arrays are
/// ordinary classes here and therefore managed pointers; their element layout
/// lives only in typed `ArrayType` metadata.
pub(super) fn lir_type(ty: &mir::Type) -> lir::LirType {
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

pub(super) fn uses_indirect_result(enums: &Arena<lir::EnumDef>, ty: &lir::LirType) -> bool {
    match ty {
        lir::LirType::Aggregate(_) | lir::LirType::Struct(_) | lir::LirType::ExceptionRecord => {
            true
        }
        lir::LirType::Enum(id) => matches!(enums[*id].repr, lir::EnumRepr::Tagged { .. }),
        lir::LirType::Void | lir::LirType::I1 | lir::LirType::I64 | lir::LirType::Ptr(_) => false,
    }
}
