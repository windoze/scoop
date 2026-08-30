//! LIR stage: type layout, exception lowering. LIR contains nothing
//! Scoop-specific and is mechanically translatable to the target IR.
//! (Impl spec 2.4's statepoint insertion is applied one stage later,
//! in codegen — see the M9 note below.)
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
//! representation. Enum layouts in the
//! meta keep recursive per-variant scan programs — scanning an enum
//! value depends on its tag and composes when the enum is nested in an
//! aggregate (runtime spec 2.2).
//!
//! M5: arrays (docs/milestone5/DESIGN.md 2.4). Both array kinds map
//! onto `LirType::Array` — a pointer to `{ td, gc_word, i64 size,
//! inline elements }`; mutability is compile-time only. The MIR array nodes
//! become the `ArrayAlloc` / `ArrayGet` / `ArrayLen` / `ArraySet` /
//! `ArrayClone` instructions (bounds checks and the clone's runtime
//! call are codegen's job). Every array type appearing in the module
//! gets a meta layout (after the tuple layouts) whose size / align are
//! element-level — the element stride and alignment of the region
//! after header + size — plus a recursive scan program for one inline
//! element. This preserves references inside structs, tuples, and
//! tagged enums without boxing.
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
//! Two instruction-level conventions carry the M6 lowerings:
//!
//! - `HeapLoad` / `HeapStore` carry byte offsets, not field indices.
//!   Class fields therefore follow their natural alignment after the
//!   16-byte object header, including consecutive sub-word fields.
//!   Fixed runtime metadata uses the same load primitive: offset 0 is
//!   an object's TypeDescriptor pointer and offset 40 is the vtable
//!   pointer in `ScoopTypeDescriptor`.
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
//!
//! M8: exceptions (docs/milestone8/DESIGN.md section 3.4). A
//! structured `mir::StatementKind::Try` becomes basic blocks: inside
//! the body every user call / dispatch that may throw is emitted as
//! `Invoke` / `InvokeIndirect` to the innermost try's unwind block
//! (a per-function stack tracks the pads, so nested trys unwind to
//! their own pad and catch / finally code unwinds to the enclosing
//! one). The unwind block starts with `LandingPad`, spills its ABI
//! record/raw pointer, then an ordinary dispatch block performs
//! `BeginCatch` and the `scoop_rt_is_instance` decision chain.
//! Handler/exit pads balance the active catch and forward the same
//! record into an enclosing dispatch/cleanup continuation (or resume
//! out of the function). An exception no catch matches invokes
//! `scoop_rt_rethrow` through that chain. `finally` is inlined on every
//! path — normal completion, after each catch body, before the
//! rethrow, and before a `return` out of the body or a catch (the
//! copies are duplicated per exit site: a shared block cannot carry
//! the per-site return value without a phi). A `throw` outside any
//! try is the `Throw` instruction; inside a try it is invoked to the
//! current pad (a plain call would unwind straight past the
//! function's own landing pad).
//!
//! Block-terminator convention: `Invoke` / `InvokeIndirect` must be
//! the last instruction of its block; the block's own `terminator`
//! is the redundant `Br(normal)` — it only restates the invoke's
//! normal successor for dump readability, and codegen uses the
//! instruction as the LLVM terminator without emitting the branch.
//! `Throw` is noreturn: its block ends `Unreachable` (the same shape
//! as the M3 trap path). Personality is a
//! function-level implicit marker (the personality convention with
//! codegen): every function containing a `LandingPad` instruction
//! gets `scoop_eh_personality` — codegen derives the flag from the
//! instruction, so no separate field exists. Functions without a try
//! are unaffected: their calls stay plain `Call`s.
//!
//! M9: the 16-byte object header (milestone9 DESIGN section 0). Every
//! heap object is `{ ptr td, u64 gc_word, ... }` — the GC's mark / pin
//! word sits between the TypeDescriptor pointer and the payload. Class
//! fields start at naturally aligned byte offsets from 16; the boxed
//! payload and array size are at byte 16 (elements start at 24), and
//! the String length is at byte 16 (bytes at 24). The layout math below
//! counts the header as 16 bytes; `scoop_rt_alloc` writes both header words (the TD from
//! its argument, a zeroed GC word), so no lowering stores the header.
//! The statepoint side of M9 (the GC strategy, safepoint polls, and
//! the stackmap emission of impl spec 2.4's statepoint insertion) is
//! applied in codegen — it is an instrumentation of the emitted LLVM,
//! and keeping it out of LIR keeps these dumps stable.

use std::collections::HashMap;

use la_arena::Arena;
use scoop_lir as lir;

/// Runtime object allocation: `ptr scoop_rt_alloc(ptr td, i64 size)`
/// (runtime spec 2.1). `ClassInit` lowerings call it.
const ALLOC_SYMBOL: &str = "scoop_rt_alloc";
/// Runtime throw entry: `void scoop_rt_throw(ptr)` (runtime spec 5).
/// A `throw` inside a `try` is invoked to the current landing pad
/// through this symbol; outside a `try` the `Throw` instruction is
/// used instead (both lower to `__cxa_throw` in codegen / the C
/// runtime). Noreturn.
const THROW_SYMBOL: &str = "scoop_rt_throw";
/// Runtime rethrow entry: `void scoop_rt_rethrow(void)` — resumes
/// unwinding of the active exception (`__cxa_rethrow`) when no catch
/// of the current try matches. Noreturn.
const RETHROW_SYMBOL: &str = "scoop_rt_rethrow";
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
        | mir::Type::UInt
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
        let (size, align, scan) = class_layout(module, enums, def);
        layouts.push(lir::Layout {
            name: def.name.clone(),
            size,
            align,
            kind: lir::LayoutKind::Plain { scan },
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

/// The runtime `String` object layout (runtime spec 2.4): the 16-byte
/// object header (TD pointer + GC word, M9) + `len` (u64, 8 bytes) at
/// offset 16. The string data is variable-length and not counted in
/// `size`.
fn string_layout() -> lir::Layout {
    lir::Layout {
        name: "String".to_string(),
        size: 24,
        align: 8,
        kind: lir::LayoutKind::Plain {
            scan: lir::RefScan::None,
        },
    }
}

fn scalar_layout(name: &str, size: u64, align: u64) -> lir::Layout {
    lir::Layout {
        name: name.to_string(),
        size,
        align,
        kind: lir::LayoutKind::Plain {
            scan: lir::RefScan::None,
        },
    }
}

/// Layout of an aggregate value (struct / tuple / Unit): fields in
/// declaration order at their natural alignment. The recursive scan
/// program preserves references nested in aggregates and tagged enums.
fn aggregate_layout(
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
        kind: lir::LayoutKind::Plain { scan },
    }
}

/// Layout of an enum value: niche form is a bare pointer; tagged form
/// records one recursive scan program per variant.
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
                        scan: if variant.fields.is_empty() {
                            lir::RefScan::None
                        } else {
                            lir::RefScan::References(vec![0])
                        },
                    })
                    .collect(),
            },
        },
        lir::EnumRepr::Tagged { .. } => {
            let (size, align) = repr_shape(&enums[enum_def_id(id)].repr);
            let payload_offset = enum_payload_offset(&enums[enum_def_id(id)].repr);
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
                    lir::VariantLayout {
                        scan: scan_fields(module, enums, &field_types, &offsets, payload_offset),
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

/// Layout of an array object (M5, DESIGN 2.4): `{ td, gc_word, i64
/// size, inline elements }`. The object is variable-length, so `size`
/// / `align` here are element-level information: the element stride
/// (element size rounded up to its alignment) and alignment of the
/// element region that follows the 16-byte header (M9) + 8-byte size
/// field. `element_scan` recursively describes one inline element.
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
            element_scan: ref_scan(module, enums, element, 0),
        },
    }
}

/// Layout of a class object (M6, runtime spec 2.1/2.2): the 16-byte
/// object header (M9: TD pointer + GC word) followed by the fields
/// — mir-lower already flattened the base-class prefix into
/// `ClassDef::fields`. Returns size, align, and the reference offsets
/// relative to the object start (the header itself is not a scanned
/// reference). Boxed value types use the same shape: header + the
/// inline payload field.
fn class_layout(
    module: &mir::Module,
    enums: &Arena<lir::EnumDef>,
    def: &mir::ClassDef,
) -> (u64, u64, lir::RefScan) {
    let (offsets, size, align) = class_shape(module, enums, def);
    let fields: Vec<mir::Type> = def.fields.iter().map(|field| field.ty.clone()).collect();
    let scan = scan_fields(module, enums, &fields, &offsets, 0);
    (size, align, scan)
}

