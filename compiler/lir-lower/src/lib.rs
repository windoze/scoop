//! LIR stage: type layout, statepoint insertion, exception lowering.
//! LIR contains nothing Scoop-specific and is mechanically translatable
//! to the target IR.
//!
//! See `docs/specs/SCOOP-IMPL-SPEC.md` section 2.4 and
//! `docs/milestone4/DESIGN.md` section 3.4.
//!
//! M2: structured MIR control flow becomes basic blocks with
//! terminators, and `&&` / `||` are expanded here into short-circuit
//! branches. Every MIR local gets a stack slot (stores on declaration
//! and assignment, implicit loads on use); SSA construction is left to
//! LLVM's mem2reg. Struct / tuple / Unit values are LLVM literal
//! structs, and the type layouts — including reference-field offsets,
//! which the M9 GC depends on — are computed into the LIR meta. This
//! stage never fails: all errors were already reported by hir-lower.
//!
//! M3: function signatures. Parameters are SSA values (`Value::Param`),
//! not stack slots — they are immutable, so no store ever targets them.
//! `Unit`-returning functions are void at the LLVM level: their
//! `return` carries no value (Unit values are still materialized as
//! empty aggregates where produced; they are just never returned).
//!
//! M4: enums. Every MIR enum definition gets an `EnumDef` with a fixed
//! representation (spec 7.4): the niche pointer form when there are
//! exactly two variants, one without fields and the other with a
//! single field mapping to `Ptr` (`None` = null — this is
//! `Option<String>`); otherwise the `{ i64 tag, [N x i8] payload }`
//! tagged form with N and alignment taken from the largest variant.
//! The MIR enum operations map onto `EnumWrap` / `EnumTag` /
//! `EnumField`, which codegen translates mechanically per the
//! representation. A trap call (`!!` on `None`) branches to a
//! per-function shared trap block that calls `scoop_rt_trap`
//! (noreturn) with a `CString` message global. Enum layouts in the
//! meta keep per-variant reference offsets — scanning an enum value
//! depends on its tag (runtime spec 2.2).
//!
//! M5: arrays (docs/milestone5/DESIGN.md 2.4). Both array kinds map
//! onto `LirType::Array` — a pointer to `{ td, i64 size, inline
//! elements }`; mutability is compile-time only. The MIR array nodes
//! become the `ArrayAlloc` / `ArrayGet` / `ArrayLen` / `ArraySet` /
//! `ArrayClone` instructions (bounds checks and the clone's runtime
//! call are codegen's job). Every array type appearing in the module
//! gets a meta layout (after the tuple layouts) whose size / align are
//! element-level — the element stride and alignment of the region
//! after header + size — plus `element_is_ref`, which tells the M9 GC
//! whether to scan the whole element region as references.
//!
//! M6: reference types and dispatch (docs/milestone6/DESIGN.md 2.4).
//! `Class` / `Interface` / `Any` map onto `Ptr`. A `Virtual` call
//! loads the TypeDescriptor from the receiver's object header and the
//! vtable pointer from the TD, then `CallIndirect`s through it; an
//! `Interface` call gets its table from `scoop_rt_itable_lookup(td,
//! iface_td)` first. Class layouts (header + fields, reference
//! offsets relative to the object start) and the meta TypeDescriptors
//! (interfaces first — their symbols are itable keys — then classes
//! base-before-derived, so `parent` / interface references always
//! name already-emitted entries and codegen needs no forward
//! declarations) fill the LIR meta.
//!
//! Two instruction-level conventions make the M6 lowerings fit the
//! existing instruction set (DESIGN 2.4's "复用 GEP 类指令，实现时统一"):
//!
//! - `ExtractValue` with a `Ptr` aggregate operand is a heap object
//!   field load over the raw object struct `{ ptr header, fields...
//!   }`: index 0 reads the object header (the TypeDescriptor
//!   pointer), indices 1..=n read the fields. For TypeDescriptor
//!   pointers the indices follow the codegen-emitted
//!   `ScoopTypeDescriptor` struct (field 5 is the vtable pointer).
//!   This plays the role of DESIGN 2.4's `LoadField`. `HeapStore`
//!   (mir-lower's `FieldSet`, and the field initialization inside a
//!   `ClassInit` lowering) is the matching store with the same
//!   indexing.
//! - A TypeDescriptor operand is passed as `Value::Global` naming a
//!   global whose symbol is the TD's (`scoop_td_<name>`); lir-lower
//!   appends one such stub per referenced TD to the globals arena
//!   (its `CString("")` init is a placeholder). Codegen must skip the
//!   stubs when emitting data — the TD itself comes from
//!   `LirMeta::type_descriptors` — and resolve the operand by symbol.
//!   For `scoop_rt_box` codegen materializes the by-value aggregate
//!   payload behind a stack pointer (the "临时 alloca 取地址" of the
//!   lowering contract).
//!
//! A `mir::Expr::ClassInit` (only ever produced inside mir-lower's
//! generated ctor functions) is the raw construction primitive:
//! `scoop_rt_alloc(td, size)` plus one `HeapStore` per flattened
//! field. Use-site construction already became a plain ctor call in
//! MIR.

use std::collections::HashMap;

use la_arena::Arena;
use scoop_lir as lir;

/// Runtime object allocation: `ptr scoop_rt_alloc(ptr td, i64 size)`
/// (runtime spec 2.1). `ClassInit` lowerings call it.
const ALLOC_SYMBOL: &str = "scoop_rt_alloc";
use scoop_mir as mir;

/// Lower MIR to LIR.
pub fn lower(module: &mir::Module) -> lir::Module {
    // Every MIR string constant becomes a global with the same symbol.
    let mut globals = Arena::new();
    let mut global_map: HashMap<mir::StringConstId, lir::GlobalId> = HashMap::new();
    for (id, string) in module.strings.iter() {
        let global = globals.alloc(lir::Global {
            symbol: string.symbol.clone(),
            init: lir::GlobalInit::StringConst(string.value.clone()),
        });
        global_map.insert(id, global);
    }

    // Enum definitions with fixed representations, in the MIR arena's
    // order: `mir::EnumId` and `lir::EnumDefId` align.
    let enums = lower_enums(module);

    // Tuple types encountered while mapping value types, in
    // first-appearance order; each one gets a meta layout.
    let mut layout_types = Vec::new();
    // Trap message globals (`scoop.cstr.N`), numbered in creation order.
    let mut cstr_count = 0usize;
    // TypeDescriptor reference stubs (`scoop_td_*`), deduplicated by
    // symbol (see the module docs for the TD-reference convention).
    let mut td_map = HashMap::new();
    let functions = module
        .top_level
        .iter()
        .map(|&id| {
            lower_function(
                module,
                &module.functions[id],
                &global_map,
                &mut globals,
                &mut cstr_count,
                &mut layout_types,
                &enums,
                &mut td_map,
            )
        })
        .collect();

    let layouts = layouts(module, &enums, &layout_types);
    let type_descriptors = type_descriptors(module, &enums);
    lir::Module {
        globals,
        enums,
        functions,
        entry_symbol: module.functions[module.entry].symbol.clone(),
        meta: lir::LirMeta {
            layouts,
            type_descriptors,
        },
    }
}

/// The `lir::EnumDefId` of a MIR enum (the arenas are transposed 1:1).
fn enum_def_id(id: mir::EnumId) -> lir::EnumDefId {
    lir::EnumDefId::from_raw(id.into_raw())
}

/// Fix the representation of every MIR enum definition (spec 7.4).
fn lower_enums(module: &mir::Module) -> Arena<lir::EnumDef> {
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
        });
    }
    enums
}

/// Every enum type nested inside `ty` (through tuple elements and
/// struct fields), for representation sizing.
fn nested_enums(module: &mir::Module, ty: &mir::Type, out: &mut Vec<mir::EnumId>) {
    match ty {
        mir::Type::Enum(id, _) => out.push(*id),
        mir::Type::Tuple(elements) => {
            for element in elements {
                nested_enums(module, element, out);
            }
        }
        mir::Type::Struct(id) => {
            for field in &module.structs[*id].fields {
                nested_enums(module, &field.ty, out);
            }
        }
        mir::Type::Array(element) | mir::Type::MutableArray(element) => {
            nested_enums(module, element, out);
        }
        // References hide whatever they point at behind a pointer.
        mir::Type::Unit
        | mir::Type::Int
        | mir::Type::Boolean
        | mir::Type::String
        | mir::Type::Class(_)
        | mir::Type::Interface(_)
        | mir::Type::Any => {}
    }
}

/// Compute (memoized) the representation of one enum: the niche
/// pointer form when there are exactly two variants, one without
/// fields and the other with exactly one field mapping to `Ptr`;
/// otherwise the tagged form `{ i64 tag, [N x i8] payload }` with N
/// and alignment taken from the largest variant. Nested enums are
/// computed first because variant sizing needs their shapes.
fn compute_repr(module: &mir::Module, reprs: &mut Vec<Option<lir::EnumRepr>>, id: mir::EnumId) {
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

    // The niche check needs only the mapped field types, not sizes.
    if def.variants.len() == 2 {
        let has_unit = def.variants.iter().any(|v| v.fields.is_empty());
        let payload = def
            .variants
            .iter()
            .enumerate()
            .find(|(_, v)| !v.fields.is_empty());
        if let (true, Some((payload_index, payload_variant))) = (has_unit, payload) {
            if payload_variant.fields.len() == 1
                && lir_type(module, &payload_variant.fields[0].ty) == lir::LirType::Ptr
            {
                reprs[index] = Some(lir::EnumRepr::Niche {
                    payload_variant: payload_index as u32,
                });
                return;
            }
        }
    }

    // Tagged form: field types per variant, and the payload big
    // enough for the largest variant.
    let enum_shape = |id: mir::EnumId| {
        repr_shape(
            reprs[id.into_raw().into_u32() as usize]
                .as_ref()
                .expect("nested enum representations are computed first"),
        )
    };
    let mut payload_size = 0u64;
    let mut payload_align = 1u64;
    let mut variants = Vec::new();
    for variant in &def.variants {
        let fields: Vec<lir::LirType> = variant
            .fields
            .iter()
            .map(|field| lir_type(module, &field.ty))
            .collect();
        let field_types: Vec<mir::Type> = variant
            .fields
            .iter()
            .map(|field| field.ty.clone())
            .collect();
        let (_, size, align) = aggregate_shape(module, &enum_shape, &field_types);
        payload_size = payload_size.max(size);
        payload_align = payload_align.max(align);
        variants.push(fields);
    }
    reprs[index] = Some(lir::EnumRepr::Tagged {
        variants,
        payload_size,
        payload_align,
    });
}

/// Size and alignment of an enum value from its representation: the
/// niche form is a bare pointer; the tagged form is `{ i64 tag,
/// [N x i8] payload }` (8-byte tag, payload at offset 8).
fn repr_shape(repr: &lir::EnumRepr) -> (u64, u64) {
    match repr {
        lir::EnumRepr::Niche { .. } => (8, 8),
        lir::EnumRepr::Tagged {
            payload_size,
            payload_align,
            ..
        } => {
            let align = 8.max(*payload_align);
            ((8 + payload_size).next_multiple_of(align), align)
        }
    }
}

/// The meta layouts (DESIGN 2.4 / 3.4): the runtime `String` object
/// header, the `Int` / `Boolean` scalars, every struct in declaration
/// order, every enum in declaration order (with per-variant reference
/// offsets), every class in declaration order (M6: header + fields),
/// every tuple type that appears in the module, and every array type
/// that appears in the module (M5).
fn layouts(
    module: &mir::Module,
    enums: &Arena<lir::EnumDef>,
    from_code: &[mir::Type],
) -> Vec<lir::Layout> {
    // Tuple types reachable from struct / enum / class declarations
    // appear even when no code value mentions them directly.
    let mut types = Vec::new();
    for (_, def) in module.structs.iter() {
        for field in &def.fields {
            record_layout_types(&field.ty, &mut types);
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
        for field in &def.fields {
            record_layout_types(&field.ty, &mut types);
        }
    }
    for ty in from_code {
        record_layout_types(ty, &mut types);
    }

    let mut layouts = vec![
        string_layout(),
        scalar_layout("Int", 8, 8),
        scalar_layout("Boolean", 1, 1),
    ];
    for (_, def) in module.structs.iter() {
        let fields: Vec<mir::Type> = def.fields.iter().map(|field| field.ty.clone()).collect();
        layouts.push(aggregate_layout(module, enums, def.name.clone(), &fields));
    }
    for (id, def) in module.enums.iter() {
        layouts.push(enum_layout(module, enums, id, def));
    }
    for (_, def) in module.classes.iter() {
        let (size, align, refs) = class_layout(module, enums, def);
        layouts.push(lir::Layout {
            name: def.name.clone(),
            size,
            align,
            kind: lir::LayoutKind::Plain {
                ref_field_offsets: refs,
            },
        });
    }
    // Tuples first, then arrays (M5): the M4 layout order is kept.
    for ty in &types {
        if let mir::Type::Tuple(elements) = ty {
            layouts.push(aggregate_layout(
                module,
                enums,
                mir::type_name(module, ty),
                elements,
            ));
        }
    }
    for ty in &types {
        match ty {
            mir::Type::Tuple(_) => {}
            mir::Type::Array(element) | mir::Type::MutableArray(element) => {
                layouts.push(array_layout(module, enums, ty, element));
            }
            // `record_layout_types` only records tuples and arrays.
            _ => unreachable!("only tuple and array types get layouts"),
        }
    }
    layouts
}

/// The runtime `String` object layout (runtime spec 2.4): object
/// header (one pointer, 8 bytes) + `len` (u64, 8 bytes). The string
/// data is variable-length and not counted in `size`.
fn string_layout() -> lir::Layout {
    lir::Layout {
        name: "String".to_string(),
        size: 16,
        align: 8,
        kind: lir::LayoutKind::Plain {
            ref_field_offsets: Vec::new(),
        },
    }
}

fn scalar_layout(name: &str, size: u64, align: u64) -> lir::Layout {
    lir::Layout {
        name: name.to_string(),
        size,
        align,
        kind: lir::LayoutKind::Plain {
            ref_field_offsets: Vec::new(),
        },
    }
}

/// Layout of an aggregate value (struct / tuple / Unit): fields in
/// declaration order at their natural alignment. `ref_field_offsets`
/// lists the byte offset of every reference, including references
/// nested inside aggregate fields — the M9 GC scans exactly these
/// offsets.
fn aggregate_layout(
    module: &mir::Module,
    enums: &Arena<lir::EnumDef>,
    name: String,
    fields: &[mir::Type],
) -> lir::Layout {
    let enum_shape = |id: mir::EnumId| repr_shape(&enums[enum_def_id(id)].repr);
    let (offsets, size, align) = aggregate_shape(module, &enum_shape, fields);
    let mut refs = Vec::new();
    for (field, offset) in fields.iter().zip(offsets) {
        collect_ref_offsets(module, enums, field, offset, &mut refs);
    }
    lir::Layout {
        name,
        size,
        align,
        kind: lir::LayoutKind::Plain {
            ref_field_offsets: refs,
        },
    }
}

/// Layout of an enum value: niche form is a bare pointer (the value
/// *is* the reference, hence the payload variant's `refs=[0]`);
/// tagged form records, per variant, the absolute byte offsets of the
/// variant's references inside the value — the payload sits behind
/// the 8-byte tag, so payload offsets shift by 8.
fn enum_layout(
    module: &mir::Module,
    enums: &Arena<lir::EnumDef>,
    id: mir::EnumId,
    def: &mir::EnumDef,
) -> lir::Layout {
    match &enums[enum_def_id(id)].repr {
        lir::EnumRepr::Niche { .. } => lir::Layout {
            name: def.name.clone(),
            size: 8,
            align: 8,
            kind: lir::LayoutKind::Enum {
                variants: def
                    .variants
                    .iter()
                    .map(|variant| lir::VariantLayout {
                        ref_field_offsets: if variant.fields.is_empty() {
                            Vec::new()
                        } else {
                            vec![0]
                        },
                    })
                    .collect(),
            },
        },
        lir::EnumRepr::Tagged { .. } => {
            let (size, align) = repr_shape(&enums[enum_def_id(id)].repr);
            let enum_shape = |eid: mir::EnumId| repr_shape(&enums[enum_def_id(eid)].repr);
            let variants = def
                .variants
                .iter()
                .map(|variant| {
                    let field_types: Vec<mir::Type> = variant
                        .fields
                        .iter()
                        .map(|field| field.ty.clone())
                        .collect();
                    let (offsets, _, _) = aggregate_shape(module, &enum_shape, &field_types);
                    let mut refs = Vec::new();
                    for (field, offset) in field_types.iter().zip(offsets) {
                        // The payload sits behind the 8-byte tag.
                        collect_ref_offsets(module, enums, field, 8 + offset, &mut refs);
                    }
                    lir::VariantLayout {
                        ref_field_offsets: refs,
                    }
                })
                .collect();
            lir::Layout {
                name: def.name.clone(),
                size,
                align,
                kind: lir::LayoutKind::Enum { variants },
            }
        }
    }
}

/// Layout of an array object (M5, DESIGN 2.4): `{ td, i64 size,
/// inline elements }`. The object is variable-length, so `size` /
/// `align` here are element-level information: the element stride
/// (element size rounded up to its alignment) and alignment of the
/// element region that follows the 8-byte header + 8-byte size field.
/// `element_is_ref` tells the M9 GC whether the whole element region
/// is scanned as references.
fn array_layout(
    module: &mir::Module,
    enums: &Arena<lir::EnumDef>,
    ty: &mir::Type,
    element: &mir::Type,
) -> lir::Layout {
    let enum_shape = |id: mir::EnumId| repr_shape(&enums[enum_def_id(id)].repr);
    let (size, align) = size_align(module, &enum_shape, element);
    lir::Layout {
        name: mir::type_name(module, ty),
        size: size.next_multiple_of(align),
        align,
        kind: lir::LayoutKind::Array {
            element_is_ref: element_is_ref(enums, element),
        },
    }
}

/// Layout of a class object (M6, runtime spec 2.1/2.2): the 8-byte
/// object header (the TypeDescriptor pointer) followed by the fields
/// — mir-lower already flattened the base-class prefix into
/// `ClassDef::fields`. Returns size, align, and the reference offsets
/// relative to the object start (the header itself is not a scanned
/// reference). Boxed value types use the same shape: header + the
/// inline payload field.
fn class_layout(
    module: &mir::Module,
    enums: &Arena<lir::EnumDef>,
    def: &mir::ClassDef,
) -> (u64, u64, Vec<u64>) {
    let enum_shape = |id: mir::EnumId| repr_shape(&enums[enum_def_id(id)].repr);
    let mut offsets = Vec::with_capacity(def.fields.len());
    let mut size = 8u64;
    let mut align = 8u64;
    for field in &def.fields {
        let (field_size, field_align) = size_align(module, &enum_shape, &field.ty);
        let offset = size.next_multiple_of(field_align);
        offsets.push(offset);
        size = offset + field_size;
        align = align.max(field_align);
    }
    let size = size.next_multiple_of(align);
    let mut refs = Vec::new();
    for (field, offset) in def.fields.iter().zip(offsets) {
        collect_ref_offsets(module, enums, &field.ty, offset, &mut refs);
    }
    (size, align, refs)
}

/// Class ids ordered base-before-derived (single inheritance: depth
/// in the base chain; ties keep declaration order).
fn class_order(module: &mir::Module) -> Vec<mir::ClassId> {
    fn depth(module: &mir::Module, id: mir::ClassId) -> usize {
        match module.classes[id].base_class {
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
fn td_symbol(name: &str) -> String {
    format!("scoop_td_{name}")
}

/// The symbol a vtable / itable slot points at: a module function
/// (user methods, generated equals / thunks) or a runtime function
/// (the `Any` defaults).
fn slot_symbol(module: &mir::Module, slot: &mir::TableSlot) -> String {
    match slot {
        mir::TableSlot::Function(id) => module.functions[*id].symbol.clone(),
        mir::TableSlot::Runtime(function) => function.symbol().to_string(),
    }
}

/// The meta TypeDescriptors (runtime spec 2.2, milestone6 DESIGN
/// 2.4): interfaces first — their symbols are the itable keys — then
/// classes (including the boxed value types) base-before-derived, so
/// `parent` / interface references always name already-emitted
/// entries and codegen needs no forward declarations.
fn type_descriptors(module: &mir::Module, enums: &Arena<lir::EnumDef>) -> Vec<lir::TypeDescriptor> {
    let mut tds = Vec::new();
    for (_, def) in module.interfaces.iter() {
        tds.push(lir::TypeDescriptor {
            name: def.name.clone(),
            symbol: td_symbol(&def.name),
            size: 0,
            align: 0,
            ref_offsets: Vec::new(),
            parent: None,
            vtable: Vec::new(),
            itables: Vec::new(),
        });
    }
    for id in class_order(module) {
        let def = &module.classes[id];
        let (size, align, refs) = class_layout(module, enums, def);
        tds.push(lir::TypeDescriptor {
            name: def.name.clone(),
            symbol: td_symbol(&def.name),
            size,
            align,
            ref_offsets: refs,
            parent: def
                .base_class
                .map(|base| td_symbol(&module.classes[base].name)),
            vtable: def
                .vtable
                .iter()
                .map(|slot| slot_symbol(module, slot))
                .collect(),
            itables: def
                .itables
                .iter()
                .map(|record| lir::ItableRecord {
                    interface_symbol: td_symbol(&module.interfaces[record.interface].name),
                    slots: record
                        .slots
                        .iter()
                        .map(|slot| slot_symbol(module, slot))
                        .collect(),
                })
                .collect(),
        });
    }
    tds
}

/// Whether an array element is a reference: elements mapping to `Ptr`
/// (String, classes, interfaces, `Any`, and arrays themselves) and
/// niche-form enums (a bare pointer) are; scalars and inline
/// aggregates are not. Tagged enum elements keep their references per
/// variant — the flat flag cannot express them, so they count as
/// non-reference here (the boundary recorded in M4, DESIGN 6).
fn element_is_ref(enums: &Arena<lir::EnumDef>, element: &mir::Type) -> bool {
    match element {
        mir::Type::String
        | mir::Type::Class(_)
        | mir::Type::Interface(_)
        | mir::Type::Any
        | mir::Type::Array(_)
        | mir::Type::MutableArray(_) => true,
        mir::Type::Enum(id, _) => {
            matches!(enums[enum_def_id(*id)].repr, lir::EnumRepr::Niche { .. })
        }
        mir::Type::Unit
        | mir::Type::Int
        | mir::Type::Boolean
        | mir::Type::Struct(_)
        | mir::Type::Tuple(_) => false,
    }
}

/// Field offsets plus total size and alignment of an aggregate with
/// the given field types: each field sits at the next offset aligned
/// to its own alignment, and the size is rounded up to the aggregate
/// alignment (natural layout, as for LLVM literal structs). Enum
/// shapes come from `enum_shape`, so the same code serves both the
/// representation-computation phase and completed modules.
fn aggregate_shape(
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

/// Size and alignment of a value of type `ty`. `String` and the M6
/// reference types are pointers (pointer-sized); aggregates recurse;
/// enums take their representation's shape.
fn size_align(
    module: &mir::Module,
    enum_shape: &dyn Fn(mir::EnumId) -> (u64, u64),
    ty: &mir::Type,
) -> (u64, u64) {
    match ty {
        mir::Type::Unit => (0, 1),
        mir::Type::Int => (8, 8),
        mir::Type::Boolean => (1, 1),
        mir::Type::String | mir::Type::Class(_) | mir::Type::Interface(_) | mir::Type::Any => {
            (8, 8)
        }
        mir::Type::Struct(id) => {
            let fields: Vec<mir::Type> = module.structs[*id]
                .fields
                .iter()
                .map(|field| field.ty.clone())
                .collect();
            let (_, size, align) = aggregate_shape(module, enum_shape, &fields);
            (size, align)
        }
        mir::Type::Tuple(elements) => {
            let (_, size, align) = aggregate_shape(module, enum_shape, elements);
            (size, align)
        }
        // An array value is a pointer to the array object.
        mir::Type::Array(_) | mir::Type::MutableArray(_) => (8, 8),
        mir::Type::Enum(id, _) => enum_shape(*id),
    }
}

/// Byte offsets (relative to `base`) of every reference inside a value
/// of type `ty`, recursing into aggregate fields.
fn collect_ref_offsets(
    module: &mir::Module,
    enums: &Arena<lir::EnumDef>,
    ty: &mir::Type,
    base: u64,
    offsets: &mut Vec<u64>,
) {
    let fields: Vec<mir::Type> = match ty {
        mir::Type::String | mir::Type::Class(_) | mir::Type::Interface(_) | mir::Type::Any => {
            // A reference: the value itself is the pointer (for
            // classes / interfaces / Any the pointee's references are
            // the class layout's business).
            offsets.push(base);
            return;
        }
        // An array value is itself a reference to the array object
        // (the elements' references are the array layout's business).
        mir::Type::Array(_) | mir::Type::MutableArray(_) => {
            offsets.push(base);
            return;
        }
        mir::Type::Struct(id) => module.structs[*id]
            .fields
            .iter()
            .map(|field| field.ty.clone())
            .collect(),
        mir::Type::Tuple(elements) => elements.clone(),
        mir::Type::Enum(id, _) => match &enums[enum_def_id(*id)].repr {
            // Niche form: the value itself is the reference (or null).
            lir::EnumRepr::Niche { .. } => {
                offsets.push(base);
                return;
            }
            // Tagged form: the reference offsets depend on the tag and
            // live in the enum's own `LayoutKind::Enum` entry; a flat
            // per-aggregate offset list cannot express them (runtime
            // spec 2.2, M9).
            lir::EnumRepr::Tagged { .. } => return,
        },
        // Scalars contain no references.
        mir::Type::Unit | mir::Type::Int | mir::Type::Boolean => return,
    };
    let enum_shape = |id: mir::EnumId| repr_shape(&enums[enum_def_id(id)].repr);
    let (field_offsets, _, _) = aggregate_shape(module, &enum_shape, &fields);
    for (field, offset) in fields.iter().zip(field_offsets) {
        collect_ref_offsets(module, enums, field, base + offset, offsets);
    }
}

/// Record every tuple and array type reachable from `ty`
/// (first-appearance order, duplicates skipped) so each gets a meta
/// layout. Structs and enums are covered by their own
/// declaration-driven layout sections.
fn record_layout_types(ty: &mir::Type, types: &mut Vec<mir::Type>) {
    match ty {
        mir::Type::Tuple(elements) => {
            if !types.contains(ty) {
                types.push(ty.clone());
            }
            for element in elements {
                record_layout_types(element, types);
            }
        }
        mir::Type::Array(element) | mir::Type::MutableArray(element) => {
            if !types.contains(ty) {
                types.push(ty.clone());
            }
            record_layout_types(element, types);
        }
        _ => {}
    }
}

/// Map a MIR type onto its LIR value type (DESIGN 2.4 / 3.4): Unit is
/// the empty aggregate, String and the M6 reference types pointers,
/// struct / tuple literal aggregates of their mapped fields, and
/// enums `LirType::Enum` — their representation lives in the
/// `EnumDef`, so the mapping is the identity on enum ids. Both array
/// kinds map onto `LirType::Array` (a pointer to the array object;
/// the payload is the element layout).
fn lir_type(module: &mir::Module, ty: &mir::Type) -> lir::LirType {
    match ty {
        mir::Type::Unit => lir::LirType::Aggregate(Vec::new()),
        mir::Type::Int => lir::LirType::I64,
        mir::Type::Boolean => lir::LirType::I1,
        mir::Type::String | mir::Type::Class(_) | mir::Type::Interface(_) | mir::Type::Any => {
            lir::LirType::Ptr
        }
        mir::Type::Array(element) | mir::Type::MutableArray(element) => {
            lir::LirType::Array(Box::new(lir_type(module, element)))
        }
        mir::Type::Struct(id) => lir::LirType::Aggregate(
            module.structs[*id]
                .fields
                .iter()
                .map(|field| lir_type(module, &field.ty))
                .collect(),
        ),
        mir::Type::Tuple(elements) => {
            lir::LirType::Aggregate(elements.iter().map(|e| lir_type(module, e)).collect())
        }
        mir::Type::Enum(id, _) => lir::LirType::Enum(enum_def_id(*id)),
    }
}

/// Map a primitive MIR binary operator onto its LIR operator, result
/// type, and operand type (aggregate equality has been expanded away
/// in MIR).
fn binary_op(op: mir::BinOp) -> (lir::BinOp, lir::LirType, mir::Type) {
    use lir::LirType::*;
    use mir::Type::*;
    match op {
        mir::BinOp::IntAdd => (lir::BinOp::Add, I64, Int),
        mir::BinOp::IntSub => (lir::BinOp::Sub, I64, Int),
        mir::BinOp::IntMul => (lir::BinOp::Mul, I64, Int),
        mir::BinOp::IntDiv => (lir::BinOp::SDiv, I64, Int),
        mir::BinOp::IntLt => (lir::BinOp::Lt, I1, Int),
        mir::BinOp::IntLe => (lir::BinOp::Le, I1, Int),
        mir::BinOp::IntGt => (lir::BinOp::Gt, I1, Int),
        mir::BinOp::IntGe => (lir::BinOp::Ge, I1, Int),
        mir::BinOp::IntEq => (lir::BinOp::Eq, I1, Int),
        mir::BinOp::IntNe => (lir::BinOp::Ne, I1, Int),
        mir::BinOp::BoolEq => (lir::BinOp::Eq, I1, Boolean),
        mir::BinOp::BoolNe => (lir::BinOp::Ne, I1, Boolean),
        // Handled by the caller as short-circuit branches.
        mir::BinOp::And | mir::BinOp::Or => {
            unreachable!("`&&` / `||` are lowered by short-circuit expansion")
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn lower_function(
    module: &mir::Module,
    function: &mir::Function,
    global_map: &HashMap<mir::StringConstId, lir::GlobalId>,
    globals: &mut Arena<lir::Global>,
    cstr_count: &mut usize,
    layout_types: &mut Vec<mir::Type>,
    enums: &Arena<lir::EnumDef>,
    td_map: &mut HashMap<String, lir::GlobalId>,
) -> lir::Function {
    // Parameters are SSA values (`Value::Param`), not stack slots;
    // they are immutable (M3), so no store ever targets them.
    let mut local_map = HashMap::new();
    let params: Vec<lir::LirType> = function
        .params
        .iter()
        .enumerate()
        .map(|(index, param)| {
            record_layout_types(&param.ty, layout_types);
            local_map.insert(param.local, LocalSlot::Param(index as u32));
            lir_type(module, &param.ty)
        })
        .collect();

    // One LIR stack slot per non-parameter MIR local, in declaration
    // order.
    let mut locals = Arena::new();
    for (mir_id, local) in function.body.locals.iter() {
        if local_map.contains_key(&mir_id) {
            continue; // a parameter
        }
        record_layout_types(&local.ty, layout_types);
        let lir_id = locals.alloc(lir::Local {
            name: local.name.clone(),
            ty: lir_type(module, &local.ty),
        });
        local_map.insert(mir_id, LocalSlot::Slot(lir_id));
    }

    // Unit-returning functions are void at the LLVM level (DESIGN 2.4).
    record_layout_types(&function.return_ty, layout_types);
    let returns_void = function.return_ty == mir::Type::Unit;
    let return_ty = if returns_void {
        lir::LirType::Void
    } else {
        lir_type(module, &function.return_ty)
    };

    let mut blocks = Arena::new();
    let entry = blocks.alloc(lir::BasicBlock {
        name: "entry".to_string(),
        instructions: Vec::new(),
        terminator: lir::Terminator::Return { value: None }, // placeholder, see FunctionLowerer
    });
    let mut lowerer = FunctionLowerer {
        module,
        mir_locals: &function.body.locals,
        global_map,
        globals,
        cstr_count,
        layout_types,
        enums,
        td_map,
        local_map,
        locals,
        temps: Arena::new(),
        blocks,
        current: entry,
        block_count: 0,
        hidden_count: 0,
        mir_return_ty: function.return_ty.clone(),
        returns_void,
        trap_blocks: HashMap::new(),
        current_sealed: false,
    };
    lowerer.lower_statements(&function.body.statements);
    if !lowerer.current_sealed {
        // Unit functions fall off the end with a bare return; non-Unit
        // functions always end in `return` (hir-lower enforces it,
        // DESIGN 1).
        assert!(
            returns_void,
            "hir-lower requires non-Unit functions to end with `return`"
        );
        lowerer.seal(lir::Terminator::Return { value: None });
    }
    lir::Function {
        symbol: function.symbol.clone(),
        params,
        return_ty,
        locals: lowerer.locals,
        temps: lowerer.temps,
        blocks: lowerer.blocks,
        entry,
    }
}

/// Where a MIR local lives in LIR: parameters are SSA values, all
/// other locals get stack slots.
#[derive(Clone, Copy)]
enum LocalSlot {
    Slot(lir::LocalId),
    Param(u32),
}

/// The element type of an array type. `Array` and `MutableArray`
/// share the object layout; mutability is compile-time only.
fn array_element(ty: &mir::Type) -> &mir::Type {
    match ty {
        mir::Type::Array(element) | mir::Type::MutableArray(element) => element,
        _ => unreachable!("expected an array type"),
    }
}

/// The global symbol of the TypeDescriptor a runtime check / box
/// refers to: classes and interfaces have their own; value types are
/// compared / boxed through their boxed class's (`box$<encoded>`, as
/// mir-lower names it); String keeps its built-in TD. `Any` has no TD
/// (mir-lower folds those checks), and array TDs are assigned by
/// codegen (`scoop_td_array.N`), so neither is referenceable here.
fn td_symbol_for(module: &mir::Module, ty: &mir::Type) -> String {
    match ty {
        mir::Type::Class(id) => td_symbol(&module.classes[*id].name),
        mir::Type::Interface(id) => td_symbol(&module.interfaces[*id].name),
        mir::Type::String => lir::STRING_TD_SYMBOL.to_string(),
        mir::Type::Struct(_)
        | mir::Type::Enum(..)
        | mir::Type::Tuple(_)
        | mir::Type::Int
        | mir::Type::Boolean
        | mir::Type::Unit => td_symbol(&format!("box${}", mir::encode_type(module, ty))),
        mir::Type::Any | mir::Type::Array(_) | mir::Type::MutableArray(_) => {
            unreachable!("no referenceable TypeDescriptor for {ty:?}")
        }
    }
}

/// Per-function lowering state: locals, temps, and the basic blocks
/// built so far. Invariant: the `current` block is always unsealed
/// (its terminator is a placeholder); a block is sealed exactly when
/// control flow leaves it. `current_sealed` tracks whether the current
/// block was already sealed (by a `return`, or by a trap call), so
/// structured control flow does not seal it again with a branch.
struct FunctionLowerer<'a> {
    module: &'a mir::Module,
    /// Locals of the MIR function being lowered (for `expr_ty`).
    mir_locals: &'a Arena<mir::Local>,
    global_map: &'a HashMap<mir::StringConstId, lir::GlobalId>,
    /// Sink for trap message globals (`scoop.cstr.N`) and
    /// TypeDescriptor reference stubs (`scoop_td_*`).
    globals: &'a mut Arena<lir::Global>,
    cstr_count: &'a mut usize,
    /// Sink for tuple types encountered in value types (meta layouts).
    layout_types: &'a mut Vec<mir::Type>,
    /// Enum definitions with fixed representations (enum value
    /// sizing, e.g. for `scoop_rt_box` payload sizes).
    enums: &'a Arena<lir::EnumDef>,
    /// TypeDescriptor reference stubs, deduplicated by symbol.
    td_map: &'a mut HashMap<String, lir::GlobalId>,
    local_map: HashMap<mir::LocalId, LocalSlot>,
    locals: Arena<lir::Local>,
    temps: Arena<lir::Temp>,
    blocks: Arena<lir::BasicBlock>,
    current: lir::BlockId,
    /// Counters for unique block / hidden-local names.
    block_count: usize,
    hidden_count: usize,
    /// The function's MIR return type (`Unit` ⇒ void at LLVM level).
    mir_return_ty: mir::Type,
    returns_void: bool,
    /// The shared trap blocks of this function, one per message,
    /// created on first use.
    trap_blocks: HashMap<String, lir::BlockId>,
    /// Whether the current block was already sealed.
    current_sealed: bool,
}

impl FunctionLowerer<'_> {
    fn new_block(&mut self, base: &str) -> lir::BlockId {
        self.block_count += 1;
        self.blocks.alloc(lir::BasicBlock {
            name: format!("{base}.{}", self.block_count),
            instructions: Vec::new(),
            terminator: lir::Terminator::Return { value: None }, // placeholder, see struct docs
        })
    }

    /// Seal the current block with its terminator.
    fn seal(&mut self, terminator: lir::Terminator) {
        self.blocks[self.current].terminator = terminator;
    }

    /// Make `block` the current (unsealed) block.
    fn enter(&mut self, block: lir::BlockId) {
        self.current = block;
        self.current_sealed = false;
    }

    fn push(&mut self, instruction: lir::Instruction) {
        self.blocks[self.current].instructions.push(instruction);
    }

    fn new_temp(&mut self, ty: lir::LirType) -> lir::TempId {
        self.temps.alloc(lir::Temp { ty })
    }

    /// A fresh hidden slot carrying a short-circuit result across
    /// basic blocks (LIR has no phi nodes; mem2reg removes it).
    fn new_hidden_local(&mut self, ty: lir::LirType) -> lir::LocalId {
        self.hidden_count += 1;
        self.locals.alloc(lir::Local {
            name: format!("$sc.{}", self.hidden_count),
            ty,
        })
    }

    /// The value of a MIR local: a stack slot load, or the SSA
    /// parameter itself.
    fn local_value(&self, local: mir::LocalId) -> lir::Value {
        match self.local_map[&local] {
            LocalSlot::Slot(id) => lir::Value::Local(id),
            LocalSlot::Param(index) => lir::Value::Param(index),
        }
    }

    /// The stack slot of a MIR local that is stored to. Parameters are
    /// immutable (M3), so stores never target them.
    fn local_slot(&self, local: mir::LocalId) -> lir::LocalId {
        match self.local_map[&local] {
            LocalSlot::Slot(id) => id,
            LocalSlot::Param(_) => {
                unreachable!("parameters are immutable; stores never target them")
            }
        }
    }

    /// The LIR value type of a MIR type; tuple types are recorded for
    /// the meta layouts on the way.
    fn value_type(&mut self, ty: &mir::Type) -> lir::LirType {
        record_layout_types(ty, self.layout_types);
        lir_type(self.module, ty)
    }

    /// The MIR type of an expression (MIR expressions don't carry
    /// types, so they are reconstructed from locals and struct / enum
    /// defs). `VariantConstruct` is the one MIR expression that does
    /// carry its type.
    fn expr_ty(&self, expr: &mir::Expr) -> mir::Type {
        match expr {
            mir::Expr::StringConst(_) => mir::Type::String,
            mir::Expr::IntLiteral(_) => mir::Type::Int,
            mir::Expr::BoolLiteral(_) => mir::Type::Boolean,
            mir::Expr::UnitLiteral => mir::Type::Unit,
            mir::Expr::TupleLiteral(elements) => {
                mir::Type::Tuple(elements.iter().map(|e| self.expr_ty(e)).collect())
            }
            mir::Expr::StructInit { struct_id, .. } => mir::Type::Struct(*struct_id),
            mir::Expr::ArrayLiteral(elements) => {
                // The literal's kind (Array vs MutableArray) is not
                // recorded on the node and does not matter here: both
                // map onto the same LIR type. Empty literals only
                // appear where the context supplies the type
                // (hir-lower rejects `[]` without one), and `expr_ty`
                // is never queried on those paths.
                let element = elements
                    .first()
                    .expect("empty array literals only appear with an expected type");
                mir::Type::Array(Box::new(self.expr_ty(element)))
            }
            mir::Expr::ArrayGet { array, .. } => array_element(&self.expr_ty(array)).clone(),
            mir::Expr::ArrayLen(_) => mir::Type::Int,
            mir::Expr::ArrayClone(operand) => match self.expr_ty(operand) {
                // The conversion produces the other array kind with the
                // same element type (spec 10.4).
                mir::Type::Array(element) => mir::Type::MutableArray(element),
                mir::Type::MutableArray(element) => mir::Type::Array(element),
                _ => unreachable!("an array conversion's operand is an array"),
            },
            mir::Expr::Local(local) => self.mir_locals[*local].ty.clone(),
            mir::Expr::FieldAccess { receiver, index } => match self.expr_ty(receiver) {
                mir::Type::Struct(id) => self.module.structs[id].fields[*index as usize].ty.clone(),
                mir::Type::Tuple(elements) => elements[*index as usize].clone(),
                mir::Type::Class(id) => self.module.classes[id].fields[*index as usize].ty.clone(),
                // mir-lower only emits field accesses on aggregates
                // and class objects.
                _ => unreachable!("field access on a non-aggregate"),
            },
            mir::Expr::VariantConstruct { ty, .. } => ty.clone(),
            mir::Expr::ClassInit { class_id, .. } => mir::Type::Class(*class_id),
            mir::Expr::EnumTag(_) => mir::Type::Int,
            mir::Expr::EnumField {
                operand,
                variant,
                index,
            } => match self.expr_ty(operand) {
                mir::Type::Enum(id, _) => self.module.enums[id].variants[*variant as usize].fields
                    [*index as usize]
                    .ty
                    .clone(),
                _ => unreachable!("enum field access on a non-enum"),
            },
            mir::Expr::Call(call) => match call.target.callee {
                mir::Callee::User(id) => self.module.functions[id].return_ty.clone(),
                mir::Callee::Runtime(function) => match function {
                    mir::RuntimeFn::StringConcat => mir::Type::String,
                    mir::RuntimeFn::StringEq => mir::Type::Boolean,
                    mir::RuntimeFn::Box | mir::RuntimeFn::ITableLookup => mir::Type::Any,
                    mir::RuntimeFn::IsInstance | mir::RuntimeFn::AnyEquals => mir::Type::Boolean,
                    mir::RuntimeFn::AnyHashCode => mir::Type::Int,
                    mir::RuntimeFn::AnyToString => mir::Type::String,
                    mir::RuntimeFn::IntToString | mir::RuntimeFn::BoolToString => {
                        mir::Type::String
                    }
                    mir::RuntimeFn::Write
                    // The trap is noreturn; its statement is typed Unit.
                    | mir::RuntimeFn::Trap => mir::Type::Unit,
                },
            },
            // A `Box` result is a reference (`Any` or an interface;
            // both are `Ptr` at this level).
            mir::Expr::Box(_) => mir::Type::Any,
            // mir-lower routes `Unbox` results through typed locals /
            // returns / call arguments, so the type always comes from
            // the context; it is never reconstructed here.
            mir::Expr::Unbox(_) => {
                unreachable!("an Unbox result's type comes from the enclosing context")
            }
            mir::Expr::IsInstance { .. } => mir::Type::Boolean,
            // mir-lower expands `as` / `as?` into runtime checks plus
            // Option wrapping; the node never reaches LIR.
            mir::Expr::Cast { .. } => {
                unreachable!("mir-lower expands casts before LIR")
            }
            mir::Expr::Binary { op, .. } => match op {
                mir::BinOp::IntAdd
                | mir::BinOp::IntSub
                | mir::BinOp::IntMul
                | mir::BinOp::IntDiv => mir::Type::Int,
                mir::BinOp::IntLt
                | mir::BinOp::IntLe
                | mir::BinOp::IntGt
                | mir::BinOp::IntGe
                | mir::BinOp::IntEq
                | mir::BinOp::IntNe
                | mir::BinOp::BoolEq
                | mir::BinOp::BoolNe
                | mir::BinOp::And
                | mir::BinOp::Or => mir::Type::Boolean,
            },
            mir::Expr::Unary { op, .. } => match op {
                mir::UnOp::IntNeg => mir::Type::Int,
                mir::UnOp::BoolNot => mir::Type::Boolean,
            },
        }
    }

    fn lower_statements(&mut self, statements: &[mir::Statement]) {
        for statement in statements {
            self.lower_statement(statement);
        }
    }

    fn lower_statement(&mut self, statement: &mir::Statement) {
        match &statement.kind {
            mir::StatementKind::Expr(expr) => {
                let ty = self.expr_ty(expr);
                self.lower_expr(expr, &ty);
            }
            // Initialization and assignment are both stores into the
            // local's stack slot.
            mir::StatementKind::ValDecl { local, init } => {
                let ty = self.mir_locals[*local].ty.clone();
                let value = self.lower_expr(init, &ty);
                self.push(lir::Instruction::Store {
                    local: self.local_slot(*local),
                    value,
                });
            }
            mir::StatementKind::Assign { local, value } => {
                let ty = self.mir_locals[*local].ty.clone();
                let value = self.lower_expr(value, &ty);
                self.push(lir::Instruction::Store {
                    local: self.local_slot(*local),
                    value,
                });
            }
            // `m[i] = v`: bounds check and the element store are
            // codegen's job; the element layout comes from the array
            // operand's type.
            mir::StatementKind::ArraySet {
                array,
                index,
                value,
            } => {
                let array_ty = self.expr_ty(array);
                record_layout_types(&array_ty, self.layout_types);
                let element_ty = array_element(&array_ty).clone();
                let array = self.lower_expr(array, &array_ty);
                let index = self.lower_expr(index, &mir::Type::Int);
                let value = self.lower_expr(value, &element_ty);
                self.push(lir::Instruction::ArraySet {
                    array,
                    index,
                    value,
                });
            }
            // `obj.field = value`: a heap field store. The MIR index
            // is the flattened field index; the raw object struct has
            // the header at index 0, so the field sits at index + 1
            // (the same indexing as `ExtractValue` on a pointer).
            mir::StatementKind::FieldSet {
                object,
                index,
                value,
            } => {
                let object_ty = self.expr_ty(object);
                let mir::Type::Class(class_id) = &object_ty else {
                    unreachable!("a field store targets a class object")
                };
                let field_ty = self.module.classes[*class_id].fields[*index as usize]
                    .ty
                    .clone();
                let object = self.lower_expr(object, &object_ty);
                let value = self.lower_expr(value, &field_ty);
                self.push(lir::Instruction::HeapStore {
                    object,
                    index: index + 1,
                    value,
                });
            }
            mir::StatementKind::Return { value } => {
                let value = match (self.returns_void, value) {
                    (true, None) => None,
                    // hir-lower emits bare `return` in Unit functions,
                    // so this is defensive: evaluate the Unit value for
                    // its instructions, but the void function does not
                    // return it.
                    (true, Some(value)) => {
                        self.lower_expr(value, &mir::Type::Unit);
                        None
                    }
                    (false, Some(value)) => {
                        let ty = self.mir_return_ty.clone();
                        Some(self.lower_expr(value, &ty))
                    }
                    // hir-lower: non-Unit functions return a value.
                    (false, None) => unreachable!("non-Unit `return` without a value"),
                };
                self.seal(lir::Terminator::Return { value });
                self.current_sealed = true;
            }
            mir::StatementKind::If {
                cond,
                then_body,
                else_body,
            } => {
                let cond = self.lower_expr(cond, &mir::Type::Boolean);
                let then_block = self.new_block("if.then");
                let else_block = else_body.as_ref().map(|_| self.new_block("if.else"));
                let merge_block = self.new_block("if.merge");
                self.seal(lir::Terminator::CondBr {
                    cond,
                    then_block,
                    else_block: else_block.unwrap_or(merge_block),
                });
                self.enter(then_block);
                self.lower_statements(then_body);
                // A branch ending in `return` / a trap is sealed
                // already.
                if !self.current_sealed {
                    self.seal(lir::Terminator::Br(merge_block));
                }
                if let (Some(else_body), Some(else_block)) = (else_body, else_block) {
                    self.enter(else_block);
                    self.lower_statements(else_body);
                    if !self.current_sealed {
                        self.seal(lir::Terminator::Br(merge_block));
                    }
                }
                self.enter(merge_block);
            }
            mir::StatementKind::While { cond, body } => {
                let cond_block = self.new_block("while.cond");
                self.seal(lir::Terminator::Br(cond_block));
                self.enter(cond_block);
                let cond = self.lower_expr(cond, &mir::Type::Boolean);
                let body_block = self.new_block("while.body");
                let exit_block = self.new_block("while.exit");
                self.seal(lir::Terminator::CondBr {
                    cond,
                    then_block: body_block,
                    else_block: exit_block,
                });
                self.enter(body_block);
                self.lower_statements(body);
                if !self.current_sealed {
                    self.seal(lir::Terminator::Br(cond_block));
                }
                self.enter(exit_block);
            }
        }
    }

    /// Lower an expression of MIR type `ty`, appending its
    /// instructions to the current block, and return the value it
    /// evaluates to. The type comes from the context (the local's
    /// declared type, the callee's parameter type, the enclosing
    /// aggregate's field type, ...); MIR expressions carry none, and
    /// `VariantConstruct` carries its own.
    fn lower_expr(&mut self, expr: &mir::Expr, ty: &mir::Type) -> lir::Value {
        match expr {
            mir::Expr::StringConst(id) => lir::Value::Global(self.global_map[id]),
            mir::Expr::IntLiteral(value) => lir::Value::IntConst(*value),
            mir::Expr::BoolLiteral(value) => lir::Value::BoolConst(*value),
            mir::Expr::UnitLiteral => self.unit_value(),
            mir::Expr::TupleLiteral(elements) => {
                let mir::Type::Tuple(element_types) = ty else {
                    unreachable!("a tuple literal has a tuple type")
                };
                let element_types = element_types.clone();
                let elements: Vec<lir::Value> = elements
                    .iter()
                    .zip(&element_types)
                    .map(|(element, ty)| self.lower_expr(element, ty))
                    .collect();
                self.make_aggregate(ty, elements)
            }
            mir::Expr::StructInit { struct_id, args } => {
                let field_types: Vec<mir::Type> = self.module.structs[*struct_id]
                    .fields
                    .iter()
                    .map(|field| field.ty.clone())
                    .collect();
                let args: Vec<lir::Value> = args
                    .iter()
                    .zip(&field_types)
                    .map(|(arg, ty)| self.lower_expr(arg, ty))
                    .collect();
                self.make_aggregate(ty, args)
            }
            // Raw class construction (only ever inside mir-lower's
            // generated ctor functions): `scoop_rt_alloc(td, size)`,
            // then one heap store per flattened field.
            mir::Expr::ClassInit { class_id, args } => {
                let def = &self.module.classes[*class_id];
                let (size, _, _) = class_layout(self.module, self.enums, def);
                let field_types: Vec<mir::Type> =
                    def.fields.iter().map(|field| field.ty.clone()).collect();
                assert_eq!(
                    args.len(),
                    field_types.len(),
                    "a ClassInit initializes every flattened field"
                );
                let td = self.td_ref(&mir::Type::Class(*class_id));
                let out = self.new_temp(lir::LirType::Ptr);
                self.push(lir::Instruction::Call {
                    out: Some(out),
                    symbol: ALLOC_SYMBOL.to_string(),
                    args: vec![td, lir::Value::IntConst(size as i64)],
                });
                for (index, (arg, field_ty)) in args.iter().zip(&field_types).enumerate() {
                    let value = self.lower_expr(arg, field_ty);
                    self.push(lir::Instruction::HeapStore {
                        object: lir::Value::Temp(out),
                        index: index as u32 + 1,
                        value,
                    });
                }
                lir::Value::Temp(out)
            }
            // The array operations map onto the corresponding LIR
            // instructions (DESIGN 2.4); the element layout is the
            // `Array(...)` type of the array operand (or of `out` for
            // the alloc), and bounds checks plus the clone's runtime
            // call are codegen's job.
            mir::Expr::ArrayLiteral(elements) => {
                let element_ty = array_element(ty).clone();
                let elements: Vec<lir::Value> = elements
                    .iter()
                    .map(|element| self.lower_expr(element, &element_ty))
                    .collect();
                let out_ty = self.value_type(ty);
                let out = self.new_temp(out_ty);
                self.push(lir::Instruction::ArrayAlloc { out, elements });
                lir::Value::Temp(out)
            }
            mir::Expr::ArrayGet { array, index } => {
                let array_ty = self.expr_ty(array);
                record_layout_types(&array_ty, self.layout_types);
                let array = self.lower_expr(array, &array_ty);
                let index = self.lower_expr(index, &mir::Type::Int);
                let out_ty = self.value_type(ty);
                let out = self.new_temp(out_ty);
                self.push(lir::Instruction::ArrayGet { out, array, index });
                lir::Value::Temp(out)
            }
            mir::Expr::ArrayLen(operand) => {
                let operand_ty = self.expr_ty(operand);
                record_layout_types(&operand_ty, self.layout_types);
                let operand = self.lower_expr(operand, &operand_ty);
                let out = self.new_temp(lir::LirType::I64);
                self.push(lir::Instruction::ArrayLen { out, operand });
                lir::Value::Temp(out)
            }
            mir::Expr::ArrayClone(operand) => {
                let operand_ty = self.expr_ty(operand);
                record_layout_types(&operand_ty, self.layout_types);
                let operand = self.lower_expr(operand, &operand_ty);
                let out_ty = self.value_type(ty);
                let out = self.new_temp(out_ty);
                self.push(lir::Instruction::ArrayClone { out, operand });
                lir::Value::Temp(out)
            }
            mir::Expr::Local(local) => self.local_value(*local),
            mir::Expr::FieldAccess { receiver, index } => {
                let receiver_ty = self.expr_ty(receiver);
                let aggregate = self.lower_expr(receiver, &receiver_ty);
                let ty = self.value_type(ty);
                let out = self.new_temp(ty);
                // A class field is a heap object load: index into the
                // raw object struct `{ ptr header, fields... }` (see
                // the module docs), so the field sits at index + 1.
                // Aggregate receivers extract from the SSA value.
                let index = if matches!(receiver_ty, mir::Type::Class(_)) {
                    index + 1
                } else {
                    *index
                };
                self.push(lir::Instruction::ExtractValue {
                    out,
                    aggregate,
                    index,
                });
                lir::Value::Temp(out)
            }
            // `scoop_rt_box(td, payload, size)` (runtime spec 2.3):
            // the td and the size come from the payload's static
            // type; codegen materializes the by-value aggregate
            // payload behind a stack pointer (module docs).
            mir::Expr::Box(operand) => {
                let payload_ty = self.expr_ty(operand);
                record_layout_types(&payload_ty, self.layout_types);
                let payload = self.lower_expr(operand, &payload_ty);
                let td = self.td_ref(&payload_ty);
                let enum_shape = |id: mir::EnumId| repr_shape(&self.enums[enum_def_id(id)].repr);
                let (size, _) = size_align(self.module, &enum_shape, &payload_ty);
                let out = self.new_temp(lir::LirType::Ptr);
                self.push(lir::Instruction::Call {
                    out: Some(out),
                    symbol: mir::RuntimeFn::Box.symbol().to_string(),
                    args: vec![td, payload, lir::Value::IntConst(size as i64)],
                });
                lir::Value::Temp(out)
            }
            // The payload sits right behind the object header: field
            // 1 of the boxed object's `{ ptr header, payload }` (see
            // the module docs).
            mir::Expr::Unbox(operand) => {
                let object = self.lower_expr(operand, &mir::Type::Any);
                let ty = self.value_type(ty);
                let out = self.new_temp(ty);
                self.push(lir::Instruction::ExtractValue {
                    out,
                    aggregate: object,
                    index: 1,
                });
                lir::Value::Temp(out)
            }
            // `scoop_rt_is_instance(obj, td)` (runtime spec 2.3).
            mir::Expr::IsInstance { operand, check_ty } => {
                let operand_ty = self.expr_ty(operand);
                let object = self.lower_expr(operand, &operand_ty);
                let td = self.td_ref(check_ty);
                let out = self.new_temp(lir::LirType::I1);
                self.push(lir::Instruction::Call {
                    out: Some(out),
                    symbol: mir::RuntimeFn::IsInstance.symbol().to_string(),
                    args: vec![object, td],
                });
                lir::Value::Temp(out)
            }
            // mir-lower expands `as` / `as?` into runtime checks plus
            // Option wrapping; the node never reaches LIR.
            mir::Expr::Cast { .. } => unreachable!("mir-lower expands casts before LIR"),
            // The enum operations map onto the corresponding LIR
            // instructions; the concrete representation (niche pointer
            // or tagged union) is fixed by the `EnumDef`, so codegen
            // translates them mechanically.
            mir::Expr::VariantConstruct {
                ty,
                variant,
                fields,
            } => {
                let mir::Type::Enum(enum_id, _) = ty else {
                    unreachable!("a variant construction has an enum type")
                };
                let field_types: Vec<mir::Type> = self.module.enums[*enum_id].variants
                    [*variant as usize]
                    .fields
                    .iter()
                    .map(|field| field.ty.clone())
                    .collect();
                let fields: Vec<lir::Value> = fields
                    .iter()
                    .zip(&field_types)
                    .map(|(field, ty)| self.lower_expr(field, ty))
                    .collect();
                let out_ty = self.value_type(ty);
                let out = self.new_temp(out_ty);
                self.push(lir::Instruction::EnumWrap {
                    out,
                    enum_id: enum_def_id(*enum_id),
                    variant: *variant,
                    fields,
                });
                lir::Value::Temp(out)
            }
            mir::Expr::EnumTag(operand) => {
                let operand_ty = self.expr_ty(operand);
                let mir::Type::Enum(enum_id, _) = &operand_ty else {
                    unreachable!("a tag read's operand is an enum value")
                };
                let operand = self.lower_expr(operand, &operand_ty);
                let out = self.new_temp(lir::LirType::I64);
                self.push(lir::Instruction::EnumTag {
                    out,
                    enum_id: enum_def_id(*enum_id),
                    operand,
                });
                lir::Value::Temp(out)
            }
            mir::Expr::EnumField {
                operand,
                variant,
                index,
            } => {
                let operand_ty = self.expr_ty(operand);
                let mir::Type::Enum(enum_id, _) = &operand_ty else {
                    unreachable!("an enum field read's operand is an enum value")
                };
                let enum_id = *enum_id;
                let operand = self.lower_expr(operand, &operand_ty);
                let out_ty = self.value_type(ty);
                let out = self.new_temp(out_ty);
                self.push(lir::Instruction::EnumField {
                    out,
                    enum_id: enum_def_id(enum_id),
                    variant: *variant,
                    index: *index,
                    operand,
                });
                lir::Value::Temp(out)
            }
            mir::Expr::Call(call) => self.lower_call(call, ty),
            mir::Expr::Binary { op, lhs, rhs } => match op {
                mir::BinOp::And => self.lower_short_circuit(lhs, rhs, true),
                mir::BinOp::Or => self.lower_short_circuit(lhs, rhs, false),
                _ => {
                    let (lir_op, ty, operand_ty) = binary_op(*op);
                    let lhs = self.lower_expr(lhs, &operand_ty);
                    let rhs = self.lower_expr(rhs, &operand_ty);
                    let out = self.new_temp(ty);
                    self.push(lir::Instruction::BinOp {
                        out,
                        op: lir_op,
                        lhs,
                        rhs,
                    });
                    lir::Value::Temp(out)
                }
            },
            mir::Expr::Unary { op, operand } => {
                let (lir_op, ty, operand_ty) = match op {
                    mir::UnOp::IntNeg => (lir::UnOp::Neg, lir::LirType::I64, mir::Type::Int),
                    mir::UnOp::BoolNot => (lir::UnOp::Not, lir::LirType::I1, mir::Type::Boolean),
                };
                let operand = self.lower_expr(operand, &operand_ty);
                let out = self.new_temp(ty);
                self.push(lir::Instruction::UnaryOp {
                    out,
                    op: lir_op,
                    operand,
                });
                lir::Value::Temp(out)
            }
        }
    }

    /// The shared trap block for `message` in this function (one per
    /// message, created on first use): calls the runtime trap —
    /// `void scoop_rt_trap(ptr)`, noreturn — with the message global
    /// and ends `unreachable`.
    fn trap_block(&mut self, message: &str) -> lir::BlockId {
        if let Some(&block) = self.trap_blocks.get(message) {
            return block;
        }
        let symbol = format!("scoop.cstr.{}", *self.cstr_count);
        *self.cstr_count += 1;
        let global = self.globals.alloc(lir::Global {
            symbol,
            init: lir::GlobalInit::CString(message.to_string()),
        });
        let block = self.new_block("unwrap.trap");
        // Fill the trap block out of line; the caller seals the
        // suspended current block with the branch.
        let saved = self.current;
        let saved_sealed = self.current_sealed;
        self.enter(block);
        self.push(lir::Instruction::Call {
            out: None,
            symbol: lir::TRAP_SYMBOL.to_string(),
            args: vec![lir::Value::Global(global)],
        });
        self.seal(lir::Terminator::Unreachable);
        self.trap_blocks.insert(message.to_string(), block);
        self.current = saved;
        self.current_sealed = saved_sealed;
        block
    }

    /// A `Value` naming the TypeDescriptor of `ty` (see the module
    /// docs for the TD-reference convention).
    fn td_ref(&mut self, ty: &mir::Type) -> lir::Value {
        lir::Value::Global(self.td_global(td_symbol_for(self.module, ty)))
    }

    /// The globals-arena stub for one referenced TypeDescriptor,
    /// deduplicated by symbol.
    fn td_global(&mut self, symbol: String) -> lir::GlobalId {
        if let Some(&id) = self.td_map.get(&symbol) {
            return id;
        }
        let id = self.globals.alloc(lir::Global {
            symbol: symbol.clone(),
            init: lir::GlobalInit::CString(String::new()),
        });
        self.td_map.insert(symbol, id);
        id
    }

    /// Struct / tuple construction: an aggregate of the mapped field
    /// values in declaration order.
    fn make_aggregate(&mut self, ty: &mir::Type, elements: Vec<lir::Value>) -> lir::Value {
        let ty = self.value_type(ty);
        let out = self.new_temp(ty);
        self.push(lir::Instruction::MakeAggregate { out, elements });
        lir::Value::Temp(out)
    }

    /// The Unit value: an empty aggregate (void calls and Unit
    /// literals produce it; void functions never return it).
    fn unit_value(&mut self) -> lir::Value {
        let out = self.new_temp(lir::LirType::Aggregate(Vec::new()));
        self.push(lir::Instruction::MakeAggregate {
            out,
            elements: Vec::new(),
        });
        lir::Value::Temp(out)
    }

    /// Lower `lhs && rhs` / `lhs || rhs` into basic blocks (DESIGN
    /// 2.4). The result flows through a hidden stack slot because LIR
    /// has no phi nodes; `rhs` is evaluated only in its own block, so
    /// side effects in `rhs` happen exactly when the short-circuit
    /// semantics demand it.
    fn lower_short_circuit(
        &mut self,
        lhs: &mir::Expr,
        rhs: &mir::Expr,
        is_and: bool,
    ) -> lir::Value {
        let result = self.new_hidden_local(lir::LirType::I1);
        let lhs = self.lower_expr(lhs, &mir::Type::Boolean);
        self.push(lir::Instruction::Store {
            local: result,
            value: lhs,
        });
        let rhs_block = self.new_block("sc.rhs");
        let merge_block = self.new_block("sc.merge");
        // `&&`: rhs decides only when lhs is true; `||`: when false.
        let (then_block, else_block) = if is_and {
            (rhs_block, merge_block)
        } else {
            (merge_block, rhs_block)
        };
        self.seal(lir::Terminator::CondBr {
            cond: lhs,
            then_block,
            else_block,
        });
        self.enter(rhs_block);
        let rhs = self.lower_expr(rhs, &mir::Type::Boolean);
        self.push(lir::Instruction::Store {
            local: result,
            value: rhs,
        });
        self.seal(lir::Terminator::Br(merge_block));
        self.enter(merge_block);
        lir::Value::Local(result)
    }

    fn lower_call(&mut self, call: &mir::Call, result_ty: &mir::Type) -> lir::Value {
        match call.target.callee {
            mir::Callee::User(id) => {
                let callee = &self.module.functions[id];
                let param_types: Vec<mir::Type> =
                    callee.params.iter().map(|param| param.ty.clone()).collect();
                let returns_unit = callee.return_ty == mir::Type::Unit;
                let symbol = callee.symbol.clone();
                // Arguments are evaluated left to right, before the call.
                let args: Vec<lir::Value> = call
                    .args
                    .iter()
                    .zip(&param_types)
                    .map(|(arg, ty)| self.lower_expr(arg, ty))
                    .collect();
                match call.target.kind {
                    mir::CallKind::Direct => {
                        self.finish_call(symbol, args, returns_unit, result_ty)
                    }
                    // vtable dispatch (impl spec 2.9): the receiver's
                    // object header holds the TypeDescriptor, whose
                    // vtable pointer is `ScoopTypeDescriptor` field 5.
                    mir::CallKind::Virtual { slot } => {
                        let td = self.heap_load(args[0], 0);
                        let vtable = self.heap_load(lir::Value::Temp(td), 5);
                        self.finish_indirect(vtable, slot, args, returns_unit, result_ty)
                    }
                    // itable dispatch: `scoop_rt_itable_lookup(td,
                    // iface_td)` finds the interface's table by its
                    // TypeDescriptor key.
                    mir::CallKind::Interface { interface, slot } => {
                        let td = self.heap_load(args[0], 0);
                        let iface_td = self.td_ref(&mir::Type::Interface(interface));
                        let table = self.new_temp(lir::LirType::Ptr);
                        self.push(lir::Instruction::Call {
                            out: Some(table),
                            symbol: mir::RuntimeFn::ITableLookup.symbol().to_string(),
                            args: vec![lir::Value::Temp(td), iface_td],
                        });
                        self.finish_indirect(table, slot, args, returns_unit, result_ty)
                    }
                }
            }
            mir::Callee::Runtime(mir::RuntimeFn::Trap) => {
                // The trap call (from `!!`) only appears as a
                // statement: the current block branches to the
                // function's shared trap block and is sealed, so
                // anything after it is unreachable. The message string
                // constant becomes a `CString` global.
                let message = match &call.args[0] {
                    mir::Expr::StringConst(id) => self.module.strings[*id].value.clone(),
                    _ => unreachable!("the trap message is a string constant"),
                };
                let trap = self.trap_block(&message);
                self.seal(lir::Terminator::Br(trap));
                self.current_sealed = true;
                // Dead value: the block is sealed, nothing consumes it.
                lir::Value::IntConst(0)
            }
            mir::Callee::Runtime(function) => {
                let symbol = function.symbol().to_string();
                let arg_types: Vec<mir::Type> = match function {
                    mir::RuntimeFn::Write => vec![mir::Type::String],
                    mir::RuntimeFn::IntToString => vec![mir::Type::Int],
                    mir::RuntimeFn::BoolToString => vec![mir::Type::Boolean],
                    mir::RuntimeFn::StringConcat | mir::RuntimeFn::StringEq => {
                        vec![mir::Type::String, mir::Type::String]
                    }
                    // The M6 runtime functions are emitted by the
                    // dedicated lowerings (Box / IsInstance / dispatch)
                    // or appear only as vtable slot symbols — never as
                    // plain MIR calls.
                    mir::RuntimeFn::Box
                    | mir::RuntimeFn::IsInstance
                    | mir::RuntimeFn::ITableLookup
                    | mir::RuntimeFn::AnyEquals
                    | mir::RuntimeFn::AnyHashCode
                    | mir::RuntimeFn::AnyToString => {
                        unreachable!("{function:?} calls are emitted by the dedicated M6 lowerings")
                    }
                    // Handled by the arm above.
                    mir::RuntimeFn::Trap => unreachable!("trap calls never reach here"),
                };
                let args: Vec<lir::Value> = call
                    .args
                    .iter()
                    .zip(&arg_types)
                    .map(|(arg, ty)| self.lower_expr(arg, ty))
                    .collect();
                match function {
                    mir::RuntimeFn::StringConcat
                    | mir::RuntimeFn::IntToString
                    | mir::RuntimeFn::BoolToString => {
                        self.call_with_result(symbol, args, lir::LirType::Ptr)
                    }
                    mir::RuntimeFn::StringEq => {
                        self.call_with_result(symbol, args, lir::LirType::I1)
                    }
                    mir::RuntimeFn::Write => {
                        self.push(lir::Instruction::Call {
                            out: None,
                            symbol,
                            args,
                        });
                        self.unit_value()
                    }
                    mir::RuntimeFn::Box
                    | mir::RuntimeFn::IsInstance
                    | mir::RuntimeFn::ITableLookup
                    | mir::RuntimeFn::AnyEquals
                    | mir::RuntimeFn::AnyHashCode
                    | mir::RuntimeFn::AnyToString => {
                        unreachable!("{function:?} calls are emitted by the dedicated M6 lowerings")
                    }
                    mir::RuntimeFn::Trap => unreachable!("trap calls never reach here"),
                }
            }
        }
    }

    /// A heap object field load: field `index` of the raw object
    /// struct the `Ptr` operand points at (see the module docs).
    fn heap_load(&mut self, object: lir::Value, index: u32) -> lir::TempId {
        let out = self.new_temp(lir::LirType::Ptr);
        self.push(lir::Instruction::ExtractValue {
            out,
            aggregate: object,
            index,
        });
        out
    }

    /// A direct call: Unit-returning callees are void at the LLVM
    /// level; their Unit value is a fresh empty aggregate.
    fn finish_call(
        &mut self,
        symbol: String,
        args: Vec<lir::Value>,
        returns_unit: bool,
        result_ty: &mir::Type,
    ) -> lir::Value {
        if returns_unit {
            self.push(lir::Instruction::Call {
                out: None,
                symbol,
                args,
            });
            self.unit_value()
        } else {
            let ty = self.value_type(result_ty);
            let out = self.new_temp(ty);
            self.push(lir::Instruction::Call {
                out: Some(out),
                symbol,
                args,
            });
            lir::Value::Temp(out)
        }
    }

    /// An indirect call through a function table (vtable / itable
    /// dispatch, impl spec 2.9).
    fn finish_indirect(
        &mut self,
        table: lir::TempId,
        slot: u32,
        args: Vec<lir::Value>,
        returns_unit: bool,
        result_ty: &mir::Type,
    ) -> lir::Value {
        if returns_unit {
            self.push(lir::Instruction::CallIndirect {
                out: None,
                table: lir::Value::Temp(table),
                slot,
                args,
            });
            self.unit_value()
        } else {
            let ty = self.value_type(result_ty);
            let out = self.new_temp(ty);
            self.push(lir::Instruction::CallIndirect {
                out: Some(out),
                table: lir::Value::Temp(table),
                slot,
                args,
            });
            lir::Value::Temp(out)
        }
    }

    fn call_with_result(
        &mut self,
        symbol: String,
        args: Vec<lir::Value>,
        ty: lir::LirType,
    ) -> lir::Value {
        let out = self.new_temp(ty);
        self.push(lir::Instruction::Call {
            out: Some(out),
            symbol,
            args,
        });
        lir::Value::Temp(out)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use scoop_ast::Span;

    const SPAN: Span = Span { start: 0, end: 0 };

    /// MIR module shell as mir-lower produces it.
    struct Builder {
        functions: Arena<mir::Function>,
        strings: Arena<mir::StringConst>,
        structs: Arena<mir::StructDef>,
        enums: Arena<mir::EnumDef>,
        classes: Arena<mir::ClassDef>,
        interfaces: Arena<mir::InterfaceDef>,
        top_level: Vec<mir::FunctionId>,
    }

    impl Builder {
        fn new() -> Self {
            Builder {
                functions: Arena::new(),
                strings: Arena::new(),
                structs: Arena::new(),
                enums: Arena::new(),
                classes: Arena::new(),
                interfaces: Arena::new(),
                top_level: Vec::new(),
            }
        }

        /// `enum Option<T> { Some(T), None }` instantiated at
        /// `payload`, named as mir-lower names its instances.
        fn option_enum(&mut self, name: &str, payload: mir::Type) -> mir::EnumId {
            self.enums.alloc(mir::EnumDef {
                name: name.to_string(),
                variants: vec![
                    mir::VariantDef {
                        name: "Some".to_string(),
                        fields: vec![mir::Field {
                            name: "_1".to_string(),
                            ty: payload,
                        }],
                    },
                    mir::VariantDef {
                        name: "None".to_string(),
                        fields: Vec::new(),
                    },
                ],
            })
        }

        fn string(&mut self, value: &str) -> mir::StringConstId {
            let symbol = format!("scoop.str.{}", self.strings.len());
            self.strings.alloc(mir::StringConst {
                value: value.to_string(),
                symbol,
            })
        }

        fn strukt(&mut self, name: &str, fields: &[(&str, mir::Type)]) -> mir::StructId {
            self.structs.alloc(mir::StructDef {
                name: name.to_string(),
                fields: fields
                    .iter()
                    .map(|(name, ty)| mir::Field {
                        name: name.to_string(),
                        ty: ty.clone(),
                    })
                    .collect(),
            })
        }

        fn interface(&mut self, name: &str, methods: &[&str]) -> mir::InterfaceId {
            self.interfaces.alloc(mir::InterfaceDef {
                name: name.to_string(),
                methods: methods.iter().map(|m| m.to_string()).collect(),
            })
        }

        fn class(
            &mut self,
            name: &str,
            base: Option<mir::ClassId>,
            fields: &[(&str, mir::Type)],
            vtable: Vec<mir::TableSlot>,
            itables: Vec<mir::ItableRecord>,
        ) -> mir::ClassId {
            self.classes.alloc(mir::ClassDef {
                modifier: mir::ClassModifier::Final,
                name: name.to_string(),
                fields: fields
                    .iter()
                    .map(|(name, ty)| mir::Field {
                        name: name.to_string(),
                        ty: ty.clone(),
                    })
                    .collect(),
                base_class: base,
                interfaces: Vec::new(),
                vtable,
                itables,
            })
        }

        /// A function that exists only as a signature (e.g. an
        /// interface method shell): not pushed to `top_level`, so it
        /// is never emitted.
        fn decl_fn(
            &mut self,
            name: &str,
            symbol: &str,
            params: Vec<mir::Param>,
            return_ty: mir::Type,
        ) -> mir::FunctionId {
            self.functions.alloc(mir::Function {
                name: name.to_string(),
                symbol: symbol.to_string(),
                params,
                return_ty,
                body: mir::Body {
                    locals: Arena::new(),
                    statements: Vec::new(),
                },
            })
        }

        fn user_fn(
            &mut self,
            name: &str,
            symbol: &str,
            locals: Arena<mir::Local>,
            statements: Vec<mir::Statement>,
        ) -> mir::FunctionId {
            self.user_fn_full(
                name,
                symbol,
                Vec::new(),
                mir::Type::Unit,
                locals,
                statements,
            )
        }

        fn user_fn_full(
            &mut self,
            name: &str,
            symbol: &str,
            params: Vec<mir::Param>,
            return_ty: mir::Type,
            locals: Arena<mir::Local>,
            statements: Vec<mir::Statement>,
        ) -> mir::FunctionId {
            let id = self.functions.alloc(mir::Function {
                name: name.to_string(),
                symbol: symbol.to_string(),
                params,
                return_ty,
                body: mir::Body { locals, statements },
            });
            self.top_level.push(id);
            id
        }

        fn main(
            &mut self,
            locals: Arena<mir::Local>,
            statements: Vec<mir::Statement>,
        ) -> mir::FunctionId {
            self.user_fn("main", mir::ENTRY_SYMBOL, locals, statements)
        }

        fn finish(self, entry: mir::FunctionId) -> mir::Module {
            mir::Module {
                functions: self.functions,
                top_level: self.top_level,
                strings: self.strings,
                structs: self.structs,
                enums: self.enums,
                classes: self.classes,
                interfaces: self.interfaces,
                entry,
                meta: mir::MirMeta::default(),
            }
        }
    }

    /// The reference-field offsets of a plain (non-enum) layout.
    fn plain_refs(layout: &lir::Layout) -> &[u64] {
        let lir::LayoutKind::Plain { ref_field_offsets } = &layout.kind else {
            panic!("expected a plain layout")
        };
        ref_field_offsets
    }

    fn local(name: &str, ty: mir::Type) -> mir::Local {
        mir::Local {
            name: name.to_string(),
            ty,
            mutable: false,
        }
    }

    fn var(name: &str, ty: mir::Type) -> mir::Local {
        mir::Local {
            name: name.to_string(),
            ty,
            mutable: true,
        }
    }

    fn stmt(kind: mir::StatementKind) -> mir::Statement {
        mir::Statement { kind, span: SPAN }
    }

    fn val_decl(local: mir::LocalId, init: mir::Expr) -> mir::Statement {
        stmt(mir::StatementKind::ValDecl { local, init })
    }

    fn assign(local: mir::LocalId, value: mir::Expr) -> mir::Statement {
        stmt(mir::StatementKind::Assign { local, value })
    }

    fn expr_stmt(expr: mir::Expr) -> mir::Statement {
        stmt(mir::StatementKind::Expr(expr))
    }

    fn binary(op: mir::BinOp, lhs: mir::Expr, rhs: mir::Expr) -> mir::Expr {
        mir::Expr::Binary {
            op,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        }
    }

    fn runtime_call(function: mir::RuntimeFn, args: Vec<mir::Expr>) -> mir::Expr {
        mir::Expr::Call(mir::Call {
            target: mir::CallTarget {
                kind: mir::CallKind::Direct,
                callee: mir::Callee::Runtime(function),
            },
            args,
        })
    }

    fn user_call(function: mir::FunctionId) -> mir::Expr {
        mir::Expr::Call(mir::Call {
            target: mir::CallTarget {
                kind: mir::CallKind::Direct,
                callee: mir::Callee::User(function),
            },
            args: Vec::new(),
        })
    }

    /// `main` writes `"hello, world"` (core's `write` primitive, M7)
    /// then calls `helper()`, which writes `"!"`.
    fn hello_world() -> mir::Module {
        let mut b = Builder::new();
        let hello = b.string("hello, world");
        let bang = b.string("!");
        let helper = b.user_fn(
            "helper",
            "scoop.helper",
            Arena::new(),
            vec![expr_stmt(runtime_call(
                mir::RuntimeFn::Write,
                vec![mir::Expr::StringConst(bang)],
            ))],
        );
        let main = b.main(
            Arena::new(),
            vec![
                expr_stmt(runtime_call(
                    mir::RuntimeFn::Write,
                    vec![mir::Expr::StringConst(hello)],
                )),
                expr_stmt(user_call(helper)),
            ],
        );
        b.finish(main)
    }

    #[test]
    fn lowers_hello_world() {
        let module = lower(&hello_world());

        // Globals: one per MIR string constant, same symbol and value.
        let globals: Vec<(&str, &str)> = module
            .globals
            .iter()
            .map(|(_, g)| match &g.init {
                lir::GlobalInit::StringConst(value) => (g.symbol.as_str(), value.as_str()),
                lir::GlobalInit::CString(value) => (g.symbol.as_str(), value.as_str()),
            })
            .collect();
        assert_eq!(
            globals,
            [("scoop.str.0", "hello, world"), ("scoop.str.1", "!")]
        );

        // Functions keep their mangled symbols; the entry symbol is the
        // fixed `scoop_main`.
        let symbols: Vec<&str> = module.functions.iter().map(|f| f.symbol.as_str()).collect();
        assert_eq!(symbols, ["scoop.helper", mir::ENTRY_SYMBOL]);
        assert_eq!(module.entry_symbol, mir::ENTRY_SYMBOL);

        // Golden dump locks the output structure.
        let expected = "\
Module
  global @scoop.str.0 = \"hello, world\"
  global @scoop.str.1 = \"!\"
  fun @scoop.helper() -> void
  block entry
    call @scoop_rt_print(global1)
    t0 = aggregate () : {}
    ret
  fun @scoop_main() -> void
  block entry
    call @scoop_rt_print(global0)
    t0 = aggregate () : {}
    call @scoop.helper()
    t1 = aggregate () : {}
    ret
  layout String size=16 align=8 refs=[]
  layout Int size=8 align=8 refs=[]
  layout Boolean size=1 align=1 refs=[]
  entry @scoop_main
";
        assert_eq!(lir::dump(&module), expected);
    }

    #[test]
    fn if_else_becomes_basic_blocks() {
        let mut b = Builder::new();
        let ok = b.string("ok");
        let ng = b.string("ng");
        let main = b.main(
            Arena::new(),
            vec![stmt(mir::StatementKind::If {
                cond: mir::Expr::BoolLiteral(true),
                then_body: vec![expr_stmt(runtime_call(
                    mir::RuntimeFn::Write,
                    vec![mir::Expr::StringConst(ok)],
                ))],
                else_body: Some(vec![expr_stmt(runtime_call(
                    mir::RuntimeFn::Write,
                    vec![mir::Expr::StringConst(ng)],
                ))]),
            })],
        );
        let module = lower(&b.finish(main));

        let expected = "\
Module
  global @scoop.str.0 = \"ok\"
  global @scoop.str.1 = \"ng\"
  fun @scoop_main() -> void
  block entry
    cbr true then @if.then.1 else @if.else.2
  block if.then.1
    call @scoop_rt_print(global0)
    t0 = aggregate () : {}
    br @if.merge.3
  block if.else.2
    call @scoop_rt_print(global1)
    t1 = aggregate () : {}
    br @if.merge.3
  block if.merge.3
    ret
  layout String size=16 align=8 refs=[]
  layout Int size=8 align=8 refs=[]
  layout Boolean size=1 align=1 refs=[]
  entry @scoop_main
";
        assert_eq!(lir::dump(&module), expected);
    }

    #[test]
    fn while_becomes_basic_blocks() {
        // var n = 0; while (n < 3) { n = n + 1 }
        let mut b = Builder::new();
        let mut locals = Arena::new();
        let n = locals.alloc(var("n", mir::Type::Int));
        let main = b.main(
            locals,
            vec![
                val_decl(n, mir::Expr::IntLiteral(0)),
                stmt(mir::StatementKind::While {
                    cond: binary(
                        mir::BinOp::IntLt,
                        mir::Expr::Local(n),
                        mir::Expr::IntLiteral(3),
                    ),
                    body: vec![assign(
                        n,
                        binary(
                            mir::BinOp::IntAdd,
                            mir::Expr::Local(n),
                            mir::Expr::IntLiteral(1),
                        ),
                    )],
                }),
            ],
        );
        let module = lower(&b.finish(main));

        let expected = "\
Module
  fun @scoop_main() -> void
    local %0 n: i64
  block entry
    store 0 -> local0
    br @while.cond.1
  block while.cond.1
    t0 = Lt local0, 3 : i1
    cbr t0 then @while.body.2 else @while.exit.3
  block while.body.2
    t1 = Add local0, 1 : i64
    store t1 -> local0
    br @while.cond.1
  block while.exit.3
    ret
  layout String size=16 align=8 refs=[]
  layout Int size=8 align=8 refs=[]
  layout Boolean size=1 align=1 refs=[]
  entry @scoop_main
";
        assert_eq!(lir::dump(&module), expected);
    }

    #[test]
    fn and_short_circuits_through_blocks() {
        // val b = (s0 == s1) && (s2 == s3): the second comparison call
        // sits in its own block, executed only when the first is true.
        let mut b = Builder::new();
        let s0 = b.string("a");
        let s1 = b.string("b");
        let s2 = b.string("c");
        let s3 = b.string("d");
        let string_eq = |l, r| {
            runtime_call(
                mir::RuntimeFn::StringEq,
                vec![mir::Expr::StringConst(l), mir::Expr::StringConst(r)],
            )
        };
        let mut locals = Arena::new();
        let result = locals.alloc(local("b", mir::Type::Boolean));
        let main = b.main(
            locals,
            vec![val_decl(
                result,
                binary(mir::BinOp::And, string_eq(s0, s1), string_eq(s2, s3)),
            )],
        );
        let module = lower(&b.finish(main));

        let expected = "\
Module
  global @scoop.str.0 = \"a\"
  global @scoop.str.1 = \"b\"
  global @scoop.str.2 = \"c\"
  global @scoop.str.3 = \"d\"
  fun @scoop_main() -> void
    local %0 b: i1
    local %1 $sc.1: i1
  block entry
    t0 = call @scoop_rt_string_eq(global0, global1) : i1
    store t0 -> local1
    cbr t0 then @sc.rhs.1 else @sc.merge.2
  block sc.rhs.1
    t1 = call @scoop_rt_string_eq(global2, global3) : i1
    store t1 -> local1
    br @sc.merge.2
  block sc.merge.2
    store local1 -> local0
    ret
  layout String size=16 align=8 refs=[]
  layout Int size=8 align=8 refs=[]
  layout Boolean size=1 align=1 refs=[]
  entry @scoop_main
";
        assert_eq!(lir::dump(&module), expected);
    }

    #[test]
    fn or_short_circuits_through_blocks() {
        // val b = x || y: when x is true, y is never evaluated.
        let mut b = Builder::new();
        let mut locals = Arena::new();
        let x = locals.alloc(local("x", mir::Type::Boolean));
        let y = locals.alloc(local("y", mir::Type::Boolean));
        let result = locals.alloc(local("b", mir::Type::Boolean));
        let main = b.main(
            locals,
            vec![
                val_decl(x, mir::Expr::BoolLiteral(true)),
                val_decl(y, mir::Expr::BoolLiteral(false)),
                val_decl(
                    result,
                    binary(mir::BinOp::Or, mir::Expr::Local(x), mir::Expr::Local(y)),
                ),
            ],
        );
        let module = lower(&b.finish(main));

        let expected = "\
Module
  fun @scoop_main() -> void
    local %0 x: i1
    local %1 y: i1
    local %2 b: i1
    local %3 $sc.1: i1
  block entry
    store true -> local0
    store false -> local1
    store local0 -> local3
    cbr local0 then @sc.merge.2 else @sc.rhs.1
  block sc.rhs.1
    store local1 -> local3
    br @sc.merge.2
  block sc.merge.2
    store local3 -> local2
    ret
  layout String size=16 align=8 refs=[]
  layout Int size=8 align=8 refs=[]
  layout Boolean size=1 align=1 refs=[]
  entry @scoop_main
";
        assert_eq!(lir::dump(&module), expected);
    }

    #[test]
    fn runtime_calls_with_results_produce_typed_temps() {
        let mut b = Builder::new();
        let s0 = b.string("a");
        let s1 = b.string("b");
        let helper = b.user_fn("helper", "scoop.helper", Arena::new(), vec![]);
        let mut locals = Arena::new();
        let s = locals.alloc(local("s", mir::Type::String));
        let e = locals.alloc(local("e", mir::Type::Boolean));
        let i = locals.alloc(local("i", mir::Type::String));
        let o = locals.alloc(local("o", mir::Type::String));
        let main = b.main(
            locals,
            vec![
                val_decl(
                    s,
                    runtime_call(
                        mir::RuntimeFn::StringConcat,
                        vec![mir::Expr::StringConst(s0), mir::Expr::StringConst(s1)],
                    ),
                ),
                val_decl(
                    e,
                    runtime_call(
                        mir::RuntimeFn::StringEq,
                        vec![mir::Expr::StringConst(s0), mir::Expr::StringConst(s1)],
                    ),
                ),
                val_decl(
                    i,
                    runtime_call(mir::RuntimeFn::IntToString, vec![mir::Expr::IntLiteral(42)]),
                ),
                val_decl(
                    o,
                    runtime_call(
                        mir::RuntimeFn::BoolToString,
                        vec![mir::Expr::BoolLiteral(true)],
                    ),
                ),
                expr_stmt(user_call(helper)),
            ],
        );
        let module = lower(&b.finish(main));

        // top_level order: helper first, then main.
        let function = &module.functions[1];
        let instructions = &function.blocks[function.entry].instructions;

        let lir::Instruction::Call {
            out: Some(concat_out),
            symbol,
            ..
        } = &instructions[0]
        else {
            panic!("string concat must produce a value")
        };
        assert_eq!(symbol, "scoop_rt_string_concat");
        assert_eq!(function.temps[*concat_out].ty, lir::LirType::Ptr);
        assert!(matches!(instructions[1], lir::Instruction::Store { .. }));

        let lir::Instruction::Call {
            out: Some(eq_out),
            symbol,
            ..
        } = &instructions[2]
        else {
            panic!("string eq must produce a value")
        };
        assert_eq!(symbol, "scoop_rt_string_eq");
        assert_eq!(function.temps[*eq_out].ty, lir::LirType::I1);
        assert!(matches!(instructions[3], lir::Instruction::Store { .. }));

        // The M7 conversion intrinsics (`ptr(i64)` / `ptr(i1)`).
        let lir::Instruction::Call {
            out: Some(its_out),
            symbol,
            ..
        } = &instructions[4]
        else {
            panic!("intToString must produce a value")
        };
        assert_eq!(symbol, "scoop_rt_int_to_string");
        assert_eq!(function.temps[*its_out].ty, lir::LirType::Ptr);
        assert!(matches!(instructions[5], lir::Instruction::Store { .. }));

        let lir::Instruction::Call {
            out: Some(bts_out),
            symbol,
            ..
        } = &instructions[6]
        else {
            panic!("boolToString must produce a value")
        };
        assert_eq!(symbol, "scoop_rt_bool_to_string");
        assert_eq!(function.temps[*bts_out].ty, lir::LirType::Ptr);
        assert!(matches!(instructions[7], lir::Instruction::Store { .. }));

        // User calls return void; the Unit value is a fresh empty
        // aggregate.
        let lir::Instruction::Call {
            out: None, symbol, ..
        } = &instructions[8]
        else {
            panic!("user calls must return void")
        };
        assert_eq!(symbol, "scoop.helper");
        let lir::Instruction::MakeAggregate { out, elements } = &instructions[9] else {
            panic!("a void call's Unit value must be an empty aggregate")
        };
        assert!(elements.is_empty());
        assert_eq!(function.temps[*out].ty, lir::LirType::Aggregate(Vec::new()));
    }

    #[test]
    fn arithmetic_and_comparison_ops_map_to_lir_ops() {
        let mut b = Builder::new();
        let int_cases = [
            (mir::BinOp::IntAdd, lir::BinOp::Add, lir::LirType::I64),
            (mir::BinOp::IntSub, lir::BinOp::Sub, lir::LirType::I64),
            (mir::BinOp::IntMul, lir::BinOp::Mul, lir::LirType::I64),
            (mir::BinOp::IntDiv, lir::BinOp::SDiv, lir::LirType::I64),
            (mir::BinOp::IntLt, lir::BinOp::Lt, lir::LirType::I1),
            (mir::BinOp::IntLe, lir::BinOp::Le, lir::LirType::I1),
            (mir::BinOp::IntGt, lir::BinOp::Gt, lir::LirType::I1),
            (mir::BinOp::IntGe, lir::BinOp::Ge, lir::LirType::I1),
            (mir::BinOp::IntEq, lir::BinOp::Eq, lir::LirType::I1),
            (mir::BinOp::IntNe, lir::BinOp::Ne, lir::LirType::I1),
        ];
        let mut statements: Vec<mir::Statement> = int_cases
            .iter()
            .map(|(mir_op, _, _)| {
                expr_stmt(binary(
                    *mir_op,
                    mir::Expr::IntLiteral(1),
                    mir::Expr::IntLiteral(2),
                ))
            })
            .collect();
        let bool_cases = [
            (mir::BinOp::BoolEq, lir::BinOp::Eq, lir::LirType::I1),
            (mir::BinOp::BoolNe, lir::BinOp::Ne, lir::LirType::I1),
        ];
        for (mir_op, _, _) in &bool_cases {
            statements.push(expr_stmt(binary(
                *mir_op,
                mir::Expr::BoolLiteral(true),
                mir::Expr::BoolLiteral(false),
            )));
        }
        let main = b.main(Arena::new(), statements);
        let module = lower(&b.finish(main));

        let expected: Vec<(lir::BinOp, lir::LirType)> = int_cases
            .iter()
            .chain(bool_cases.iter())
            .map(|(_, lir_op, ty)| (*lir_op, ty.clone()))
            .collect();
        let function = &module.functions[0];
        let ops: Vec<(lir::BinOp, lir::LirType)> = function.blocks[function.entry]
            .instructions
            .iter()
            .map(|instruction| {
                let lir::Instruction::BinOp { out, op, .. } = instruction else {
                    panic!("expected a binary instruction")
                };
                (*op, function.temps[*out].ty.clone())
            })
            .collect();
        assert_eq!(ops, expected);
    }

    #[test]
    fn unary_ops_map_to_lir_unops() {
        let mut b = Builder::new();
        let main = b.main(
            Arena::new(),
            vec![
                expr_stmt(mir::Expr::Unary {
                    op: mir::UnOp::IntNeg,
                    operand: Box::new(mir::Expr::IntLiteral(1)),
                }),
                expr_stmt(mir::Expr::Unary {
                    op: mir::UnOp::BoolNot,
                    operand: Box::new(mir::Expr::BoolLiteral(true)),
                }),
            ],
        );
        let module = lower(&b.finish(main));

        let function = &module.functions[0];
        let ops: Vec<(lir::UnOp, lir::LirType)> = function.blocks[function.entry]
            .instructions
            .iter()
            .map(|instruction| {
                let lir::Instruction::UnaryOp { out, op, .. } = instruction else {
                    panic!("expected a unary instruction")
                };
                (*op, function.temps[*out].ty.clone())
            })
            .collect();
        assert_eq!(
            ops,
            [
                (lir::UnOp::Neg, lir::LirType::I64),
                (lir::UnOp::Not, lir::LirType::I1),
            ]
        );
    }

    #[test]
    fn struct_values_are_aggregates() {
        let mut b = Builder::new();
        let point = b.strukt("Point", &[("x", mir::Type::Int), ("y", mir::Type::Int)]);
        let mut locals = Arena::new();
        let p = locals.alloc(local("p", mir::Type::Struct(point)));
        let x = locals.alloc(local("x", mir::Type::Int));
        let main = b.main(
            locals,
            vec![
                val_decl(
                    p,
                    mir::Expr::StructInit {
                        struct_id: point,
                        args: vec![mir::Expr::IntLiteral(1), mir::Expr::IntLiteral(2)],
                    },
                ),
                val_decl(
                    x,
                    mir::Expr::FieldAccess {
                        receiver: Box::new(mir::Expr::Local(p)),
                        index: 0,
                    },
                ),
            ],
        );
        let module = lower(&b.finish(main));

        let function = &module.functions[0];
        let local_types: Vec<lir::LirType> =
            function.locals.iter().map(|(_, l)| l.ty.clone()).collect();
        assert_eq!(
            local_types,
            [
                lir::LirType::Aggregate(vec![lir::LirType::I64, lir::LirType::I64]),
                lir::LirType::I64,
            ]
        );

        let instructions = &function.blocks[function.entry].instructions;
        let lir::Instruction::MakeAggregate { out, elements } = &instructions[0] else {
            panic!("struct construction must build an aggregate")
        };
        assert_eq!(elements.len(), 2);
        assert_eq!(
            function.temps[*out].ty,
            lir::LirType::Aggregate(vec![lir::LirType::I64, lir::LirType::I64])
        );
        assert!(matches!(instructions[1], lir::Instruction::Store { .. }));
        let lir::Instruction::ExtractValue { out, index, .. } = &instructions[2] else {
            panic!("field access must extract from the aggregate")
        };
        assert_eq!(*index, 0);
        assert_eq!(function.temps[*out].ty, lir::LirType::I64);
        assert!(matches!(instructions[3], lir::Instruction::Store { .. }));

        // The struct layout is in the meta.
        let layout = module
            .meta
            .layouts
            .iter()
            .find(|l| l.name == "Point")
            .expect("a layout per struct");
        assert_eq!((layout.size, layout.align), (16, 8));
        assert!(plain_refs(layout).is_empty());
    }

    #[test]
    fn unit_is_the_empty_aggregate() {
        let mut b = Builder::new();
        let mut locals = Arena::new();
        let u = locals.alloc(local("u", mir::Type::Unit));
        let main = b.main(locals, vec![val_decl(u, mir::Expr::UnitLiteral)]);
        let module = lower(&b.finish(main));

        let function = &module.functions[0];
        let (_, u_local) = function.locals.iter().next().expect("one local");
        assert_eq!(u_local.ty, lir::LirType::Aggregate(Vec::new()));
        let lir::Instruction::MakeAggregate { out, elements } =
            &function.blocks[function.entry].instructions[0]
        else {
            panic!("Unit must be an empty aggregate")
        };
        assert!(elements.is_empty());
        assert_eq!(function.temps[*out].ty, lir::LirType::Aggregate(Vec::new()));

        // Unit itself gets no layout entry (it is just `{}`).
        assert!(!module.meta.layouts.iter().any(|l| l.name == "Unit"));
    }

    #[test]
    fn layouts_mark_reference_fields_for_the_gc() {
        let mut b = Builder::new();
        // String field behind one Int: the reference sits at offset 8.
        let s = b.strukt("S", &[("a", mir::Type::Int), ("s", mir::Type::String)]);
        // A String nested inside a tuple field, after a Boolean: the
        // tuple is 8-aligned, so it starts at offset 8.
        let pair = mir::Type::Tuple(vec![mir::Type::String, mir::Type::Int]);
        let _outer = b.strukt("Outer", &[("flag", mir::Type::Boolean), ("pair", pair)]);
        let _ = s;
        // A tuple type that only appears in code (padding: Boolean
        // then Int at offset 8).
        let mut locals = Arena::new();
        let _t = locals.alloc(local(
            "t",
            mir::Type::Tuple(vec![mir::Type::Boolean, mir::Type::Int]),
        ));
        let main = b.main(locals, vec![]);
        let module = lower(&b.finish(main));

        let by_name = |name: &str| {
            module
                .meta
                .layouts
                .iter()
                .find(|l| l.name == name)
                .unwrap_or_else(|| panic!("missing layout for {name}"))
        };

        // Builtins first: the runtime String object header + scalars.
        let names: Vec<&str> = module
            .meta
            .layouts
            .iter()
            .map(|l| l.name.as_str())
            .collect();
        assert_eq!(
            names,
            [
                "String",
                "Int",
                "Boolean",
                "S",
                "Outer",
                "(String, Int)",
                "(Boolean, Int)"
            ]
        );

        let string = by_name("String");
        assert_eq!((string.size, string.align), (16, 8));
        assert!(plain_refs(string).is_empty());

        // S { a: Int @0, s: String @8 }: size 16, align 8, refs [8].
        let s_layout = by_name("S");
        assert_eq!((s_layout.size, s_layout.align), (16, 8));
        assert_eq!(plain_refs(s_layout), [8]);

        // Outer { flag: Boolean @0, pair: (String, Int) @8 } with the
        // String at pair+0: size 24, align 8, refs [8].
        let outer = by_name("Outer");
        assert_eq!((outer.size, outer.align), (24, 8));
        assert_eq!(plain_refs(outer), [8]);

        // The tuple field type gets its own layout too.
        let pair_layout = by_name("(String, Int)");
        assert_eq!((pair_layout.size, pair_layout.align), (16, 8));
        assert_eq!(plain_refs(pair_layout), [0]);

        // (Boolean, Int): Int is 8-aligned, so it sits at offset 8 and
        // the size rounds up to 16.
        let padded = by_name("(Boolean, Int)");
        assert_eq!((padded.size, padded.align), (16, 8));
        assert!(plain_refs(padded).is_empty());
    }

    fn param(name: &str, ty: mir::Type, local: mir::LocalId) -> mir::Param {
        mir::Param {
            name: name.to_string(),
            ty,
            local,
        }
    }

    fn return_stmt(value: mir::Expr) -> mir::Statement {
        stmt(mir::StatementKind::Return { value: Some(value) })
    }

    #[test]
    fn function_signatures_params_and_calls() {
        let mut b = Builder::new();
        // fun add(x: Int, y: Int): Int { return x + y }
        let mut locals = Arena::new();
        let x = locals.alloc(local("x", mir::Type::Int));
        let y = locals.alloc(local("y", mir::Type::Int));
        let add = b.user_fn_full(
            "add",
            "scoop.add",
            vec![param("x", mir::Type::Int, x), param("y", mir::Type::Int, y)],
            mir::Type::Int,
            locals,
            vec![return_stmt(binary(
                mir::BinOp::IntAdd,
                mir::Expr::Local(x),
                mir::Expr::Local(y),
            ))],
        );
        // main: val r = add(40, 2)
        let mut main_locals = Arena::new();
        let r = main_locals.alloc(local("r", mir::Type::Int));
        let main = b.main(
            main_locals,
            vec![val_decl(
                r,
                mir::Expr::Call(mir::Call {
                    target: mir::CallTarget {
                        kind: mir::CallKind::Direct,
                        callee: mir::Callee::User(add),
                    },
                    args: vec![mir::Expr::IntLiteral(40), mir::Expr::IntLiteral(2)],
                }),
            )],
        );
        let module = lower(&b.finish(main));

        // Parameters are SSA values (`Value::Param`), not stack slots;
        // the add body has no locals at all.
        let add_fn = &module.functions[0];
        assert_eq!(add_fn.params, [lir::LirType::I64, lir::LirType::I64]);
        assert_eq!(add_fn.return_ty, lir::LirType::I64);
        assert_eq!(add_fn.locals.len(), 0);

        let expected = "\
Module
  fun @scoop.add(i64, i64) -> i64
  block entry
    t0 = Add param0, param1 : i64
    ret t0
  fun @scoop_main() -> void
    local %0 r: i64
  block entry
    t0 = call @scoop.add(40, 2) : i64
    store t0 -> local0
    ret
  layout String size=16 align=8 refs=[]
  layout Int size=8 align=8 refs=[]
  layout Boolean size=1 align=1 refs=[]
  entry @scoop_main
";
        assert_eq!(lir::dump(&module), expected);
    }

    #[test]
    fn boxed_primitive_tostring_unboxes_and_converts() {
        // mir-lower's generated `scoop.tostring.I` (M7, the boxed
        // Int's vtable slot 2): unbox the payload and convert it
        // through the runtime.
        let mut b = Builder::new();
        let mut locals = Arena::new();
        let this = locals.alloc(local("this", mir::Type::Any));
        b.user_fn_full(
            "tostring.I",
            "scoop.tostring.I",
            vec![param("this", mir::Type::Any, this)],
            mir::Type::String,
            locals,
            vec![return_stmt(runtime_call(
                mir::RuntimeFn::IntToString,
                vec![mir::Expr::Unbox(Box::new(mir::Expr::Local(this)))],
            ))],
        );
        let main = b.main(Arena::new(), vec![]);
        let module = lower(&b.finish(main));

        let expected = "\
Module
  fun @scoop.tostring.I(ptr) -> ptr
  block entry
    t0 = extract param0, 1 : i64
    t1 = call @scoop_rt_int_to_string(t0) : ptr
    ret t1
  fun @scoop_main() -> void
  block entry
    ret
  layout String size=16 align=8 refs=[]
  layout Int size=8 align=8 refs=[]
  layout Boolean size=1 align=1 refs=[]
  entry @scoop_main
";
        assert_eq!(lir::dump(&module), expected);
    }

    #[test]
    fn return_inside_a_branch_seals_its_block() {
        // fun f(x: Int): Int { if (true) { return x }; return 0 }
        let mut b = Builder::new();
        let mut locals = Arena::new();
        let x = locals.alloc(local("x", mir::Type::Int));
        let f = b.user_fn_full(
            "f",
            "scoop.f",
            vec![param("x", mir::Type::Int, x)],
            mir::Type::Int,
            locals,
            vec![
                stmt(mir::StatementKind::If {
                    cond: mir::Expr::BoolLiteral(true),
                    then_body: vec![return_stmt(mir::Expr::Local(x))],
                    else_body: None,
                }),
                return_stmt(mir::Expr::IntLiteral(0)),
            ],
        );
        let _ = f;
        let main = b.main(Arena::new(), vec![]);
        let module = lower(&b.finish(main));

        // The `return` seals the then block: no branch to the merge
        // block follows it.
        let expected = "\
Module
  fun @scoop.f(i64) -> i64
  block entry
    cbr true then @if.then.1 else @if.merge.2
  block if.then.1
    ret param0
  block if.merge.2
    ret 0
  fun @scoop_main() -> void
  block entry
    ret
  layout String size=16 align=8 refs=[]
  layout Int size=8 align=8 refs=[]
  layout Boolean size=1 align=1 refs=[]
  entry @scoop_main
";
        assert_eq!(lir::dump(&module), expected);
    }

    /// The LIR enum definition transposed from a MIR enum (the arenas
    /// align 1:1).
    fn edef(module: &lir::Module, id: mir::EnumId) -> &lir::EnumDef {
        &module.enums[lir::EnumDefId::from_raw(id.into_raw())]
    }

    /// main holding `o: Option<T>` through a None / tag / field /
    /// wrap round-trip; shared shell of the two representation tests.
    fn option_round_trip(name: &str, payload: mir::Type) -> mir::Module {
        let mut b = Builder::new();
        let option = b.option_enum(name, payload.clone());
        let option_ty = mir::Type::Enum(option, vec![payload.clone()]);
        let mut locals = Arena::new();
        let o = locals.alloc(local("o", option_ty.clone()));
        let t = locals.alloc(local("t", mir::Type::Int));
        let p = locals.alloc(local("p", payload));
        let o2 = locals.alloc(local("o2", option_ty.clone()));
        let main = b.main(
            locals,
            vec![
                // None
                val_decl(
                    o,
                    mir::Expr::VariantConstruct {
                        ty: option_ty.clone(),
                        variant: 1,
                        fields: Vec::new(),
                    },
                ),
                val_decl(t, mir::Expr::EnumTag(Box::new(mir::Expr::Local(o)))),
                val_decl(
                    p,
                    mir::Expr::EnumField {
                        operand: Box::new(mir::Expr::Local(o)),
                        variant: 0,
                        index: 0,
                    },
                ),
                val_decl(
                    o2,
                    mir::Expr::VariantConstruct {
                        ty: option_ty,
                        variant: 0,
                        fields: vec![mir::Expr::Local(p)],
                    },
                ),
            ],
        );
        b.finish(main)
    }

    #[test]
    fn option_of_string_uses_the_niche_representation() {
        // Option<String>: the payload maps to `Ptr`, so the value is
        // the pointer itself with None = null (spec 7.4).
        let module = lower(&option_round_trip("Option$S", mir::Type::String));

        let expected = "\
Module
  enum Option$S niche(payload_variant=0)
  fun @scoop_main() -> void
    local %0 o: enum0
    local %1 t: i64
    local %2 p: ptr
    local %3 o2: enum0
  block entry
    t0 = enum_wrap e0 v1 () : enum0
    store t0 -> local0
    t1 = enum_tag e0 local0 : i64
    store t1 -> local1
    t2 = enum_field e0 v0 f0 local0 : ptr
    store t2 -> local2
    t3 = enum_wrap e0 v0 (local2) : enum0
    store t3 -> local3
    ret
  layout String size=16 align=8 refs=[]
  layout Int size=8 align=8 refs=[]
  layout Boolean size=1 align=1 refs=[]
  layout Option$S size=8 align=8 enum-refs=[[0], []]
  entry @scoop_main
";
        assert_eq!(lir::dump(&module), expected);
    }

    #[test]
    fn option_of_int_uses_the_tagged_representation() {
        // Option<Int>: the `{ i64 tag, [8 x i8] payload }` tagged
        // form — size 16, align 8.
        let module = lower(&option_round_trip("Option$I", mir::Type::Int));

        let expected = "\
Module
  enum Option$I tagged size=8 align=8 variants=(i64) ()
  fun @scoop_main() -> void
    local %0 o: enum0
    local %1 t: i64
    local %2 p: i64
    local %3 o2: enum0
  block entry
    t0 = enum_wrap e0 v1 () : enum0
    store t0 -> local0
    t1 = enum_tag e0 local0 : i64
    store t1 -> local1
    t2 = enum_field e0 v0 f0 local0 : i64
    store t2 -> local2
    t3 = enum_wrap e0 v0 (local2) : enum0
    store t3 -> local3
    ret
  layout String size=16 align=8 refs=[]
  layout Int size=8 align=8 refs=[]
  layout Boolean size=1 align=1 refs=[]
  layout Option$I size=16 align=8 enum-refs=[[], []]
  entry @scoop_main
";
        assert_eq!(lir::dump(&module), expected);
    }

    #[test]
    fn niche_detection_requires_exactly_one_pointer_payload() {
        let mut b = Builder::new();
        let option_s = b.option_enum("Option$S", mir::Type::String);
        let option_i = b.option_enum("Option$I", mir::Type::Int);
        // Reversed declaration order: the payload variant comes second.
        let flip = b.enums.alloc(mir::EnumDef {
            name: "Flip".to_string(),
            variants: vec![
                mir::VariantDef {
                    name: "Naught".to_string(),
                    fields: Vec::new(),
                },
                mir::VariantDef {
                    name: "Value".to_string(),
                    fields: vec![mir::Field {
                        name: "_1".to_string(),
                        ty: mir::Type::String,
                    }],
                },
            ],
        });
        // Two variants, but the payload variant has two fields: tagged.
        let pair_or_none = b.enums.alloc(mir::EnumDef {
            name: "PairOrNone".to_string(),
            variants: vec![
                mir::VariantDef {
                    name: "Pair".to_string(),
                    fields: vec![
                        mir::Field {
                            name: "_1".to_string(),
                            ty: mir::Type::Int,
                        },
                        mir::Field {
                            name: "_2".to_string(),
                            ty: mir::Type::Int,
                        },
                    ],
                },
                mir::VariantDef {
                    name: "Empty".to_string(),
                    fields: Vec::new(),
                },
            ],
        });
        let main = b.main(Arena::new(), Vec::new());
        let module = lower(&b.finish(main));

        assert!(matches!(
            edef(&module, option_s).repr,
            lir::EnumRepr::Niche { payload_variant: 0 }
        ));
        assert!(matches!(
            edef(&module, flip).repr,
            lir::EnumRepr::Niche { payload_variant: 1 }
        ));
        let lir::EnumRepr::Tagged {
            variants,
            payload_size,
            payload_align,
        } = &edef(&module, option_i).repr
        else {
            panic!("Option<Int> must use the tagged representation")
        };
        assert_eq!(variants.as_slice(), &[vec![lir::LirType::I64], Vec::new()]);
        assert_eq!((*payload_size, *payload_align), (8, 8));
        assert!(matches!(
            edef(&module, pair_or_none).repr,
            lir::EnumRepr::Tagged { .. }
        ));
    }

    #[test]
    fn tagged_enum_layout_records_per_variant_ref_offsets() {
        let mut b = Builder::new();
        // enum Msg { Text(String), Pair(Boolean, String), Empty }
        b.enums.alloc(mir::EnumDef {
            name: "Msg".to_string(),
            variants: vec![
                mir::VariantDef {
                    name: "Text".to_string(),
                    fields: vec![mir::Field {
                        name: "value".to_string(),
                        ty: mir::Type::String,
                    }],
                },
                mir::VariantDef {
                    name: "Pair".to_string(),
                    fields: vec![
                        mir::Field {
                            name: "flag".to_string(),
                            ty: mir::Type::Boolean,
                        },
                        mir::Field {
                            name: "s".to_string(),
                            ty: mir::Type::String,
                        },
                    ],
                },
                mir::VariantDef {
                    name: "Empty".to_string(),
                    fields: Vec::new(),
                },
            ],
        });
        // A niche enum inside a struct: the value itself is the
        // reference.
        let option_s = b.option_enum("Option$S", mir::Type::String);
        let _s = b.strukt(
            "S",
            &[("o", mir::Type::Enum(option_s, vec![mir::Type::String]))],
        );
        let main = b.main(Arena::new(), Vec::new());
        let module = lower(&b.finish(main));

        let by_name = |name: &str| {
            module
                .meta
                .layouts
                .iter()
                .find(|l| l.name == name)
                .unwrap_or_else(|| panic!("missing layout for {name}"))
        };

        // Msg: the largest variant is Pair (Boolean + String at
        // payload offsets 0 and 8), so the tagged value is 8 + 16
        // bytes at align 8.
        let msg_layout = by_name("Msg");
        assert_eq!((msg_layout.size, msg_layout.align), (24, 8));
        let lir::LayoutKind::Enum { variants } = &msg_layout.kind else {
            panic!("an enum layout keeps per-variant offsets")
        };
        // Text: the String at payload offset 0, absolute offset 8
        // (behind the 8-byte tag); Pair: the String at payload offset
        // 8, absolute 16; Empty: no references.
        assert_eq!(variants[0].ref_field_offsets, [8]);
        assert_eq!(variants[1].ref_field_offsets, [16]);
        assert!(variants[2].ref_field_offsets.is_empty());

        // The niche layout: the payload variant is the reference
        // itself; the unit variant has none.
        let option_layout = by_name("Option$S");
        assert_eq!((option_layout.size, option_layout.align), (8, 8));
        let lir::LayoutKind::Enum { variants } = &option_layout.kind else {
            panic!("an enum layout keeps per-variant offsets")
        };
        assert_eq!(variants[0].ref_field_offsets, [0]);
        assert!(variants[1].ref_field_offsets.is_empty());

        // S { o: Option<String> }: the niche value at offset 0 is the
        // struct's reference field.
        let s_layout = by_name("S");
        assert_eq!((s_layout.size, s_layout.align), (8, 8));
        assert_eq!(plain_refs(s_layout), [0]);
    }

    #[test]
    fn trap_calls_branch_to_a_shared_trap_block() {
        // fun f(o: Option<Int>): Int { return o!! + o!! } — in the
        // mir-lower shape: each `o!!` is `if (tag == Some) { val $uw =
        // field0 } else { trap(msg) }`.
        let mut b = Builder::new();
        let option_i = b.option_enum("Option$I", mir::Type::Int);
        let option_ty = mir::Type::Enum(option_i, vec![mir::Type::Int]);
        let message = b.string("unwrap on None (function f)");
        let mut locals = Arena::new();
        let o = locals.alloc(local("o", option_ty.clone()));
        let uw1 = locals.alloc(local("$uw.1", mir::Type::Int));
        let uw2 = locals.alloc(local("$uw.2", mir::Type::Int));
        let unwrap = |result: mir::LocalId| {
            stmt(mir::StatementKind::If {
                cond: binary(
                    mir::BinOp::IntEq,
                    mir::Expr::EnumTag(Box::new(mir::Expr::Local(o))),
                    mir::Expr::IntLiteral(0),
                ),
                then_body: vec![val_decl(
                    result,
                    mir::Expr::EnumField {
                        operand: Box::new(mir::Expr::Local(o)),
                        variant: 0,
                        index: 0,
                    },
                )],
                else_body: Some(vec![expr_stmt(runtime_call(
                    mir::RuntimeFn::Trap,
                    vec![mir::Expr::StringConst(message)],
                ))]),
            })
        };
        let f = b.user_fn_full(
            "f",
            "scoop.f",
            vec![param("o", option_ty, o)],
            mir::Type::Int,
            locals,
            vec![
                unwrap(uw1),
                unwrap(uw2),
                return_stmt(binary(
                    mir::BinOp::IntAdd,
                    mir::Expr::Local(uw1),
                    mir::Expr::Local(uw2),
                )),
            ],
        );
        let _ = f;
        let main = b.main(Arena::new(), Vec::new());
        let module = lower(&b.finish(main));

        // Both `!!` share the one trap block of the function.
        let expected = "\
Module
  global @scoop.str.0 = \"unwrap on None (function f)\"
  global @scoop.cstr.0 = c\"unwrap on None (function f)\"
  enum Option$I tagged size=8 align=8 variants=(i64) ()
  fun @scoop.f(enum0) -> i64
    local %0 $uw.1: i64
    local %1 $uw.2: i64
  block entry
    t0 = enum_tag e0 param0 : i64
    t1 = Eq t0, 0 : i1
    cbr t1 then @if.then.1 else @if.else.2
  block if.then.1
    t2 = enum_field e0 v0 f0 param0 : i64
    store t2 -> local0
    br @if.merge.3
  block if.else.2
    br @unwrap.trap.4
  block if.merge.3
    t3 = enum_tag e0 param0 : i64
    t4 = Eq t3, 0 : i1
    cbr t4 then @if.then.5 else @if.else.6
  block unwrap.trap.4
    call @scoop_rt_trap(global1)
    unreachable
  block if.then.5
    t5 = enum_field e0 v0 f0 param0 : i64
    store t5 -> local1
    br @if.merge.7
  block if.else.6
    br @unwrap.trap.4
  block if.merge.7
    t6 = Add local0, local1 : i64
    ret t6
  fun @scoop_main() -> void
  block entry
    ret
  layout String size=16 align=8 refs=[]
  layout Int size=8 align=8 refs=[]
  layout Boolean size=1 align=1 refs=[]
  layout Option$I size=16 align=8 enum-refs=[[], []]
  entry @scoop_main
";
        assert_eq!(lir::dump(&module), expected);
    }

    #[test]
    fn array_nodes_become_array_instructions() {
        // val a = [1, 2]; val x = a[0]; val n = a.size
        // val m = MutableArray(a); m[0] = 40
        let mut b = Builder::new();
        let array_int = mir::Type::Array(Box::new(mir::Type::Int));
        let mutable_int = mir::Type::MutableArray(Box::new(mir::Type::Int));
        let mut locals = Arena::new();
        let a = locals.alloc(local("a", array_int));
        let x = locals.alloc(local("x", mir::Type::Int));
        let n = locals.alloc(local("n", mir::Type::Int));
        let m = locals.alloc(local("m", mutable_int));
        let main = b.main(
            locals,
            vec![
                val_decl(
                    a,
                    mir::Expr::ArrayLiteral(vec![
                        mir::Expr::IntLiteral(1),
                        mir::Expr::IntLiteral(2),
                    ]),
                ),
                val_decl(
                    x,
                    mir::Expr::ArrayGet {
                        array: Box::new(mir::Expr::Local(a)),
                        index: Box::new(mir::Expr::IntLiteral(0)),
                    },
                ),
                val_decl(n, mir::Expr::ArrayLen(Box::new(mir::Expr::Local(a)))),
                val_decl(m, mir::Expr::ArrayClone(Box::new(mir::Expr::Local(a)))),
                stmt(mir::StatementKind::ArraySet {
                    array: mir::Expr::Local(m),
                    index: mir::Expr::IntLiteral(0),
                    value: mir::Expr::IntLiteral(40),
                }),
            ],
        );
        let module = lower(&b.finish(main));

        // Both array kinds map onto the same `LirType::Array` value
        // type; the layouts come after the M4 sections (builtins,
        // structs, enums, tuples).
        let expected = "\
Module
  fun @scoop_main() -> void
    local %0 a: [i64]
    local %1 x: i64
    local %2 n: i64
    local %3 m: [i64]
  block entry
    t0 = array_alloc (1, 2) : [i64]
    store t0 -> local0
    t1 = array_get local0 0 : i64
    store t1 -> local1
    t2 = array_len local0 : i64
    store t2 -> local2
    t3 = array_clone local0 : [i64]
    store t3 -> local3
    array_set local3 0 40
    ret
  layout String size=16 align=8 refs=[]
  layout Int size=8 align=8 refs=[]
  layout Boolean size=1 align=1 refs=[]
  layout Array<Int> size=8 align=8 array(element_is_ref=false)
  layout MutableArray<Int> size=8 align=8 array(element_is_ref=false)
  entry @scoop_main
";
        assert_eq!(lir::dump(&module), expected);
    }

    #[test]
    fn array_layouts_mark_reference_elements() {
        let mut b = Builder::new();
        let option_s = b.option_enum("Option$S", mir::Type::String);
        let point = b.strukt("Point", &[("x", mir::Type::Int), ("y", mir::Type::Int)]);
        let option_string = mir::Type::Enum(option_s, vec![mir::Type::String]);
        let array_int = mir::Type::Array(Box::new(mir::Type::Int));
        let mut locals = Arena::new();
        let _ints = locals.alloc(local("ints", array_int.clone()));
        let _strings = locals.alloc(local(
            "strings",
            mir::Type::Array(Box::new(mir::Type::String)),
        ));
        let _options = locals.alloc(local("options", mir::Type::Array(Box::new(option_string))));
        let _points = locals.alloc(local(
            "points",
            mir::Type::Array(Box::new(mir::Type::Struct(point))),
        ));
        let _nested = locals.alloc(local("nested", mir::Type::Array(Box::new(array_int))));
        let main = b.main(locals, vec![]);
        let module = lower(&b.finish(main));

        let array_layout = |name: &str| {
            let layout = module
                .meta
                .layouts
                .iter()
                .find(|l| l.name == name)
                .unwrap_or_else(|| panic!("missing layout for {name}"));
            let lir::LayoutKind::Array { element_is_ref } = layout.kind else {
                panic!("expected an array layout for {name}")
            };
            (layout.size, layout.align, element_is_ref)
        };
        // size / align are element-level: the element stride and
        // alignment of the region after header + size.
        assert_eq!(array_layout("Array<Int>"), (8, 8, false));
        // String elements are references.
        assert_eq!(array_layout("Array<String>"), (8, 8, true));
        // Option<String> uses the niche representation — a bare
        // pointer, hence a reference element.
        assert_eq!(array_layout("Array<Option$S<String>>"), (8, 8, true));
        // A value-type element is inline: the Point stride.
        assert_eq!(array_layout("Array<Point>"), (16, 8, false));
        // An array element is itself a reference; the nested element
        // type gets its own layout too.
        assert_eq!(array_layout("Array<Array<Int>>"), (8, 8, true));
    }

    #[test]
    fn array_fields_are_reference_fields() {
        let mut b = Builder::new();
        let _holder = b.strukt(
            "Holder",
            &[
                ("flag", mir::Type::Boolean),
                ("xs", mir::Type::Array(Box::new(mir::Type::Int))),
            ],
        );
        let main = b.main(Arena::new(), vec![]);
        let module = lower(&b.finish(main));

        // flag @0 (1 byte), xs @8: an array value is a pointer-sized
        // reference.
        let holder = module
            .meta
            .layouts
            .iter()
            .find(|l| l.name == "Holder")
            .expect("a layout per struct");
        assert_eq!((holder.size, holder.align), (16, 8));
        assert_eq!(plain_refs(holder), [8]);
        // The array type reachable from the struct field gets a layout
        // too, even though no code value mentions it.
        assert!(module.meta.layouts.iter().any(|l| l.name == "Array<Int>"));
    }

    // ---- M6: reference types ----

    /// The fixed `Any` vtable prefix, as mir-lower emits it.
    fn any_slots() -> Vec<mir::TableSlot> {
        vec![
            mir::TableSlot::Runtime(mir::RuntimeFn::AnyEquals),
            mir::TableSlot::Runtime(mir::RuntimeFn::AnyHashCode),
            mir::TableSlot::Runtime(mir::RuntimeFn::AnyToString),
        ]
    }

    #[test]
    fn virtual_calls_load_the_vtable_and_call_indirect() {
        let mut b = Builder::new();
        let c = b.class("C", None, &[], any_slots(), vec![]);
        // `C.m(this: C): Int { return 1 }`.
        let mut method_locals = Arena::new();
        let this = method_locals.alloc(local("this", mir::Type::Class(c)));
        let m = b.user_fn_full(
            "C.m",
            "scoop.C.m",
            vec![param("this", mir::Type::Class(c), this)],
            mir::Type::Int,
            method_locals,
            vec![return_stmt(mir::Expr::IntLiteral(1))],
        );
        // main: `val p: C; val r = p.m()` (a virtual call at slot 3).
        let mut locals = Arena::new();
        let p = locals.alloc(local("p", mir::Type::Class(c)));
        let r = locals.alloc(local("r", mir::Type::Int));
        let main = b.main(
            locals,
            vec![val_decl(
                r,
                mir::Expr::Call(mir::Call {
                    target: mir::CallTarget {
                        kind: mir::CallKind::Virtual { slot: 3 },
                        callee: mir::Callee::User(m),
                    },
                    args: vec![mir::Expr::Local(p)],
                }),
            )],
        );
        let module = lower(&b.finish(main));

        // The receiver's object header (index 0) holds the TD; its
        // vtable pointer is ScoopTypeDescriptor field 5; the callee is
        // vtable[3].
        let expected = "\
Module
  fun @scoop.C.m(ptr) -> i64
  block entry
    ret 1
  fun @scoop_main() -> void
    local %0 p: ptr
    local %1 r: i64
  block entry
    t0 = extract local0, 0 : ptr
    t1 = extract t0, 5 : ptr
    t2 = call_indirect t1[3](local0) : i64
    store t2 -> local1
    ret
  td C @scoop_td_C size=8 vtable=3 itables=0
  layout String size=16 align=8 refs=[]
  layout Int size=8 align=8 refs=[]
  layout Boolean size=1 align=1 refs=[]
  layout C size=8 align=8 refs=[]
  entry @scoop_main
";
        assert_eq!(lir::dump(&module), expected);
    }

    #[test]
    fn interface_calls_look_up_the_itable() {
        let mut b = Builder::new();
        let iface = b.interface("Describable", &["describe", "label"]);
        // The interface method shell (signature only, never emitted).
        let mut shell_locals = Arena::new();
        let this = shell_locals.alloc(local("this", mir::Type::Interface(iface)));
        let label = b.decl_fn(
            "Describable.label",
            "scoop.Describable.label",
            vec![param("this", mir::Type::Interface(iface), this)],
            mir::Type::Int,
        );
        // main: `val i: Describable; val r = i.label()` (itable slot 1).
        let mut locals = Arena::new();
        let i = locals.alloc(local("i", mir::Type::Interface(iface)));
        let r = locals.alloc(local("r", mir::Type::Int));
        let main = b.main(
            locals,
            vec![val_decl(
                r,
                mir::Expr::Call(mir::Call {
                    target: mir::CallTarget {
                        kind: mir::CallKind::Interface {
                            interface: iface,
                            slot: 1,
                        },
                        callee: mir::Callee::User(label),
                    },
                    args: vec![mir::Expr::Local(i)],
                }),
            )],
        );
        let module = lower(&b.finish(main));

        // `scoop_rt_itable_lookup(td, iface_td)` finds the table; the
        // interface TD is referenced through a globals-arena stub (the
        // TD itself comes from the meta — see the module docs).
        let expected = "\
Module
  global @scoop_td_Describable = c\"\"
  fun @scoop_main() -> void
    local %0 i: ptr
    local %1 r: i64
  block entry
    t0 = extract local0, 0 : ptr
    t1 = call @scoop_rt_itable_lookup(t0, global0) : ptr
    t2 = call_indirect t1[1](local0) : i64
    store t2 -> local1
    ret
  td Describable @scoop_td_Describable size=0 vtable=0 itables=0
  layout String size=16 align=8 refs=[]
  layout Int size=8 align=8 refs=[]
  layout Boolean size=1 align=1 refs=[]
  entry @scoop_main
";
        assert_eq!(lir::dump(&module), expected);
    }

    #[test]
    fn type_descriptors_carry_tables_parents_and_itables() {
        let mut b = Builder::new();
        let iface = b.interface("I", &["m"]);
        // Methods (bodies don't matter for the meta).
        let mut m_locals = Arena::new();
        let base_m_this = m_locals.alloc(local("this", mir::Type::Int));
        let base_m = b.user_fn_full(
            "Base.m",
            "scoop.Base.m",
            vec![param("this", mir::Type::Int, base_m_this)],
            mir::Type::Unit,
            m_locals,
            vec![],
        );
        let mut dm_locals = Arena::new();
        let derived_m_this = dm_locals.alloc(local("this", mir::Type::Int));
        let derived_m = b.user_fn_full(
            "Derived.m",
            "scoop.Derived.m",
            vec![param("this", mir::Type::Int, derived_m_this)],
            mir::Type::Unit,
            dm_locals,
            vec![],
        );
        let mut dm2_locals = Arena::new();
        let derived_m2_this = dm2_locals.alloc(local("this", mir::Type::Int));
        let derived_m2 = b.user_fn_full(
            "Derived.m2",
            "scoop.Derived.m2",
            vec![param("this", mir::Type::Int, derived_m2_this)],
            mir::Type::Unit,
            dm2_locals,
            vec![],
        );
        // Base implements I; Derived overrides `m` and adds `m2`.
        let mut base_vtable = any_slots();
        base_vtable.push(mir::TableSlot::Function(base_m));
        let base = b.class(
            "Base",
            None,
            &[("a", mir::Type::Int)],
            base_vtable,
            vec![mir::ItableRecord {
                interface: iface,
                slots: vec![mir::TableSlot::Function(base_m)],
            }],
        );
        let mut derived_vtable = any_slots();
        derived_vtable.push(mir::TableSlot::Function(derived_m));
        derived_vtable.push(mir::TableSlot::Function(derived_m2));
        let _derived = b.class(
            "Derived",
            Some(base),
            // mir-lower flattens the base prefix into the field list.
            &[("a", mir::Type::Int), ("b", mir::Type::String)],
            derived_vtable,
            vec![mir::ItableRecord {
                interface: iface,
                slots: vec![mir::TableSlot::Function(derived_m)],
            }],
        );
        let main = b.main(Arena::new(), vec![]);
        let module = lower(&b.finish(main));

        // Interfaces first (itable keys), then classes
        // base-before-derived — references always name
        // already-emitted entries.
        let tds = &module.meta.type_descriptors;
        assert_eq!(tds.len(), 3);
        let [i_td, base_td, derived_td] = &tds[..] else {
            panic!("expected three TypeDescriptors")
        };
        assert_eq!(i_td.symbol, "scoop_td_I");
        assert_eq!((i_td.size, i_td.align), (0, 0));
        assert!(i_td.parent.is_none());

        assert_eq!(base_td.name, "Base");
        assert_eq!(base_td.symbol, "scoop_td_Base");
        assert_eq!((base_td.size, base_td.align), (16, 8));
        assert!(base_td.ref_offsets.is_empty());
        assert!(base_td.parent.is_none());
        assert_eq!(
            base_td.vtable,
            [
                "scoop_rt_any_equals",
                "scoop_rt_any_hashcode",
                "scoop_rt_any_tostring",
                "scoop.Base.m",
            ]
        );
        assert_eq!(base_td.itables.len(), 1);
        assert_eq!(base_td.itables[0].interface_symbol, "scoop_td_I");
        assert_eq!(base_td.itables[0].slots, ["scoop.Base.m"]);

        assert_eq!(derived_td.symbol, "scoop_td_Derived");
        assert_eq!(derived_td.parent.as_deref(), Some("scoop_td_Base"));
        // header 8 + Int @8 + String @16 → size 24; the String is the
        // one reference.
        assert_eq!((derived_td.size, derived_td.align), (24, 8));
        assert_eq!(derived_td.ref_offsets, [16]);
        assert_eq!(
            derived_td.vtable,
            [
                "scoop_rt_any_equals",
                "scoop_rt_any_hashcode",
                "scoop_rt_any_tostring",
                "scoop.Derived.m",
                "scoop.Derived.m2",
            ]
        );
        assert_eq!(derived_td.itables[0].slots, ["scoop.Derived.m"]);
    }

    #[test]
    fn class_layouts_shift_ref_offsets_by_the_header() {
        let mut b = Builder::new();
        let c = b.class(
            "C",
            None,
            &[
                ("a", mir::Type::Int),
                ("s", mir::Type::String),
                ("flag", mir::Type::Boolean),
                ("r", mir::Type::Any),
            ],
            any_slots(),
            vec![],
        );
        let _ = c;
        // A boxed value type: header + the inline payload; references
        // inside the payload shift by the header too.
        let s = b.strukt("S", &[("x", mir::Type::Int), ("s", mir::Type::String)]);
        let _boxed = b.class(
            "box$S",
            None,
            &[("value", mir::Type::Struct(s))],
            any_slots(),
            vec![],
        );
        let main = b.main(Arena::new(), vec![]);
        let module = lower(&b.finish(main));

        let by_name = |name: &str| {
            module
                .meta
                .layouts
                .iter()
                .find(|l| l.name == name)
                .unwrap_or_else(|| panic!("missing layout for {name}"))
        };
        // C: header 8; a @8, s @16, flag @24, r @32 → size 40.
        let c_layout = by_name("C");
        assert_eq!((c_layout.size, c_layout.align), (40, 8));
        assert_eq!(plain_refs(c_layout), [16, 32]);
        // box$S: header 8 + payload { Int @0, String @8 } @8 → the
        // String lands at 16.
        let boxed_layout = by_name("box$S");
        assert_eq!((boxed_layout.size, boxed_layout.align), (24, 8));
        assert_eq!(plain_refs(boxed_layout), [16]);
        // The TypeDescriptors carry the same reference offsets.
        let td = |name: &str| {
            module
                .meta
                .type_descriptors
                .iter()
                .find(|td| td.name == name)
                .unwrap_or_else(|| panic!("missing TypeDescriptor for {name}"))
        };
        assert_eq!(td("C").ref_offsets, [16, 32]);
        assert_eq!(td("box$S").ref_offsets, [16]);
        assert!(td("C").parent.is_none());
        assert!(td("box$S").parent.is_none());
    }

    #[test]
    fn box_unbox_and_is_instance_lower_to_runtime_calls() {
        let mut b = Builder::new();
        let s = b.strukt("S", &[("x", mir::Type::Int)]);
        // mir-lower registers the boxed class of every checked / boxed
        // value type.
        let _boxed = b.class(
            "box$S",
            None,
            &[("value", mir::Type::Struct(s))],
            any_slots(),
            vec![],
        );
        let mut locals = Arena::new();
        let a = locals.alloc(local("a", mir::Type::Any));
        let v = locals.alloc(local("v", mir::Type::Struct(s)));
        let chk = locals.alloc(local("chk", mir::Type::Boolean));
        let main = b.main(
            locals,
            vec![
                val_decl(
                    a,
                    mir::Expr::Box(Box::new(mir::Expr::StructInit {
                        struct_id: s,
                        args: vec![mir::Expr::IntLiteral(1)],
                    })),
                ),
                val_decl(v, mir::Expr::Unbox(Box::new(mir::Expr::Local(a)))),
                val_decl(
                    chk,
                    mir::Expr::IsInstance {
                        operand: Box::new(mir::Expr::Local(a)),
                        check_ty: Box::new(mir::Type::Struct(s)),
                    },
                ),
            ],
        );
        let module = lower(&b.finish(main));

        // Box → `scoop_rt_box(td, payload, size)`; Unbox → the payload
        // field behind the header; `is` → `scoop_rt_is_instance(obj,
        // td)`. Both checks share the one TD stub global.
        let expected = "\
Module
  global @scoop_td_box$S = c\"\"
  fun @scoop_main() -> void
    local %0 a: ptr
    local %1 v: {i64}
    local %2 chk: i1
  block entry
    t0 = aggregate (1) : {i64}
    t1 = call @scoop_rt_box(global0, t0, 8) : ptr
    store t1 -> local0
    t2 = extract local0, 1 : {i64}
    store t2 -> local1
    t3 = call @scoop_rt_is_instance(local0, global0) : i1
    store t3 -> local2
    ret
  td box$S @scoop_td_box$S size=16 vtable=3 itables=0
  layout String size=16 align=8 refs=[]
  layout Int size=8 align=8 refs=[]
  layout Boolean size=1 align=1 refs=[]
  layout S size=8 align=8 refs=[]
  layout box$S size=16 align=8 refs=[]
  entry @scoop_main
";
        assert_eq!(lir::dump(&module), expected);
    }

    #[test]
    fn class_field_reads_are_heap_loads() {
        let mut b = Builder::new();
        let c = b.class(
            "C",
            None,
            &[("a", mir::Type::Int), ("s", mir::Type::String)],
            any_slots(),
            vec![],
        );
        let mut locals = Arena::new();
        let p = locals.alloc(local("p", mir::Type::Class(c)));
        let s = locals.alloc(local("s", mir::Type::String));
        let main = b.main(
            locals,
            vec![val_decl(
                s,
                mir::Expr::FieldAccess {
                    receiver: Box::new(mir::Expr::Local(p)),
                    index: 1,
                },
            )],
        );
        let module = lower(&b.finish(main));

        // Field 1 of the raw `{ ptr header, a, s }` object struct is
        // at index 2 (the header is index 0).
        let function = &module.functions[0];
        let instructions = &function.blocks[function.entry].instructions;
        let lir::Instruction::ExtractValue { out, index, .. } = &instructions[0] else {
            panic!("a class field read must be a heap object load")
        };
        assert_eq!(*index, 2);
        assert_eq!(function.temps[*out].ty, lir::LirType::Ptr);
    }

    #[test]
    fn class_init_allocates_and_stores_fields() {
        // The ctor body mir-lower generates for
        // `class Point(val x: Int, val s: String)`:
        // `return ClassInit Point [x, s]` — allocation plus one heap
        // store per flattened field.
        let mut b = Builder::new();
        let str_x = b.string("x");
        let point = b.class(
            "Point",
            None,
            &[("x", mir::Type::Int), ("s", mir::Type::String)],
            any_slots(),
            vec![],
        );
        let mut ctor_locals = Arena::new();
        let x = ctor_locals.alloc(local("x", mir::Type::Int));
        let s = ctor_locals.alloc(local("s", mir::Type::String));
        let ctor = b.user_fn_full(
            "ctor.Point",
            "scoop.ctor.Point",
            vec![
                param("x", mir::Type::Int, x),
                param("s", mir::Type::String, s),
            ],
            mir::Type::Class(point),
            ctor_locals,
            vec![return_stmt(mir::Expr::ClassInit {
                class_id: point,
                args: vec![mir::Expr::Local(x), mir::Expr::Local(s)],
            })],
        );
        let mut locals = Arena::new();
        let p = locals.alloc(local("p", mir::Type::Class(point)));
        let main = b.main(
            locals,
            vec![val_decl(
                p,
                mir::Expr::Call(mir::Call {
                    target: mir::CallTarget {
                        kind: mir::CallKind::Direct,
                        callee: mir::Callee::User(ctor),
                    },
                    args: vec![mir::Expr::IntLiteral(1), mir::Expr::StringConst(str_x)],
                }),
            )],
        );
        let module = lower(&b.finish(main));

        // `scoop_rt_alloc(td, size)` with the class layout size (8
        // header + Int @8 + String @16 = 24), then the fields at the
        // raw object indices 1 and 2.
        let expected = "\
Module
  global @scoop.str.0 = \"x\"
  global @scoop_td_Point = c\"\"
  fun @scoop.ctor.Point(i64, ptr) -> ptr
  block entry
    t0 = call @scoop_rt_alloc(global1, 24) : ptr
    heap_store t0 1 param0
    heap_store t0 2 param1
    ret t0
  fun @scoop_main() -> void
    local %0 p: ptr
  block entry
    t0 = call @scoop.ctor.Point(1, global0) : ptr
    store t0 -> local0
    ret
  td Point @scoop_td_Point size=24 vtable=3 itables=0
  layout String size=16 align=8 refs=[]
  layout Int size=8 align=8 refs=[]
  layout Boolean size=1 align=1 refs=[]
  layout Point size=24 align=8 refs=[16]
  entry @scoop_main
";
        assert_eq!(lir::dump(&module), expected);
    }

    #[test]
    fn field_set_lowers_to_a_heap_store() {
        // `p.y = 3`: MIR FieldSet index 1 → heap store at the raw
        // object index 2 (the header is index 0).
        let mut b = Builder::new();
        let c = b.class(
            "C",
            None,
            &[("x", mir::Type::Int), ("y", mir::Type::Int)],
            any_slots(),
            vec![],
        );
        let mut locals = Arena::new();
        let p = locals.alloc(local("p", mir::Type::Class(c)));
        let main = b.main(
            locals,
            vec![stmt(mir::StatementKind::FieldSet {
                object: mir::Expr::Local(p),
                index: 1,
                value: mir::Expr::IntLiteral(3),
            })],
        );
        let module = lower(&b.finish(main));

        let function = &module.functions[0];
        let instructions = &function.blocks[function.entry].instructions;
        let lir::Instruction::HeapStore {
            object,
            index: 2,
            value,
        } = &instructions[0]
        else {
            panic!("a FieldSet must lower to a HeapStore")
        };
        assert!(matches!(object, lir::Value::Local(_)));
        assert!(matches!(value, lir::Value::IntConst(3)));
    }

    #[test]
    fn a_trap_only_body_seals_the_function() {
        // mir-lower's abstract-method stub is a single trap call: the
        // block is sealed by the trap branch, so the "non-Unit
        // functions must end with `return`" check must not fire (it
        // applies to hir-lower-produced bodies that fall off the end,
        // not to noreturn bodies like this one).
        let mut b = Builder::new();
        let message = b.string("call to abstract method `Base.id`");
        let mut locals = Arena::new();
        let this = locals.alloc(local("this", mir::Type::Any));
        let _stub = b.user_fn_full(
            "Base.id",
            "scoop.Base.id",
            vec![param("this", mir::Type::Any, this)],
            mir::Type::Int,
            locals,
            vec![expr_stmt(runtime_call(
                mir::RuntimeFn::Trap,
                vec![mir::Expr::StringConst(message)],
            ))],
        );
        let main = b.main(Arena::new(), vec![]);
        let module = lower(&b.finish(main));

        let function = &module.functions[0];
        assert!(matches!(
            function.blocks[function.entry].terminator,
            lir::Terminator::Br(_)
        ));
    }
}