/// Natural object layout for one flattened class: the header occupies
/// bytes 0..16 and each base/derived field starts at the next address
/// satisfying its own alignment. LIR heap operations consume these
/// byte offsets directly, so sub-word fields are not rounded to slots.
fn class_shape(
    module: &mir::Module,
    enums: &Arena<lir::EnumDef>,
    def: &mir::ClassDef,
) -> (Vec<u64>, u64, u64) {
    let enum_shape = |id: mir::EnumId| repr_shape(&enums[enum_def_id(id)].repr);
    let mut offsets = Vec::with_capacity(def.fields.len());
    let mut size = 16u64;
    let mut align = 8u64;
    for field in &def.fields {
        let (field_size, field_align) = size_align(module, &enum_shape, &field.ty);
        let offset = size.next_multiple_of(field_align);
        offsets.push(offset);
        size = offset + field_size;
        align = align.max(field_align);
    }
    (offsets, size.next_multiple_of(align), align)
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
            scan: lir::RefScan::None,
            parent: None,
            vtable: Vec::new(),
            itables: Vec::new(),
        });
    }
    for id in class_order(module) {
        let def = &module.classes[id];
        let (size, align, scan) = class_layout(module, enums, def);
        tds.push(lir::TypeDescriptor {
            name: def.name.clone(),
            symbol: td_symbol(&def.name),
            size,
            align,
            scan,
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
        // UInt shares Int's machine word (M9, spec 11.2).
        mir::Type::Int | mir::Type::UInt => (8, 8),
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

/// Payload byte offset of a tagged enum value.
fn enum_payload_offset(repr: &lir::EnumRepr) -> u64 {
    match repr {
        lir::EnumRepr::Niche { .. } => 0,
        lir::EnumRepr::Tagged { payload_align, .. } => 8u64.max(*payload_align),
    }
}

/// Normalize a list of scans: remove empty parts, flatten sequences,
/// and merge plain reference lists. Tagged branches remain explicit.
fn sequence(parts: impl IntoIterator<Item = lir::RefScan>) -> lir::RefScan {
    let mut refs = Vec::new();
    let mut conditional = Vec::new();
    for part in parts {
        match part {
            lir::RefScan::None => {}
            lir::RefScan::References(offsets) => refs.extend(offsets),
            lir::RefScan::Sequence(parts) => {
                for nested in parts {
                    match nested {
                        lir::RefScan::None => {}
                        lir::RefScan::References(offsets) => refs.extend(offsets),
                        other => conditional.push(other),
                    }
                }
            }
            other => conditional.push(other),
        }
    }
    if !refs.is_empty() {
        conditional.insert(0, lir::RefScan::References(refs));
    }
    match conditional.len() {
        0 => lir::RefScan::None,
        1 => conditional.pop().expect("one scan part"),
        _ => lir::RefScan::Sequence(conditional),
    }
}

/// Scan program for `fields` laid out at `offsets`, shifted by `base`.
fn scan_fields(
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
fn ref_scan(
    module: &mir::Module,
    enums: &Arena<lir::EnumDef>,
    ty: &mir::Type,
    base: u64,
) -> lir::RefScan {
    match ty {
        mir::Type::String | mir::Type::Class(_) | mir::Type::Interface(_) | mir::Type::Any => {
            lir::RefScan::References(vec![base])
        }
        mir::Type::Array(_) | mir::Type::MutableArray(_) => lir::RefScan::References(vec![base]),
        mir::Type::Struct(id) => {
            let fields: Vec<mir::Type> = module.structs[*id]
                .fields
                .iter()
                .map(|field| field.ty.clone())
                .collect();
            let enum_shape = |id: mir::EnumId| repr_shape(&enums[enum_def_id(id)].repr);
            let (offsets, _, _) = aggregate_shape(module, &enum_shape, &fields);
            scan_fields(module, enums, &fields, &offsets, base)
        }
        mir::Type::Tuple(fields) => {
            let enum_shape = |id: mir::EnumId| repr_shape(&enums[enum_def_id(id)].repr);
            let (offsets, _, _) = aggregate_shape(module, &enum_shape, fields);
            scan_fields(module, enums, fields, &offsets, base)
        }
        mir::Type::Enum(id, _) => match &enums[enum_def_id(*id)].repr {
            lir::EnumRepr::Niche { .. } => lir::RefScan::References(vec![base]),
            repr @ lir::EnumRepr::Tagged { .. } => {
                let payload_base = base + enum_payload_offset(repr);
                let enum_shape = |id: mir::EnumId| repr_shape(&enums[enum_def_id(id)].repr);
                let variants: Vec<lir::RefScan> = module.enums[*id]
                    .variants
                    .iter()
                    .map(|variant| {
                        let fields: Vec<mir::Type> = variant
                            .fields
                            .iter()
                            .map(|field| field.ty.clone())
                            .collect();
                        let (offsets, _, _) = aggregate_shape(module, &enum_shape, &fields);
                        scan_fields(module, enums, &fields, &offsets, payload_base)
                    })
                    .collect();
                if variants.iter().all(|scan| *scan == lir::RefScan::None) {
                    lir::RefScan::None
                } else {
                    lir::RefScan::TaggedEnum {
                        tag_offset: base,
                        variants,
                    }
                }
            }
        },
        mir::Type::Unit | mir::Type::Int | mir::Type::UInt | mir::Type::Boolean => {
            lir::RefScan::None
        }
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
        // UInt shares Int's machine word (M9, spec 11.2): the same
        // `i64` at LIR, so codegen needs no UInt-specific handling.
        mir::Type::Int | mir::Type::UInt => lir::LirType::I64,
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
fn lower_function<'a>(
    module: &'a mir::Module,
    function: &'a mir::Function,
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
        try_stack: Vec::new(),
        exception_slots: None,
        return_cleanups: Vec::new(),
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
        | mir::Type::UInt
        | mir::Type::Boolean
        | mir::Type::Unit => td_symbol(&format!("box${}", mir::encode_type(module, ty))),
        mir::Type::Any | mir::Type::Array(_) | mir::Type::MutableArray(_) => {
            unreachable!("no referenceable TypeDescriptor for {ty:?}")
        }
    }
}

#[derive(Clone)]
enum ReturnCleanup<'a> {
    /// Run a finally body. `owner_unwind` is removed from the active
    /// try stack while the copy is lowered, so it cannot catch an
    /// exception thrown by its own finally.
    Finally {
        owner_unwind: lir::BlockId,
        body: &'a [mir::Statement],
    },
    /// Balance the catch started by a `LandingPad`. Removing its
    /// cleanup pad from `try_stack` ensures later outer finally bodies
    /// cannot end the same catch twice if they throw.
    EndCatch { cleanup_pad: lir::BlockId },
}

/// An LLVM unwind destination plus the ordinary block that consumes
/// the exception record captured there. Cleanup chains branch to the
/// continuation directly after saving a replacement exception in the
/// shared EH locals; fresh invokes target `pad`.
#[derive(Clone, Copy)]
struct UnwindTarget {
    pad: lir::BlockId,
    continuation: lir::BlockId,
    /// Whether the continuation chain eventually reaches a catch
    /// dispatch in this function. Such pads need a catch-all clause;
    /// chains ending in `resume` use cleanup-only pads.
    handles_in_function: bool,
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
    /// The unwind blocks of the enclosing trys, innermost last (M8):
    /// a call that may throw inside a try body is invoked to the
    /// innermost landing pad. Empty outside try bodies — calls stay
    /// plain there.
    try_stack: Vec<UnwindTarget>,
    /// Function-local spill slots shared by all landing pads. They
    /// avoid phi nodes when an inner catch cleanup forwards the same
    /// exception to an enclosing try's dispatch block.
    exception_slots: Option<(lir::LocalId, lir::LocalId)>,
    /// Structured cleanups for `return`, innermost last (M8): finally
    /// copies and active catches are interleaved in lexical order, so
    /// nested handlers execute `end_catch; finally` from inside out.
    return_cleanups: Vec<ReturnCleanup<'a>>,
}

impl<'a> FunctionLowerer<'a> {
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

    fn exception_slots(&mut self) -> (lir::LocalId, lir::LocalId) {
        if let Some(slots) = self.exception_slots {
            return slots;
        }
        let record = self.new_hidden_local(lir::LirType::ExceptionRecord);
        let raw = self.new_hidden_local(lir::LirType::Ptr);
        let slots = (record, raw);
        self.exception_slots = Some(slots);
        slots
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
                mir::Callee::Monomorphized(id) => {
                    let function = self.module.meta.instances[id].function;
                    self.module.functions[function].return_ty.clone()
                }
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
                    // The test-only GC hooks have fixed types.
                    mir::RuntimeFn::GcCollect => mir::Type::Unit,
                    mir::RuntimeFn::GcStats => mir::Type::UInt,
                    // mir-lower routes the handle intrinsics through
                    // context-typed positions only (the wrapping
                    // StructInit's argument, a typed hidden local), so
                    // their type is never reconstructed here.
                    mir::RuntimeFn::Pin
                    | mir::RuntimeFn::Unpin
                    | mir::RuntimeFn::GetHandle
                    | mir::RuntimeFn::ReleaseHandle => {
                        unreachable!("{function:?} results are typed by the mir-lower context")
                    }
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

    fn lower_statements(&mut self, statements: &'a [mir::Statement]) {
        for statement in statements {
            self.lower_statement(statement);
        }
    }

    fn lower_statement(&mut self, statement: &'a mir::Statement) {
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
            // `obj.field = value`: MIR carries the flattened field
            // index; LIR fixes it to the class layout's byte offset.
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
                let (offsets, _, _) =
                    class_shape(self.module, self.enums, &self.module.classes[*class_id]);
                let offset = offsets[*index as usize];
                let object = self.lower_expr(object, &object_ty);
                let value = self.lower_expr(value, &field_ty);
                self.push(lir::Instruction::HeapStore {
                    object,
                    offset,
                    value,
                });
            }
            mir::StatementKind::Return { value } => {
                let mut value = match (self.returns_void, value) {
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
                if !self.return_cleanups.is_empty() {
                    // `return` inside a try/finally or an active catch
                    // (M8, DESIGN 3.4): the value is already evaluated
                    // above; now every cleanup runs innermost first.
                    // A `Local` value is stashed first — a finally may
                    // assign to that local, but the returned value must
                    // be the one evaluated here.
                    if let Some(lir::Value::Local(_)) = value {
                        let return_ty = self.mir_return_ty.clone();
                        let ty = self.value_type(&return_ty);
                        let stash = self.new_hidden_local(ty);
                        self.push(lir::Instruction::Store {
                            local: stash,
                            value: value.expect("a value is being stashed"),
                        });
                        value = Some(lir::Value::Local(stash));
                    }
                    self.emit_return_cleanups();
                    // A `return` inside a finally copy wins: the block
                    // is sealed and this return is dropped.
                    if self.current_sealed {
                        return;
                    }
                }
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
            mir::StatementKind::Try(try_) => self.lower_try(try_),
            // `throw` (M8, DESIGN 3.4): outside a try this is the
            // `Throw` instruction — codegen calls the runtime throw
            // entry and the block ends `unreachable`. Inside a try the
            // throw must reach this function's own landing pad (a
            // plain call would unwind straight past it), so it is
            // invoked to the innermost pad; the runtime entry never
            // returns, so the normal successor is unreachable.
            mir::StatementKind::Throw(expr) => {
                let ty = self.expr_ty(expr);
                let value = self.lower_expr(expr, &ty);
                match self.try_stack.last() {
                    Some(target) => {
                        let unwind = target.pad;
                        let normal = self.new_block("throw.normal");
                        self.push(lir::Instruction::Invoke {
                            out: None,
                            symbol: THROW_SYMBOL.to_string(),
                            args: vec![value],
                            normal,
                            unwind,
                        });
                        self.seal(lir::Terminator::Br(normal));
                        self.enter(normal);
                        // The runtime throw entry never returns, so the
                        // normal successor is dead.
                        self.seal(lir::Terminator::Unreachable);
                        self.current_sealed = true;
                    }
                    None => {
                        self.push(lir::Instruction::Throw { exception: value });
                        self.seal(lir::Terminator::Unreachable);
                        self.current_sealed = true;
                    }
                }
            }
        }
    }

    /// `try` / `catch` / `finally` (M8, DESIGN 3.4). While the body is
    /// lowered its unwind block tops `try_stack`, so every potentially
    /// throwing operation in the body becomes an `Invoke` /
    /// `InvokeIndirect` to it (see `finish_call`); nested trys push
    /// their own pad. The pad captures the ABI record/raw pointer;
    /// dispatch begins the catch and matches clauses in declaration
    /// order with the `scoop_rt_is_instance` chain (the same shape as
    /// `when`). An exception no catch matches is rethrown.
    ///
    /// `finally` runs on every path, inlined per exit site: once
    /// after normal completion of the body, once after each catch
    /// body, and once before the rethrow; a `return` out of the body
    /// or a catch is covered by the `Return` arm through
    /// `return_cleanups`. Catch bodies and their finally copies unwind
    /// through handler/exit pads that end the active catch; a catch
    /// body exit also runs finally before forwarding, while a throw
    /// from finally and a rethrow use an exit chain that cannot re-run it.
    fn lower_try(&mut self, try_: &'a mir::Try) {
        let enclosing = self.try_stack.last().copied();
        let (record_slot, raw_slot) = self.exception_slots();
        let unwind = self.new_block("try.unwind");
        let dispatch = self.new_block("try.dispatch");
        let handler_target = if try_.catches.is_empty() {
            None
        } else {
            let pad = self.new_block("try.handler_pad");
            let continuation = self.new_block("try.handler_cleanup");
            Some(UnwindTarget {
                pad,
                continuation,
                handles_in_function: enclosing.is_some_and(|target| target.handles_in_function),
            })
        };
        let exit_target = UnwindTarget {
            pad: self.new_block("try.exit_pad"),
            continuation: self.new_block("try.exit_cleanup"),
            handles_in_function: enclosing.is_some_and(|target| target.handles_in_function),
        };
        let end = self.new_block("try.end");
        let own_target = UnwindTarget {
            pad: unwind,
            continuation: dispatch,
            handles_in_function: true,
        };

        self.try_stack.push(own_target);
        if let Some(finally) = &try_.finally_body {
            self.return_cleanups.push(ReturnCleanup::Finally {
                owner_unwind: unwind,
                body: finally,
            });
        }
        self.lower_statements(&try_.body);
        self.try_stack.pop();
        let finally = try_.finally_body.as_deref();
        if finally.is_some() {
            self.return_cleanups.pop();
        }

        let mut end_reachable = false;
        // Normal path: inline finally once, then continue after the
        // try.
        if !self.current_sealed {
            if let Some(finally) = finally {
                self.lower_statements(finally);
            }
            if !self.current_sealed {
                self.seal(lir::Terminator::Br(end));
                end_reachable = true;
            }
        }

        // Exception path: capture into function-local EH slots, then
        // begin the catch in an ordinary block. Inner cleanup chains
        // can branch to the same dispatch after replacing those slots.
        self.enter(unwind);
        let record = self.new_temp(lir::LirType::ExceptionRecord);
        let raw = self.new_temp(lir::LirType::Ptr);
        self.push(lir::Instruction::LandingPad { record, raw });
        self.push(lir::Instruction::Store {
            local: record_slot,
            value: lir::Value::Temp(record),
        });
        self.push(lir::Instruction::Store {
            local: raw_slot,
            value: lir::Value::Temp(raw),
        });
        self.seal(lir::Terminator::Br(dispatch));

        self.enter(dispatch);
        let exception = self.new_temp(lir::LirType::Ptr);
        self.push(lir::Instruction::BeginCatch {
            out: exception,
            raw: lir::Value::Local(raw_slot),
        });
        let exception = lir::Value::Temp(exception);
        for catch in &try_.catches {
            let handler_target = handler_target.expect("a catch has a handler cleanup");
            let catch_block = self.new_block("try.catch");
            let next = self.new_block("try.next");
            let cond = self.new_temp(lir::LirType::I1);
            let catch_td = self.td_ref(&catch.ty);
            self.push(lir::Instruction::Call {
                out: Some(cond),
                symbol: mir::RuntimeFn::IsInstance.symbol().to_string(),
                args: vec![exception, catch_td],
            });
            self.seal(lir::Terminator::CondBr {
                cond: lir::Value::Temp(cond),
                then_block: catch_block,
                else_block: next,
            });
            self.enter(catch_block);
            self.push(lir::Instruction::Store {
                local: self.local_slot(catch.local),
                value: exception,
            });
            // Calls/throws in the catch body unwind through a cleanup
            // that ends this catch, runs finally, then resumes.
            self.try_stack.push(handler_target);
            let cleanup_base = self.return_cleanups.len();
            // Return cleanup order is LIFO: end the catch first, then
            // run its finally with only the enclosing try active.
            if let Some(finally) = finally {
                self.return_cleanups.push(ReturnCleanup::Finally {
                    owner_unwind: unwind,
                    body: finally,
                });
            }
            self.return_cleanups.push(ReturnCleanup::EndCatch {
                cleanup_pad: handler_target.pad,
            });
            self.lower_statements(&catch.body);
            self.return_cleanups.truncate(cleanup_base);
            self.try_stack.pop();
            if !self.current_sealed {
                self.push(lir::Instruction::EndCatch);
                if let Some(finally) = finally {
                    self.lower_statements(finally);
                }
                if !self.current_sealed {
                    self.seal(lir::Terminator::Br(end));
                    end_reachable = true;
                }
            }
            self.enter(next);
        }
        // No catch matched: finally, then rethrow the active
        // exception. A return or a new exception from the finally must
        // still end the catch begun by LandingPad.
        self.try_stack.push(exit_target);
        let cleanup_base = self.return_cleanups.len();
        self.return_cleanups.push(ReturnCleanup::EndCatch {
            cleanup_pad: exit_target.pad,
        });
        if let Some(finally) = finally {
            self.lower_statements(finally);
        }
        self.return_cleanups.truncate(cleanup_base);
        if !self.current_sealed {
            let normal = self.new_block("rethrow.normal");
            self.push(lir::Instruction::Invoke {
                out: None,
                symbol: RETHROW_SYMBOL.to_string(),
                args: Vec::new(),
                normal,
                unwind: exit_target.pad,
            });
            self.seal(lir::Terminator::Br(normal));
            self.enter(normal);
            self.seal(lir::Terminator::Unreachable);
            self.current_sealed = true;
        }
        self.try_stack.pop();

        // A catch-body exception first lands in a pad that captures the
        // replacement record, then its ordinary continuation ends the
        // old catch and runs finally. The captured exception is passed
        // to the enclosing continuation without changing identity.
        if let Some(handler_target) = handler_target {
            self.enter(handler_target.pad);
            let handler_record = self.new_temp(lir::LirType::ExceptionRecord);
            let handler_raw = self.new_temp(lir::LirType::Ptr);
            if handler_target.handles_in_function {
                self.push(lir::Instruction::LandingPad {
                    record: handler_record,
                    raw: handler_raw,
                });
            } else {
                self.push(lir::Instruction::CleanupPad {
                    record: handler_record,
                    raw: handler_raw,
                });
            }
            self.push(lir::Instruction::Store {
                local: record_slot,
                value: lir::Value::Temp(handler_record),
            });
            self.push(lir::Instruction::Store {
                local: raw_slot,
                value: lir::Value::Temp(handler_raw),
            });
            self.seal(lir::Terminator::Br(handler_target.continuation));

            self.enter(handler_target.continuation);
            self.push(lir::Instruction::EndCatch);
            if let Some(finally) = finally {
                self.lower_statements(finally);
            }
            if !self.current_sealed {
                match enclosing {
                    Some(target) => self.seal(lir::Terminator::Br(target.continuation)),
                    None => self.seal(lir::Terminator::Resume {
                        exception: lir::Value::Local(record_slot),
                    }),
                }
            }
        }

        // No-match rethrow and exceptions from a no-match finally have
        // already run (or are replacing) that finally: capture the
        // record, end the active catch, then continue the enclosing
        // cleanup/dispatch chain.
        self.enter(exit_target.pad);
        let exit_record = self.new_temp(lir::LirType::ExceptionRecord);
        let exit_raw = self.new_temp(lir::LirType::Ptr);
        if exit_target.handles_in_function {
            self.push(lir::Instruction::LandingPad {
                record: exit_record,
                raw: exit_raw,
            });
        } else {
            self.push(lir::Instruction::CleanupPad {
                record: exit_record,
                raw: exit_raw,
            });
        }
        self.push(lir::Instruction::Store {
            local: record_slot,
            value: lir::Value::Temp(exit_record),
        });
        self.push(lir::Instruction::Store {
            local: raw_slot,
            value: lir::Value::Temp(exit_raw),
        });
        self.seal(lir::Terminator::Br(exit_target.continuation));

        self.enter(exit_target.continuation);
        self.push(lir::Instruction::EndCatch);
        match enclosing {
            Some(target) => self.seal(lir::Terminator::Br(target.continuation)),
            None => self.seal(lir::Terminator::Resume {
                exception: lir::Value::Local(record_slot),
            }),
        }

        self.enter(end);
        if !end_reachable {
            // Every path through the try leaves the function (all
            // branches return or throw), so the merge block is dead.
            // Seal it: without this the function-end fallback would
            // append a bare `return` on a path that must not exist.
            self.seal(lir::Terminator::Unreachable);
            self.current_sealed = true;
        }
    }

    /// Emit every pending return cleanup, innermost first. Finally
    /// bodies are duplicated per exit site because a shared block
    /// cannot carry the per-site return value without a phi. While a
    /// cleanup is lowered the stack contains only the remaining outer
    /// actions, so a `return` inside a finally does not recurse into
    /// that same copy. EndCatch actions also remove their cleanup pad
    /// from `try_stack`, preserving the lexical nesting of handlers.
    fn emit_return_cleanups(&mut self) {
        let all = std::mem::take(&mut self.return_cleanups);
        let saved_try_stack = self.try_stack.clone();
        let mut remaining = all.clone();
        while let Some(cleanup) = remaining.pop() {
            if self.current_sealed {
                break;
            }
            self.return_cleanups = remaining.clone();
            match cleanup {
                ReturnCleanup::Finally { owner_unwind, body } => {
                    if let Some(pos) = self
                        .try_stack
                        .iter()
                        .rposition(|target| target.pad == owner_unwind)
                    {
                        self.try_stack.truncate(pos);
                    }
                    self.lower_statements(body);
                }
                ReturnCleanup::EndCatch { cleanup_pad } => {
                    self.push(lir::Instruction::EndCatch);
                    let active = self.try_stack.pop();
                    assert_eq!(
                        active.map(|target| target.pad),
                        Some(cleanup_pad),
                        "active catch cleanup nesting"
                    );
                }
            }
        }
        self.return_cleanups = all;
        self.try_stack = saved_try_stack;
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
            // then one heap store per flattened field (the header is
            // followed by naturally aligned fields at fixed byte
            // offsets).
            mir::Expr::ClassInit { class_id, args } => {
                let def = &self.module.classes[*class_id];
                let (field_offsets, size, _) = class_shape(self.module, self.enums, def);
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
                for ((arg, field_ty), offset) in args.iter().zip(&field_types).zip(field_offsets) {
                    let value = self.lower_expr(arg, field_ty);
                    self.push(lir::Instruction::HeapStore {
                        object: lir::Value::Temp(out),
                        offset,
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
                let element_scan = ref_scan(self.module, self.enums, &element_ty, 0);
                self.push(lir::Instruction::ArrayAlloc {
                    out,
                    elements,
                    element_scan,
                });
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
                let receiver = self.lower_expr(receiver, &receiver_ty);
                let out_ty = self.value_type(ty);
                let out = if let mir::Type::Class(class_id) = receiver_ty {
                    let (offsets, _, _) =
                        class_shape(self.module, self.enums, &self.module.classes[class_id]);
                    self.load_at_offset(receiver, offsets[*index as usize], out_ty)
                } else {
                    let out = self.new_temp(out_ty);
                    self.push(lir::Instruction::ExtractValue {
                        out,
                        aggregate: receiver,
                        index: *index,
                    });
                    out
                };
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
            // The payload sits right behind the 16-byte object header:
            // byte offset 16 of the boxed object (see the module docs).
            mir::Expr::Unbox(operand) => {
                let object = self.lower_expr(operand, &mir::Type::Any);
                let ty = self.value_type(ty);
                let out = self.load_at_offset(object, 16, ty);
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
            mir::Callee::User(_) | mir::Callee::Monomorphized(_) => {
                let id = match call.target.callee {
                    mir::Callee::User(id) => id,
                    mir::Callee::Monomorphized(instance) => {
                        self.module.meta.instances[instance].function
                    }
                    mir::Callee::Runtime(_) => unreachable!("matched a local callee above"),
                };
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
                        let td = self.load_at_offset(args[0], 0, lir::LirType::Ptr);
                        let vtable =
                            self.load_at_offset(lir::Value::Temp(td), 5 * 8, lir::LirType::Ptr);
                        self.finish_indirect(vtable, slot, args, returns_unit, result_ty)
                    }
                    // itable dispatch: `scoop_rt_itable_lookup(td,
                    // iface_td)` finds the interface's table by its
                    // TypeDescriptor key.
                    mir::CallKind::Interface { interface, slot } => {
                        let td = self.load_at_offset(args[0], 0, lir::LirType::Ptr);
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
                    // The GC intrinsics (M9, runtime spec 3.4): the
                    // pin / handle operations speak raw machine words
                    // — the object reference in, the word out (or the
                    // reverse); the hooks take nothing.
                    mir::RuntimeFn::Pin | mir::RuntimeFn::GetHandle => vec![mir::Type::Any],
                    mir::RuntimeFn::Unpin | mir::RuntimeFn::ReleaseHandle => {
                        vec![mir::Type::UInt]
                    }
                    mir::RuntimeFn::GcCollect | mir::RuntimeFn::GcStats => Vec::new(),
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
                    // The pin / handle intrinsics exchange a word with
                    // the runtime: `pin` / `getGcHandle` yield the raw
                    // word (i64), `unpin` / `releaseGcHandle` yield the
                    // reference (ptr), `gcStats` yields the count.
                    mir::RuntimeFn::Pin | mir::RuntimeFn::GetHandle | mir::RuntimeFn::GcStats => {
                        self.call_with_result(symbol, args, lir::LirType::I64)
                    }
                    mir::RuntimeFn::Unpin | mir::RuntimeFn::ReleaseHandle => {
                        self.call_with_result(symbol, args, lir::LirType::Ptr)
                    }
                    mir::RuntimeFn::Write | mir::RuntimeFn::GcCollect => {
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

    /// Load a value of `ty` at a fixed byte offset from a raw pointer.
    fn load_at_offset(&mut self, object: lir::Value, offset: u64, ty: lir::LirType) -> lir::TempId {
        let out = self.new_temp(ty);
        self.push(lir::Instruction::HeapLoad {
            out,
            object,
            offset,
        });
        out
    }

    /// A direct call: Unit-returning callees are void at the LLVM
    /// level; their Unit value is a fresh empty aggregate. Inside a
    /// try body the call may throw, so it is invoked to the innermost
    /// landing pad (M8): the `Invoke` ends the block (the terminator
    /// convention in the module docs) and the result is available in
    /// the normal successor.
    fn finish_call(
        &mut self,
        symbol: String,
        args: Vec<lir::Value>,
        returns_unit: bool,
        result_ty: &mir::Type,
    ) -> lir::Value {
        if let Some(target) = self.try_stack.last() {
            let unwind = target.pad;
            let normal = self.new_block("invoke.normal");
            let out = if returns_unit {
                None
            } else {
                let ty = self.value_type(result_ty);
                Some(self.new_temp(ty))
            };
            self.push(lir::Instruction::Invoke {
                out,
                symbol,
                args,
                normal,
                unwind,
            });
            self.seal(lir::Terminator::Br(normal));
            self.enter(normal);
            return match out {
                Some(temp) => lir::Value::Temp(temp),
                None => self.unit_value(),
            };
        }
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
    /// dispatch, impl spec 2.9). Inside a try body it is invoked to
    /// the innermost landing pad, like `finish_call`.
    fn finish_indirect(
        &mut self,
        table: lir::TempId,
        slot: u32,
        args: Vec<lir::Value>,
        returns_unit: bool,
        result_ty: &mir::Type,
    ) -> lir::Value {
        if let Some(target) = self.try_stack.last() {
            let unwind = target.pad;
            let normal = self.new_block("invoke.normal");
            let out = if returns_unit {
                None
            } else {
                let ty = self.value_type(result_ty);
                Some(self.new_temp(ty))
            };
            self.push(lir::Instruction::InvokeIndirect {
                out,
                table: lir::Value::Temp(table),
                slot,
                args,
                normal,
                unwind,
            });
            self.seal(lir::Terminator::Br(normal));
            self.enter(normal);
            return match out {
                Some(temp) => lir::Value::Temp(temp),
                None => self.unit_value(),
            };
        }
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
        let lir::LayoutKind::Plain { scan } = &layout.kind else {
            panic!("expected a plain layout")
        };
        match scan {
            lir::RefScan::None => &[],
            lir::RefScan::References(offsets) => offsets,
            other => panic!("expected a flat plain scan, found {other:?}"),
        }
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
  layout String size=24 align=8 refs=[]
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
  layout String size=24 align=8 refs=[]
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
  layout String size=24 align=8 refs=[]
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
  layout String size=24 align=8 refs=[]
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
  layout String size=24 align=8 refs=[]
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
    fn uint_shares_ints_machine_word() {
        // UInt (spec 11.2, M9) maps onto `i64` at LIR — the same
        // machine word as Int, so codegen needs no UInt-specific
        // handling.
        let mut b = Builder::new();
        // A UInt field in a class: an 8-byte scalar slot behind the
        // 16-byte header, exactly like an Int field.
        let _c = b.class("C", None, &[("u", mir::Type::UInt)], any_slots(), vec![]);
        let mut locals = Arena::new();
        let u = locals.alloc(local("u", mir::Type::UInt));
        let main = b.main(locals, vec![val_decl(u, mir::Expr::IntLiteral(1))]);
        let module = lower(&b.finish(main));

        let function = &module.functions[0];
        let (_, u_local) = function.locals.iter().next().expect("one local");
        assert_eq!(u_local.ty, lir::LirType::I64);

        let c_layout = module
            .meta
            .layouts
            .iter()
            .find(|l| l.name == "C")
            .expect("a layout per class");
        assert_eq!((c_layout.size, c_layout.align), (24, 8));
        assert!(plain_refs(c_layout).is_empty());
        let c_td = module
            .meta
            .type_descriptors
            .iter()
            .find(|td| td.name == "C")
            .expect("a TypeDescriptor per class");
        assert_eq!(c_td.size, 24);
        assert_eq!(c_td.scan, lir::RefScan::None);
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
        assert_eq!((string.size, string.align), (24, 8));
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
  layout String size=24 align=8 refs=[]
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
    t0 = heap_load param0 +16 : i64
    t1 = call @scoop_rt_int_to_string(t0) : ptr
    ret t1
  fun @scoop_main() -> void
  block entry
    ret
  layout String size=24 align=8 refs=[]
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
  layout String size=24 align=8 refs=[]
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
  layout String size=24 align=8 refs=[]
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
  layout String size=24 align=8 refs=[]
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
    fn recursive_scans_preserve_tagged_enums_in_aggregates_and_arrays() {
        let mut b = Builder::new();
        // enum Msg { Text(String), Pair(Boolean, String), Empty }
        let msg = b.enums.alloc(mir::EnumDef {
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
        let msg_ty = mir::Type::Enum(msg, Vec::new());
        // Nested { flag: Boolean @0, msg: Msg @8 }. The conditional
        // scan must retain the enum's tag rather than flattening all
        // variant payload offsets into unconditional references.
        let nested = b.strukt(
            "Nested",
            &[("flag", mir::Type::Boolean), ("msg", msg_ty.clone())],
        );
        // Holder { head: String @16, nested: Nested @24 } combines an
        // unconditional reference with the nested enum scan.
        b.class(
            "Holder",
            None,
            &[
                ("head", mir::Type::String),
                ("nested", mir::Type::Struct(nested)),
            ],
            any_slots(),
            vec![],
        );
        let mut locals = Arena::new();
        locals.alloc(local(
            "messages",
            mir::Type::Array(Box::new(msg_ty.clone())),
        ));
        locals.alloc(local(
            "nestedValues",
            mir::Type::Array(Box::new(mir::Type::Struct(nested))),
        ));
        let main = b.main(locals, Vec::new());
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
        assert_eq!(variants[0].scan, lir::RefScan::References(vec![8]));
        assert_eq!(variants[1].scan, lir::RefScan::References(vec![16]));
        assert_eq!(variants[2].scan, lir::RefScan::None);

        // The niche layout: the payload variant is the reference
        // itself; the unit variant has none.
        let option_layout = by_name("Option$S");
        assert_eq!((option_layout.size, option_layout.align), (8, 8));
        let lir::LayoutKind::Enum { variants } = &option_layout.kind else {
            panic!("an enum layout keeps per-variant offsets")
        };
        assert_eq!(variants[0].scan, lir::RefScan::References(vec![0]));
        assert_eq!(variants[1].scan, lir::RefScan::None);

        // S { o: Option<String> }: the niche value at offset 0 is the
        // struct's reference field.
        let s_layout = by_name("S");
        assert_eq!((s_layout.size, s_layout.align), (8, 8));
        assert_eq!(plain_refs(s_layout), [0]);

        let nested_layout = by_name("Nested");
        assert_eq!((nested_layout.size, nested_layout.align), (32, 8));
        assert_eq!(
            nested_layout.kind,
            lir::LayoutKind::Plain {
                scan: lir::RefScan::TaggedEnum {
                    tag_offset: 8,
                    variants: vec![
                        lir::RefScan::References(vec![16]),
                        lir::RefScan::References(vec![24]),
                        lir::RefScan::None,
                    ],
                },
            }
        );

        let holder_td = module
            .meta
            .type_descriptors
            .iter()
            .find(|td| td.name == "Holder")
            .expect("Holder TypeDescriptor");
        assert_eq!((holder_td.size, holder_td.align), (56, 8));
        assert_eq!(
            holder_td.scan,
            lir::RefScan::Sequence(vec![
                lir::RefScan::References(vec![16]),
                lir::RefScan::TaggedEnum {
                    tag_offset: 32,
                    variants: vec![
                        lir::RefScan::References(vec![40]),
                        lir::RefScan::References(vec![48]),
                        lir::RefScan::None,
                    ],
                },
            ])
        );

        let array_scan = |name: &str| {
            let lir::LayoutKind::Array { element_scan } = &by_name(name).kind else {
                panic!("expected array layout for {name}")
            };
            element_scan.clone()
        };
        assert_eq!(
            array_scan("Array<Msg>"),
            lir::RefScan::TaggedEnum {
                tag_offset: 0,
                variants: vec![
                    lir::RefScan::References(vec![8]),
                    lir::RefScan::References(vec![16]),
                    lir::RefScan::None,
                ],
            }
        );
        assert_eq!(
            array_scan("Array<Nested>"),
            lir::RefScan::TaggedEnum {
                tag_offset: 8,
                variants: vec![
                    lir::RefScan::References(vec![16]),
                    lir::RefScan::References(vec![24]),
                    lir::RefScan::None,
                ],
            }
        );
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
  layout String size=24 align=8 refs=[]
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
  layout String size=24 align=8 refs=[]
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
            let lir::LayoutKind::Array { element_scan } = &layout.kind else {
                panic!("expected an array layout for {name}")
            };
            (layout.size, layout.align, element_scan.clone())
        };
        // size / align are element-level: the element stride and
        // alignment of the region after header + size.
        assert_eq!(array_layout("Array<Int>"), (8, 8, lir::RefScan::None));
        // String elements are references.
        assert_eq!(
            array_layout("Array<String>"),
            (8, 8, lir::RefScan::References(vec![0]))
        );
        // Option<String> uses the niche representation — a bare
        // pointer, hence a reference element.
        assert_eq!(
            array_layout("Array<Option$S<String>>"),
            (8, 8, lir::RefScan::References(vec![0]))
        );
        // A value-type element is inline: the Point stride.
        assert_eq!(array_layout("Array<Point>"), (16, 8, lir::RefScan::None));
        // An array element is itself a reference; the nested element
        // type gets its own layout too.
        assert_eq!(
            array_layout("Array<Array<Int>>"),
            (8, 8, lir::RefScan::References(vec![0]))
        );
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
    t0 = heap_load local0 +0 : ptr
    t1 = heap_load t0 +40 : ptr
    t2 = call_indirect t1[3](local0) : i64
    store t2 -> local1
    ret
  td C @scoop_td_C size=16 vtable=3 itables=0
  layout String size=24 align=8 refs=[]
  layout Int size=8 align=8 refs=[]
  layout Boolean size=1 align=1 refs=[]
  layout C size=16 align=8 refs=[]
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
    t0 = heap_load local0 +0 : ptr
    t1 = call @scoop_rt_itable_lookup(t0, global0) : ptr
    t2 = call_indirect t1[1](local0) : i64
    store t2 -> local1
    ret
  td Describable @scoop_td_Describable size=0 vtable=0 itables=0
  layout String size=24 align=8 refs=[]
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
        // 16-byte header + Int @16 → size 24.
        assert_eq!((base_td.size, base_td.align), (24, 8));
        assert_eq!(base_td.scan, lir::RefScan::None);
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
        // header 16 + Int @16 + String @24 → size 32; the String is
        // the one reference.
        assert_eq!((derived_td.size, derived_td.align), (32, 8));
        assert_eq!(derived_td.scan, lir::RefScan::References(vec![24]));
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
        // C: header 16; a @16, s @24, flag @32, r @40 → size 48.
        let c_layout = by_name("C");
        assert_eq!((c_layout.size, c_layout.align), (48, 8));
        assert_eq!(plain_refs(c_layout), [24, 40]);
        // box$S: header 16 + payload { Int @0, String @8 } @16 → the
        // String lands at 24.
        let boxed_layout = by_name("box$S");
        assert_eq!((boxed_layout.size, boxed_layout.align), (32, 8));
        assert_eq!(plain_refs(boxed_layout), [24]);
        // The TypeDescriptors carry the same reference offsets.
        let td = |name: &str| {
            module
                .meta
                .type_descriptors
                .iter()
                .find(|td| td.name == name)
                .unwrap_or_else(|| panic!("missing TypeDescriptor for {name}"))
        };
        assert_eq!(td("C").scan, lir::RefScan::References(vec![24, 40]));
        assert_eq!(td("box$S").scan, lir::RefScan::References(vec![24]));
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
    t2 = heap_load local0 +16 : {i64}
    store t2 -> local1
    t3 = call @scoop_rt_is_instance(local0, global0) : i1
    store t3 -> local2
    ret
  td box$S @scoop_td_box$S size=24 vtable=3 itables=0
  layout String size=24 align=8 refs=[]
  layout Int size=8 align=8 refs=[]
  layout Boolean size=1 align=1 refs=[]
  layout S size=8 align=8 refs=[]
  layout box$S size=24 align=8 refs=[]
  entry @scoop_main
";
        assert_eq!(lir::dump(&module), expected);
    }

    #[test]
    fn gc_intrinsics_exchange_words_with_the_runtime() {
        // The MIR shapes mir-lower produces for the M9 GC intrinsics
        // (milestone9 DESIGN section 1): `pin` / `getGcHandle` wrap
        // the runtime's raw word into the handle struct, `unpin` /
        // `releaseGcHandle` unwrap field 0 for the reverse call, and
        // the hooks are a void call / an i64 result.
        let mut b = Builder::new();
        let pin_handle = b.strukt("PinHandle$S", &[("raw", mir::Type::UInt)]);
        let gc_handle = b.strukt("GcHandle$S", &[("raw", mir::Type::UInt)]);
        let mut locals = Arena::new();
        let v = locals.alloc(local("v", mir::Type::String));
        let h = locals.alloc(local("h", mir::Type::Struct(pin_handle)));
        let gc1 = locals.alloc(local("$gc.1", mir::Type::String));
        let p = locals.alloc(local("p", mir::Type::String));
        let gh = locals.alloc(local("gh", mir::Type::Struct(gc_handle)));
        let gc2 = locals.alloc(local("$gc.2", mir::Type::String));
        let p2 = locals.alloc(local("p2", mir::Type::String));
        let n = locals.alloc(local("n", mir::Type::UInt));
        let main = b.main(
            locals,
            vec![
                val_decl(
                    h,
                    mir::Expr::StructInit {
                        struct_id: pin_handle,
                        args: vec![runtime_call(mir::RuntimeFn::Pin, vec![mir::Expr::Local(v)])],
                    },
                ),
                val_decl(
                    gc1,
                    runtime_call(
                        mir::RuntimeFn::Unpin,
                        vec![mir::Expr::FieldAccess {
                            receiver: Box::new(mir::Expr::Local(h)),
                            index: 0,
                        }],
                    ),
                ),
                val_decl(p, mir::Expr::Local(gc1)),
                val_decl(
                    gh,
                    mir::Expr::StructInit {
                        struct_id: gc_handle,
                        args: vec![runtime_call(
                            mir::RuntimeFn::GetHandle,
                            vec![mir::Expr::Local(v)],
                        )],
                    },
                ),
                val_decl(
                    gc2,
                    runtime_call(
                        mir::RuntimeFn::ReleaseHandle,
                        vec![mir::Expr::FieldAccess {
                            receiver: Box::new(mir::Expr::Local(gh)),
                            index: 0,
                        }],
                    ),
                ),
                val_decl(p2, mir::Expr::Local(gc2)),
                expr_stmt(runtime_call(mir::RuntimeFn::GcCollect, vec![])),
                val_decl(n, runtime_call(mir::RuntimeFn::GcStats, vec![])),
            ],
        );
        let module = lower(&b.finish(main));

        let expected = "\
Module
  fun @scoop_main() -> void
    local %0 v: ptr
    local %1 h: {i64}
    local %2 $gc.1: ptr
    local %3 p: ptr
    local %4 gh: {i64}
    local %5 $gc.2: ptr
    local %6 p2: ptr
    local %7 n: i64
  block entry
    t0 = call @scoop_rt_pin(local0) : i64
    t1 = aggregate (t0) : {i64}
    store t1 -> local1
    t2 = extract local1, 0 : i64
    t3 = call @scoop_rt_unpin(t2) : ptr
    store t3 -> local2
    store local2 -> local3
    t4 = call @scoop_rt_get_handle(local0) : i64
    t5 = aggregate (t4) : {i64}
    store t5 -> local4
    t6 = extract local4, 0 : i64
    t7 = call @scoop_rt_release_handle(t6) : ptr
    store t7 -> local5
    store local5 -> local6
    call @scoop_rt_gc_collect()
    t8 = aggregate () : {}
    t9 = call @scoop_rt_gc_stats() : i64
    store t9 -> local7
    ret
  layout String size=24 align=8 refs=[]
  layout Int size=8 align=8 refs=[]
  layout Boolean size=1 align=1 refs=[]
  layout PinHandle$S size=8 align=8 refs=[]
  layout GcHandle$S size=8 align=8 refs=[]
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

        // The String field follows the 16-byte header and Int field,
        // so its natural byte offset is 24.
        let function = &module.functions[0];
        let instructions = &function.blocks[function.entry].instructions;
        let lir::Instruction::HeapLoad { out, offset, .. } = &instructions[0] else {
            panic!("a class field read must be a heap object load")
        };
        assert_eq!(*offset, 24);
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

        // `scoop_rt_alloc(td, size)` with the class layout size (16
        // header + Int @16 + String @24 = 32), then the fields at
        // those byte offsets.
        let expected = "\
Module
  global @scoop.str.0 = \"x\"
  global @scoop_td_Point = c\"\"
  fun @scoop.ctor.Point(i64, ptr) -> ptr
  block entry
    t0 = call @scoop_rt_alloc(global1, 32) : ptr
    heap_store t0 +16 param0
    heap_store t0 +24 param1
    ret t0
  fun @scoop_main() -> void
    local %0 p: ptr
  block entry
    t0 = call @scoop.ctor.Point(1, global0) : ptr
    store t0 -> local0
    ret
  td Point @scoop_td_Point size=32 vtable=3 itables=0
  layout String size=24 align=8 refs=[]
  layout Int size=8 align=8 refs=[]
  layout Boolean size=1 align=1 refs=[]
  layout Point size=32 align=8 refs=[24]
  entry @scoop_main
";
        assert_eq!(lir::dump(&module), expected);
    }

    #[test]
    fn field_set_lowers_to_a_heap_store() {
        // `p.y = 3`: MIR FieldSet index 1 → byte offset 24 after the
        // 16-byte header and the first Int field.
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
            offset: 24,
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

    /// `try { throw e } catch (e: MyError) { handled() }` minus the
    /// throw — the shared shell of the M8 tests: `helper()` in the
    /// body, `handled()` in the catch, `cleanup()` in the finally.
    fn try_shell(finally: bool) -> (Builder, mir::FunctionId, mir::FunctionId, mir::FunctionId) {
        let mut b = Builder::new();
        let helper = b.user_fn("helper", "scoop.helper", Arena::new(), vec![]);
        let handled = b.user_fn("handled", "scoop.handled", Arena::new(), vec![]);
        let cleanup = if finally {
            b.user_fn("cleanup", "scoop.cleanup", Arena::new(), vec![])
        } else {
            helper
        };
        (b, helper, handled, cleanup)
    }

    fn my_error(b: &mut Builder) -> mir::ClassId {
        b.class("MyError", None, &[], vec![], vec![])
    }

    #[test]
    fn try_catch_lowers_to_invoke_landingpad_and_rethrow() {
        let (mut b, helper, handled, _) = try_shell(false);
        let my_error = my_error(&mut b);
        let mut locals = Arena::new();
        let e = locals.alloc(local("e", mir::Type::Class(my_error)));
        let main = b.main(
            locals,
            vec![stmt(mir::StatementKind::Try(mir::Try {
                body: vec![expr_stmt(user_call(helper))],
                catches: vec![mir::CatchClause {
                    local: e,
                    ty: Box::new(mir::Type::Class(my_error)),
                    body: vec![expr_stmt(user_call(handled))],
                    span: SPAN,
                }],
                finally_body: None,
            }))],
        );
        let module = lower(&b.finish(main));

        let expected = "\
Module
  global @scoop_td_MyError = c\"\"
  fun @scoop.helper() -> void
  block entry
    ret
  fun @scoop.handled() -> void
  block entry
    ret
  fun @scoop_main() -> void
    local %0 e: ptr
    local %1 $sc.1: exception_record
    local %2 $sc.2: ptr
  block entry
    invoke @scoop.helper() normal @invoke.normal.8 unwind @try.unwind.1
    br @invoke.normal.8
  block try.unwind.1
    (t1, t2) = landingpad : (exception_record, ptr)
    store t1 -> local1
    store t2 -> local2
    br @try.dispatch.2
  block try.dispatch.2
    t3 = begin_catch local2 : ptr
    t4 = call @scoop_rt_is_instance(t3, global0) : i1
    cbr t4 then @try.catch.9 else @try.next.10
  block try.handler_pad.3
    (t6, t7) = cleanup_pad : (exception_record, ptr)
    store t6 -> local1
    store t7 -> local2
    br @try.handler_cleanup.4
  block try.handler_cleanup.4
    end_catch
    resume local1
  block try.exit_pad.5
    (t8, t9) = cleanup_pad : (exception_record, ptr)
    store t8 -> local1
    store t9 -> local2
    br @try.exit_cleanup.6
  block try.exit_cleanup.6
    end_catch
    resume local1
  block try.end.7
    ret
  block invoke.normal.8
    t0 = aggregate () : {}
    br @try.end.7
  block try.catch.9
    store t3 -> local0
    invoke @scoop.handled() normal @invoke.normal.11 unwind @try.handler_pad.3
    br @invoke.normal.11
  block try.next.10
    invoke @scoop_rt_rethrow() normal @rethrow.normal.12 unwind @try.exit_pad.5
    br @rethrow.normal.12
  block invoke.normal.11
    t5 = aggregate () : {}
    end_catch
    br @try.end.7
  block rethrow.normal.12
    unreachable
  td MyError @scoop_td_MyError size=16 vtable=0 itables=0
  layout String size=24 align=8 refs=[]
  layout Int size=8 align=8 refs=[]
  layout Boolean size=1 align=1 refs=[]
  layout MyError size=16 align=8 refs=[]
  entry @scoop_main
";
        assert_eq!(lir::dump(&module), expected);
    }

    #[test]
    fn finally_runs_on_the_normal_catch_and_rethrow_paths() {
        let (mut b, helper, handled, cleanup) = try_shell(true);
        let my_error = my_error(&mut b);
        let mut locals = Arena::new();
        let e = locals.alloc(local("e", mir::Type::Class(my_error)));
        let main = b.main(
            locals,
            vec![stmt(mir::StatementKind::Try(mir::Try {
                body: vec![expr_stmt(user_call(helper))],
                catches: vec![mir::CatchClause {
                    local: e,
                    ty: Box::new(mir::Type::Class(my_error)),
                    body: vec![expr_stmt(user_call(handled))],
                    span: SPAN,
                }],
                finally_body: Some(vec![expr_stmt(user_call(cleanup))]),
            }))],
        );
        let module = lower(&b.finish(main));
        let dump = lir::dump(&module);

        // The finally body is inlined on normal completion, after the
        // catch body, on a catch-body exceptional exit, and before the
        // no-match rethrow.
        assert_eq!(
            dump.matches("call @scoop.cleanup()").count()
                + dump.matches("invoke @scoop.cleanup()").count(),
            4
        );
        // The last copy is on the rethrow path, before the rethrow.
        let rethrow = dump
            .find("invoke @scoop_rt_rethrow()")
            .expect("a rethrow path");
        let last_cleanup = dump
            .rfind("@scoop.cleanup()")
            .expect("the rethrow path runs the finally");
        assert!(last_cleanup < rethrow);
        assert!(dump.contains("landingpad"));
        // A caught normal exit ends directly; catch-body exceptions
        // and no-match/rethrow exits have distinct cleanup pads.
        // Exactly one executes on each path.
        assert_eq!(dump.matches("end_catch").count(), 3);
    }

    #[test]
    fn return_inside_try_runs_finally_before_returning() {
        // fun f(): Int { try { return 1 } finally { cleanup() } }
        let (mut b, _, _, cleanup) = try_shell(true);
        let f = b.user_fn_full(
            "f",
            "scoop.f",
            Vec::new(),
            mir::Type::Int,
            Arena::new(),
            vec![stmt(mir::StatementKind::Try(mir::Try {
                body: vec![return_stmt(mir::Expr::IntLiteral(1))],
                catches: Vec::new(),
                finally_body: Some(vec![expr_stmt(user_call(cleanup))]),
            }))],
        );
        let _ = f;
        let main = b.main(Arena::new(), vec![]);
        let module = lower(&b.finish(main));

        // The finally copy runs before the return on the `return`
        // path and before the rethrow on the unwind path; the merge
        // block is dead (both paths leave the function).
        let expected = "\
Module
  fun @scoop.helper() -> void
  block entry
    ret
  fun @scoop.handled() -> void
  block entry
    ret
  fun @scoop.cleanup() -> void
  block entry
    ret
  fun @scoop.f() -> i64
    local %0 $sc.1: exception_record
    local %1 $sc.2: ptr
  block entry
    call @scoop.cleanup()
    t0 = aggregate () : {}
    ret 1
  block try.unwind.1
    (t1, t2) = landingpad : (exception_record, ptr)
    store t1 -> local0
    store t2 -> local1
    br @try.dispatch.2
  block try.dispatch.2
    t3 = begin_catch local1 : ptr
    invoke @scoop.cleanup() normal @invoke.normal.6 unwind @try.exit_pad.3
    br @invoke.normal.6
  block try.exit_pad.3
    (t5, t6) = cleanup_pad : (exception_record, ptr)
    store t5 -> local0
    store t6 -> local1
    br @try.exit_cleanup.4
  block try.exit_cleanup.4
    end_catch
    resume local0
  block try.end.5
    unreachable
  block invoke.normal.6
    t4 = aggregate () : {}
    invoke @scoop_rt_rethrow() normal @rethrow.normal.7 unwind @try.exit_pad.3
    br @rethrow.normal.7
  block rethrow.normal.7
    unreachable
  fun @scoop_main() -> void
  block entry
    ret
  layout String size=24 align=8 refs=[]
  layout Int size=8 align=8 refs=[]
  layout Boolean size=1 align=1 refs=[]
  entry @scoop_main
";
        assert_eq!(lir::dump(&module), expected);
    }

    #[test]
    fn throw_outside_try_is_a_throw_instruction() {
        // fun fail(): Unit { throw makeError() } — no try, so the
        // `Throw` instruction ends the block.
        let mut b = Builder::new();
        let my_error = my_error(&mut b);
        let mut ctor_locals = Arena::new();
        let make = b.user_fn_full(
            "makeError",
            "scoop.makeError",
            Vec::new(),
            mir::Type::Class(my_error),
            std::mem::take(&mut ctor_locals),
            vec![return_stmt(mir::Expr::ClassInit {
                class_id: my_error,
                args: Vec::new(),
            })],
        );
        let main = b.main(
            Arena::new(),
            vec![stmt(mir::StatementKind::Throw(mir::Expr::Call(
                mir::Call {
                    target: mir::CallTarget {
                        kind: mir::CallKind::Direct,
                        callee: mir::Callee::User(make),
                    },
                    args: Vec::new(),
                },
            )))],
        );
        let module = lower(&b.finish(main));

        // Outside a try the throw is the `Throw` instruction ending
        // the block; the callee stays a plain call.
        let expected = "\
Module
  global @scoop_td_MyError = c\"\"
  fun @scoop.makeError() -> ptr
  block entry
    t0 = call @scoop_rt_alloc(global0, 16) : ptr
    ret t0
  fun @scoop_main() -> void
  block entry
    t0 = call @scoop.makeError() : ptr
    throw t0
    unreachable
  td MyError @scoop_td_MyError size=16 vtable=0 itables=0
  layout String size=24 align=8 refs=[]
  layout Int size=8 align=8 refs=[]
  layout Boolean size=1 align=1 refs=[]
  layout MyError size=16 align=8 refs=[]
  entry @scoop_main
";
        assert_eq!(lir::dump(&module), expected);
    }

    #[test]
    fn throw_inside_try_invokes_to_the_own_landingpad() {
        // try { throw e } catch (e: MyError) {} — the throw must
        // reach this function's own pad, so it is an invoke of the
        // runtime throw entry, not a plain `Throw`.
        let mut b = Builder::new();
        let my_error = my_error(&mut b);
        let mut locals = Arena::new();
        let e = locals.alloc(local("e", mir::Type::Class(my_error)));
        let main = b.main(
            locals,
            vec![stmt(mir::StatementKind::Try(mir::Try {
                body: vec![stmt(mir::StatementKind::Throw(mir::Expr::Local(e)))],
                catches: vec![mir::CatchClause {
                    local: e,
                    ty: Box::new(mir::Type::Class(my_error)),
                    body: Vec::new(),
                    span: SPAN,
                }],
                finally_body: None,
            }))],
        );
        let module = lower(&b.finish(main));

        let function = module
            .functions
            .iter()
            .find(|f| f.symbol == mir::ENTRY_SYMBOL)
            .expect("the entry function");
        // The entry block ends with the invoke; its unwind target is
        // the block that starts with the landingpad.
        let entry = &function.blocks[function.entry];
        let lir::Instruction::Invoke {
            symbol,
            normal,
            unwind,
            ..
        } = entry.instructions.last().expect("the throw invoke")
        else {
            panic!("a throw inside a try must be invoked")
        };
        assert_eq!(symbol, "scoop_rt_throw");
        assert!(matches!(
            function.blocks[*unwind].instructions.first(),
            Some(lir::Instruction::LandingPad { .. })
        ));
        assert!(matches!(
            function.blocks[*normal].terminator,
            lir::Terminator::Unreachable
        ));
        // The invoke block's terminator is the redundant `Br` to the
        // normal target (the codegen convention).
        assert!(matches!(
            entry.terminator,
            lir::Terminator::Br(target) if target == *normal
        ));
    }

    #[test]
    fn nested_trys_unwind_to_their_own_pads() {
        // try { try { a() } catch (e1: E1) { b() } } catch (e2: E2) { c() }
        // — `a` unwinds to the inner pad; `b` (in the inner catch)
        // unwinds through the inner cleanup before the outer pad.
        let mut b = Builder::new();
        let e1 = b.class("E1", None, &[], vec![], vec![]);
        let e2 = b.class("E2", None, &[], vec![], vec![]);
        let a = b.user_fn("a", "scoop.a", Arena::new(), vec![]);
        let bb = b.user_fn("b", "scoop.b", Arena::new(), vec![]);
        let c = b.user_fn("c", "scoop.c", Arena::new(), vec![]);
        let mut locals = Arena::new();
        let e1_local = locals.alloc(local("e1", mir::Type::Class(e1)));
        let e2_local = locals.alloc(local("e2", mir::Type::Class(e2)));
        let main = b.main(
            locals,
            vec![stmt(mir::StatementKind::Try(mir::Try {
                body: vec![stmt(mir::StatementKind::Try(mir::Try {
                    body: vec![expr_stmt(user_call(a))],
                    catches: vec![mir::CatchClause {
                        local: e1_local,
                        ty: Box::new(mir::Type::Class(e1)),
                        body: vec![expr_stmt(user_call(bb))],
                        span: SPAN,
                    }],
                    finally_body: None,
                }))],
                catches: vec![mir::CatchClause {
                    local: e2_local,
                    ty: Box::new(mir::Type::Class(e2)),
                    body: vec![expr_stmt(user_call(c))],
                    span: SPAN,
                }],
                finally_body: None,
            }))],
        );
        let module = lower(&b.finish(main));

        let function = module
            .functions
            .iter()
            .find(|f| f.symbol == mir::ENTRY_SYMBOL)
            .expect("the entry function");
        // Two primary catch pads, one per try. Inner handler cleanup
        // pads also carry catch-all clauses so they can forward to the
        // outer dispatch; identify primaries by their block role.
        let pads: Vec<&str> = function
            .blocks
            .iter()
            .filter(|(_, block)| block.name.contains("try.unwind"))
            .map(|(_, block)| block.name.as_str())
            .collect();
        assert_eq!(pads.len(), 2);
        let cleanup_pad_count = function
            .blocks
            .iter()
            .filter(|(_, block)| {
                matches!(
                    block.instructions.first(),
                    Some(lir::Instruction::CleanupPad { .. })
                )
            })
            .count();
        assert_eq!(cleanup_pad_count, 2);
        let handler_pads: Vec<&str> = function
            .blocks
            .iter()
            .filter(|(_, block)| block.name.contains("handler_pad"))
            .map(|(_, block)| block.name.as_str())
            .collect();
        assert_eq!(handler_pads.len(), 2);
        // `a` unwinds to the inner catch pad. `b` runs inside that
        // handler and therefore unwinds through the inner cleanup;
        // `c` does the same through the outer cleanup.
        let mut invokes = Vec::new();
        for (_, block) in function.blocks.iter() {
            for instruction in &block.instructions {
                if let lir::Instruction::Invoke { symbol, unwind, .. } = instruction {
                    invokes.push((symbol.as_str(), function.blocks[*unwind].name.as_str()));
                }
            }
        }
        let unwind_of = |symbol: &str| {
            invokes
                .iter()
                .find(|(s, _)| *s == symbol)
                .map(|(_, u)| *u)
                .unwrap_or_else(|| panic!("{symbol} must be invoked"))
        };
        assert_eq!(unwind_of("scoop.a"), pads[1]);
        assert_eq!(unwind_of("scoop.b"), handler_pads[1]);
        assert_eq!(unwind_of("scoop.c"), handler_pads[0]);
    }
}
