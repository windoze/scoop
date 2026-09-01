//! LIR stage: type layout, exception lowering. LIR contains nothing
//! Scoop-specific and is mechanically translatable to the target IR.
//! (Impl spec 2.4's statepoint insertion is applied one stage later,
//! in codegen — see the M9 note below.)
//!
//! See `docs/specs/SCOOP-IMPL-SPEC.md` section 2.4 and
//! `docs/milestone4/DESIGN.md` section 3.4.
//!
//! Every MIR function arrives as an explicit CFG; LIR maps its blocks,
//! normal and unwind edges mechanically. Every MIR local gets a stack
//! slot (stores on declaration and assignment, implicit loads on use);
//! SSA construction is left to LLVM's mem2reg. Struct / tuple / Unit values are LLVM literal
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
//! single pointer-like field (`None` = null — this is
//! `Option<String>`); otherwise a tagged form whose pure-value variants
//! share one payload and whose ref-bearing variants have disjoint slots.
//! The MIR enum operations map onto `EnumWrap` / `EnumTag` /
//! `EnumField`, which codegen translates mechanically per the
//! representation. Enum layouts keep fixed ref offsets for all disjoint
//! ref-bearing slots; inactive slots are zero, so scanning never reads
//! the tag and composes mechanically in aggregates (runtime spec 2.2).
//!
//! M5/M14: arrays (docs/milestone5/DESIGN.md 2.4 and milestone14
//! DESIGN.md). A concrete intrinsic `Array<T>` / `MutableArray<T>` is an
//! ordinary MIR class application and maps to a managed pointer. LIR keeps one
//! complete `ArrayType` record per application; every array instruction names
//! that record with an `ArrayTypeId`. Element layout, GC scan, nominal kind,
//! and runtime descriptor identity therefore arrive from MIR explicitly and
//! are never reconstructed from instruction/result shapes in codegen.
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
//! payload and array size are at byte 16 (elements start at byte 24,
//! rounded up when the element type is over-aligned), and
//! the String length is at byte 16 (bytes at 24). The layout math below
//! counts the header as 16 bytes; `scoop_rt_alloc` writes both header words (the TD from
//! its argument, a zeroed GC word), so no lowering stores the header.
//! The statepoint side of M9 (the GC strategy, safepoint polls, and
//! the stackmap emission of impl spec 2.4's statepoint insertion) is
//! applied in codegen — it is an instrumentation of the emitted LLVM,
//! and keeping it out of LIR keeps these dumps stable.

use std::collections::{HashMap, HashSet};

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
    let mut string_global_map: HashMap<mir::StringConstId, lir::GlobalId> = HashMap::new();
    for (id, string) in module.strings.iter() {
        let global = globals.alloc(lir::Global {
            symbol: string.symbol.clone(),
            address_kind: lir::PointerKind::Managed,
            init: lir::GlobalInit::StringConst(string.value.clone()),
        });
        string_global_map.insert(id, global);
    }

    // Enum definitions with fixed representations, in the MIR arena's
    // order: `mir::EnumId` and `lir::EnumDefId` align.
    let enums = lower_enums(module);
    // Struct ids also transpose 1:1. Their definitions retain the exact
    // physical layout needed by codegen and C bridge generation.
    let structs = lower_structs(module, &enums);
    let extern_functions = lower_extern_functions(module);
    let (storage_globals, native_globals) = lower_globals(module, &mut globals);
    let callback_bridges = lower_callback_bridges(module);
    let foreign_callback_bridges = lower_foreign_callback_bridges(module);
    let (arrays, array_type_map) = array_types(module, &enums);

    // Tuple types encountered while mapping value types, in
    // first-appearance order; each one gets a meta layout.
    let mut layout_types = Vec::new();
    // Trap message globals (`scoop.cstr.N`), numbered in creation order.
    let mut cstr_count = 0usize;
    // TypeDescriptor reference stubs (`scoop_td_*`), deduplicated by
    // symbol (see the module docs for the TD-reference convention).
    let mut td_map = HashMap::new();
    let mut functions: Vec<lir::Function> = module
        .top_level
        .iter()
        .map(|&id| {
            lower_function(
                module,
                &module.functions[id],
                &string_global_map,
                &storage_globals,
                &mut globals,
                &mut cstr_count,
                &mut layout_types,
                &enums,
                &array_type_map,
                &mut td_map,
            )
        })
        .collect();
    for function in &mut functions {
        annotate_native_call_roots(function, &structs, &enums);
    }

    let (layouts, string_layout) = layouts(module, &enums, &layout_types);
    let (type_descriptors, string_type_descriptor) = type_descriptors(module, &enums);
    lir::Module {
        globals,
        structs,
        enums,
        functions,
        extern_functions,
        native_globals,
        callback_bridges,
        foreign_callback_bridges,
        entry_symbol: module.functions[module.entry].symbol.clone(),
        meta: lir::LirMeta {
            string: lir::StringMetadata {
                layout: string_layout,
                type_descriptor: string_type_descriptor,
            },
            arrays,
            layouts,
            type_descriptors,
        },
    }
}

fn lower_callback_bridges(module: &mir::Module) -> Arena<lir::CallbackBridge> {
    let mut callbacks = Arena::new();
    for (id, callback) in module.callback_bridges.iter() {
        let signature = &module.function_types[callback.signature];
        callbacks.alloc(lir::CallbackBridge {
            source_name: module.functions[callback.source].name.clone(),
            bridge_symbol: module.functions[callback.bridge_function].symbol.clone(),
            trampoline_symbol: format!("scoop_c_callback_{}", id.into_raw().into_u32()),
            params: signature
                .parameter_types
                .iter()
                .map(|ty| c_ffi_type(module, ty))
                .collect(),
            return_type: c_ffi_type(module, &signature.return_type),
        });
    }
    callbacks
}

fn lower_foreign_callback_bridges(module: &mir::Module) -> Arena<lir::ForeignCallbackBridge> {
    let mut bridges = Arena::new();
    let mut shared_trampolines: HashMap<(mir::FunctionTypeId, u32), (String, String)> =
        HashMap::new();
    for (_, bridge) in module.foreign_callback_bridges.iter() {
        let signature = &module.function_types[bridge.native_signature];
        let adapter = &module.foreign_callback_adapters[bridge.adapter];
        let key = (bridge.native_signature, bridge.context_index);
        let (trampoline_symbol, signature_symbol) =
            if let Some(symbols) = shared_trampolines.get(&key) {
                symbols.clone()
            } else {
                let raw = shared_trampolines.len();
                let symbols = (
                    format!("scoop_foreign_callback_{raw}"),
                    format!("scoop_foreign_callback_signature_{raw}"),
                );
                shared_trampolines.insert(key, symbols.clone());
                symbols
            };
        bridges.alloc(lir::ForeignCallbackBridge {
            adapter_symbol: module.functions[adapter.function].symbol.clone(),
            trampoline_symbol,
            signature_symbol,
            params: signature
                .parameter_types
                .iter()
                .map(|ty| c_ffi_type(module, ty))
                .collect(),
            return_type: c_ffi_type(module, &signature.return_type),
            context_index: bridge.context_index,
            mode: match bridge.mode {
                mir::ForeignCallbackMode::Reusable => lir::ForeignCallbackMode::Reusable,
                mir::ForeignCallbackMode::OneShot => lir::ForeignCallbackMode::OneShot,
            },
        });
    }
    bridges
}

#[derive(Clone, Copy)]
enum StorageGlobal {
    Local(lir::GlobalId),
    Native(lir::NativeGlobalId),
}

fn lower_globals(
    module: &mir::Module,
    globals: &mut Arena<lir::Global>,
) -> (
    HashMap<mir::GlobalId, StorageGlobal>,
    Arena<lir::NativeGlobal>,
) {
    let mut map = HashMap::new();
    let mut native = Arena::new();
    for (id, global) in module.globals.iter() {
        let storage = match &global.storage {
            mir::GlobalStorage::Local {
                thread_local,
                initializer,
            } => {
                let lir_id = globals.alloc(lir::Global {
                    symbol: global.symbol.clone(),
                    address_kind: lir::PointerKind::Raw,
                    init: lir::GlobalInit::Storage {
                        ty: lir_type(&global.ty),
                        initializer: lower_constant(initializer),
                        thread_local: *thread_local,
                    },
                });
                StorageGlobal::Local(lir_id)
            }
            mir::GlobalStorage::Extern {
                library,
                native_symbol,
                thread_local,
            } => {
                let raw = native.len() as u32;
                let lir_id = native.alloc(lir::NativeGlobal {
                    source_name: global.name.clone(),
                    native_symbol: native_symbol.clone(),
                    library: library.clone(),
                    ty: lir_type(&global.ty),
                    c_type: c_ffi_type(module, &global.ty),
                    mutable: global.mutable,
                    thread_local: *thread_local,
                    get_bridge_symbol: format!("scoop_c_global_get_{raw}"),
                    set_bridge_symbol: global.mutable.then(|| format!("scoop_c_global_set_{raw}")),
                    address_bridge_symbol: format!("scoop_c_global_address_{raw}"),
                });
                StorageGlobal::Native(lir_id)
            }
        };
        map.insert(id, storage);
    }
    (map, native)
}

fn lower_constant(value: &mir::ConstantValue) -> lir::ConstantValue {
    match value {
        mir::ConstantValue::Int(value) => lir::ConstantValue::Int(*value),
        mir::ConstantValue::Bool(value) => lir::ConstantValue::Bool(*value),
        mir::ConstantValue::NullPtr | mir::ConstantValue::NullFunPtr => lir::ConstantValue::NullPtr,
        mir::ConstantValue::Struct { struct_id, fields } => lir::ConstantValue::Struct {
            struct_id: struct_def_id(*struct_id),
            fields: fields.iter().map(lower_constant).collect(),
        },
    }
}

fn lower_extern_functions(module: &mir::Module) -> Arena<lir::ExternFunction> {
    let mut functions = Arena::new();
    for (id, extern_) in module.extern_functions.iter() {
        let params = extern_.params.iter().map(lir_type).collect::<Vec<_>>();
        let return_type = lir_type(&extern_.return_type);
        let kind = match extern_.abi {
            mir::ExternAbi::C => lir::ExternFunctionKind::C {
                bridge_symbol: format!("scoop_c_bridge_{}", id.into_raw().into_u32()),
                params: extern_
                    .params
                    .iter()
                    .map(|ty| c_ffi_type(module, ty))
                    .collect(),
                return_type: c_ffi_type(module, &extern_.return_type),
            },
            mir::ExternAbi::Scoop => lir::ExternFunctionKind::Scoop {
                gc_effect: match extern_.gc_effect {
                    mir::GcEffect::Managed => lir::GcEffect::Managed,
                    mir::GcEffect::NoGc => lir::GcEffect::NoGc,
                },
            },
        };
        functions.alloc(lir::ExternFunction {
            source_name: extern_.source_name.clone(),
            native_symbol: extern_.native_symbol.clone(),
            library: extern_.library.clone(),
            calling_convention: match extern_.calling_convention {
                mir::CallingConvention::Cdecl => lir::CallingConvention::Cdecl,
            },
            params,
            return_type,
            kind,
        });
    }
    functions
}

fn c_ffi_type(module: &mir::Module, ty: &mir::Type) -> lir::CType {
    match ty {
        mir::Type::Unit => lir::CType::Unit,
        mir::Type::Int => lir::CType::Int,
        mir::Type::UInt => lir::CType::UInt,
        mir::Type::Boolean => lir::CType::Boolean,
        mir::Type::Ptr(_) => lir::CType::Pointer,
        mir::Type::FunPtr(signature) => {
            let signature = &module.function_types[*signature];
            lir::CType::FunctionPointer {
                params: signature
                    .parameter_types
                    .iter()
                    .map(|ty| c_ffi_type(module, ty))
                    .collect(),
                return_type: Box::new(c_ffi_type(module, &signature.return_type)),
            }
        }
        mir::Type::Struct(id) => lir::CType::Struct(struct_def_id(*id)),
        mir::Type::Enum(_, args)
            if matches!(args.as_slice(), [mir::Type::Ptr(_) | mir::Type::FunPtr(_)]) =>
        {
            c_ffi_type(module, &args[0])
        }
        other => unreachable!(
            "HIR C-FFI classification rejects {} before MIR",
            mir::type_name(module, other)
        ),
    }
}

/// The `lir::EnumDefId` of a MIR enum (the arenas are transposed 1:1).
fn enum_def_id(id: mir::EnumId) -> lir::EnumDefId {
    lir::EnumDefId::from_raw(id.into_raw())
}

fn struct_def_id(id: mir::StructId) -> lir::StructDefId {
    lir::StructDefId::from_raw(id.into_raw())
}

fn lower_structs(module: &mir::Module, enums: &Arena<lir::EnumDef>) -> Arena<lir::StructDef> {
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
fn nested_enums(module: &mir::Module, ty: &mir::Type, out: &mut Vec<mir::EnumId>) {
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

fn is_niche_payload(ty: &mir::Type) -> bool {
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
fn repr_shape(repr: &lir::EnumRepr) -> (u64, u64) {
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
fn layouts(
    module: &mir::Module,
    enums: &Arena<lir::EnumDef>,
    from_code: &[mir::Type],
) -> (Vec<lir::Layout>, lir::Layout) {
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

    let mut layouts = Vec::new();
    for (_, def) in module.structs.iter() {
        layouts.push(struct_layout(module, enums, def));
    }
    for (id, def) in module.enums.iter() {
        layouts.push(enum_layout(module, enums, id, def));
    }
    let mut string = None;
    for (_, def) in module.classes.iter() {
        match def.representation {
            mir::ClassRepresentation::Intrinsic(mir::IntrinsicTypeRepresentation::String) => {
                let layout = class_definition_layout(module, enums, def);
                assert!(
                    string.replace(layout).is_none(),
                    "one typed String representation"
                );
            }
            mir::ClassRepresentation::Intrinsic(
                mir::IntrinsicTypeRepresentation::Array { .. }
                | mir::IntrinsicTypeRepresentation::MutableArray { .. },
            ) => {}
            _ => layouts.push(class_definition_layout(module, enums, def)),
        }
    }
    for (_, def) in module.closure_classes.iter() {
        let (_, size, align, scan) = closure_shape(module, enums, def);
        layouts.push(lir::Layout {
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
            layouts.push(aggregate_layout(
                module,
                enums,
                mir::type_name(module, ty),
                elements,
            ));
        }
    }
    (
        layouts,
        string.expect("LocalConcreteHir supplies the typed intrinsic String representation"),
    )
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

fn struct_layout(
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
fn enum_layout(
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
fn class_layout(
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
fn class_shape(
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

fn class_definition_layout(
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
fn closure_shape(
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
fn class_order(module: &mir::Module) -> Vec<mir::ClassId> {
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
fn td_symbol(name: &str) -> String {
    format!("scoop_td_{name}")
}

/// The symbol a vtable / itable slot points at.
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
fn type_descriptors(
    module: &mir::Module,
    enums: &Arena<lir::EnumDef>,
) -> (Vec<lir::TypeDescriptor>, lir::TypeDescriptor) {
    let mut tds = Vec::new();
    let mut string = None;
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
    for (id, _) in module.function_types.iter() {
        let name = format!(
            "function${}",
            mir::encode_type(module, &mir::Type::Function(id))
        );
        tds.push(lir::TypeDescriptor {
            symbol: td_symbol(&name),
            name,
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
        let descriptor = class_type_descriptor(module, enums, id);
        if matches!(
            def.representation,
            mir::ClassRepresentation::Intrinsic(mir::IntrinsicTypeRepresentation::String)
        ) {
            assert!(
                string.replace(descriptor).is_none(),
                "one typed String TypeDescriptor"
            );
        } else if !matches!(
            def.representation,
            mir::ClassRepresentation::Intrinsic(
                mir::IntrinsicTypeRepresentation::Array { .. }
                    | mir::IntrinsicTypeRepresentation::MutableArray { .. }
            )
        ) {
            tds.push(descriptor);
        }
    }
    for (_, def) in module.closure_classes.iter() {
        let (_, size, align, scan) = closure_shape(module, enums, def);
        tds.push(lir::TypeDescriptor {
            name: def.name.clone(),
            symbol: td_symbol(&def.name),
            size,
            align,
            scan,
            parent: Some(td_symbol(&format!(
                "function${}",
                mir::encode_type(module, &mir::Type::Function(def.function_type))
            ))),
            vtable: Vec::new(),
            itables: def
                .bridges
                .iter()
                .map(|bridge| lir::ItableRecord {
                    interface_symbol: td_symbol(&format!(
                        "function${}",
                        mir::encode_type(module, &mir::Type::Function(bridge.target))
                    )),
                    slots: vec![module.functions[bridge.function].symbol.clone()],
                })
                .collect(),
        });
    }
    (
        tds,
        string.expect("LocalConcreteHir supplies the typed intrinsic String TypeDescriptor"),
    )
}

fn class_type_descriptor(
    module: &mir::Module,
    enums: &Arena<lir::EnumDef>,
    id: mir::ClassId,
) -> lir::TypeDescriptor {
    let def = &module.classes[id];
    let (size, align, scan) = class_layout(module, enums, def);
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
        size,
        align,
        scan,
        parent: def
            .base_class()
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
    }
}

/// Transpose every concrete intrinsic array class application into one typed
/// LIR metadata record. The MIR class id -> LIR array id map is complete before
/// function lowering starts, so no instruction discovers metadata on demand.
fn array_types(
    module: &mir::Module,
    enums: &Arena<lir::EnumDef>,
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
            type_descriptor: class_type_descriptor(module, enums, class_id),
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

/// Exact layout of one named struct. Ordinary structs use natural field
/// alignment. `@CLayout(packed = N)` caps each field's access alignment at
/// `N`; `aligned = N` raises (but never lowers) the aggregate alignment.
fn struct_shape(
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
fn sequence(parts: impl IntoIterator<Item = lir::RefScan>) -> lir::RefScan {
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
fn record_layout_types(ty: &mir::Type, types: &mut Vec<mir::Type>) {
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
fn lir_type(ty: &mir::Type) -> lir::LirType {
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
    }
}

fn address_taken_locals(function: &mir::Function) -> HashSet<mir::LocalId> {
    fn collect_expr(value: &mir::Expr, out: &mut HashSet<mir::LocalId>) {
        match value {
            mir::Expr::AddressOf { local, .. } => {
                out.insert(*local);
            }
            mir::Expr::TupleLiteral(values)
            | mir::Expr::ArrayLiteral {
                elements: values, ..
            }
            | mir::Expr::StructInit { args: values, .. }
            | mir::Expr::ClassInit { args: values, .. }
            | mir::Expr::ClosureAlloc {
                captures: values, ..
            }
            | mir::Expr::VariantConstruct { fields: values, .. } => {
                for value in values {
                    collect_expr(value, out);
                }
            }
            mir::Expr::Retype { operand, .. }
            | mir::Expr::ClosureCapture {
                closure: operand, ..
            }
            | mir::Expr::ForeignCallbackRegister {
                closure: operand, ..
            }
            | mir::Expr::ForeignCallbackOperation {
                callback: operand, ..
            }
            | mir::Expr::FieldAccess {
                receiver: operand, ..
            }
            | mir::Expr::AtomicFieldLoad {
                object: operand, ..
            }
            | mir::Expr::Box(operand)
            | mir::Expr::Unbox(operand)
            | mir::Expr::IsInstance { operand, .. }
            | mir::Expr::Cast { operand, .. }
            | mir::Expr::ArrayLen { operand, .. }
            | mir::Expr::ArrayClone { operand, .. }
            | mir::Expr::Unary { operand, .. }
            | mir::Expr::EnumTag(operand)
            | mir::Expr::EnumField { operand, .. }
            | mir::Expr::PtrFromUInt { operand, .. }
            | mir::Expr::PtrToUInt(operand)
            | mir::Expr::PtrCast { operand, .. } => collect_expr(operand, out),
            mir::Expr::AtomicFieldCompareExchange {
                object,
                expected,
                replacement,
                ..
            } => {
                collect_expr(object, out);
                collect_expr(expected, out);
                collect_expr(replacement, out);
            }
            mir::Expr::ArrayGet { array, index, .. }
            | mir::Expr::Binary {
                lhs: array,
                rhs: index,
                ..
            }
            | mir::Expr::PtrOffset {
                pointer: array,
                offset: index,
                ..
            } => {
                collect_expr(array, out);
                collect_expr(index, out);
            }
            mir::Expr::PtrLoad {
                pointer, offset, ..
            } => {
                collect_expr(pointer, out);
                if let Some(offset) = offset {
                    collect_expr(offset, out);
                }
            }
            mir::Expr::PtrStore {
                pointer,
                offset,
                value,
                ..
            } => {
                collect_expr(pointer, out);
                if let Some(offset) = offset {
                    collect_expr(offset, out);
                }
                collect_expr(value, out);
            }
            mir::Expr::StringConst(_)
            | mir::Expr::IntLiteral(_)
            | mir::Expr::BoolLiteral(_)
            | mir::Expr::UnitLiteral
            | mir::Expr::Local(_)
            | mir::Expr::GlobalRead(_)
            | mir::Expr::GlobalAddress { .. }
            | mir::Expr::CaughtException
            | mir::Expr::SizeOf(_)
            | mir::Expr::AlignOf(_)
            | mir::Expr::FunPtrNull(_)
            | mir::Expr::FunctionAddress { .. } => {}
        }
    }

    fn collect_call(call: &mir::Call, out: &mut HashSet<mir::LocalId>) {
        for arg in &call.args {
            collect_expr(arg, out);
        }
    }

    let mut out = HashSet::new();
    for (_, block) in function.body.blocks.iter() {
        for statement in &block.statements {
            match &statement.kind {
                mir::StatementKind::Expr(value) => collect_expr(value, &mut out),
                mir::StatementKind::Call(effect) => match effect {
                    mir::CallEffect::Unit(call) | mir::CallEffect::Value { call, .. } => {
                        collect_call(call, &mut out)
                    }
                },
                mir::StatementKind::ValDecl { init, .. } => collect_expr(init, &mut out),
                mir::StatementKind::Assign { value, .. } => collect_expr(value, &mut out),
                mir::StatementKind::GlobalAssign { value, .. } => collect_expr(value, &mut out),
                mir::StatementKind::ArraySet {
                    array,
                    index,
                    value,
                    ..
                } => {
                    collect_expr(array, &mut out);
                    collect_expr(index, &mut out);
                    collect_expr(value, &mut out);
                }
                mir::StatementKind::FieldSet { object, value, .. }
                | mir::StatementKind::AtomicFieldStore { object, value, .. } => {
                    collect_expr(object, &mut out);
                    collect_expr(value, &mut out);
                }
                mir::StatementKind::Eh(_) => {}
            }
        }
        match &block.terminator {
            mir::Terminator::Branch { cond, .. } => collect_expr(cond, &mut out),
            mir::Terminator::Return { value } => {
                if let Some(value) = value {
                    collect_expr(value, &mut out);
                }
            }
            mir::Terminator::Throw { exception, .. } => collect_expr(exception, &mut out),
            mir::Terminator::Goto(_)
            | mir::Terminator::Rethrow { .. }
            | mir::Terminator::Resume
            | mir::Terminator::Trap { .. }
            | mir::Terminator::Unreachable => {}
        }
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn lower_function<'a>(
    module: &'a mir::Module,
    function: &'a mir::Function,
    global_map: &HashMap<mir::StringConstId, lir::GlobalId>,
    storage_globals: &HashMap<mir::GlobalId, StorageGlobal>,
    globals: &mut Arena<lir::Global>,
    cstr_count: &mut usize,
    layout_types: &mut Vec<mir::Type>,
    enums: &Arena<lir::EnumDef>,
    array_types: &'a HashMap<mir::ClassId, lir::ArrayTypeId>,
    td_map: &mut HashMap<String, lir::GlobalId>,
) -> lir::Function {
    // Parameters stay SSA values unless `addressOf` requires stable storage.
    // Address-taken parameters are copied once into a method-local slot.
    let address_taken = address_taken_locals(function);
    let mut local_map = HashMap::new();
    let params: Vec<lir::LirType> = function
        .params
        .iter()
        .enumerate()
        .map(|(index, param)| {
            record_layout_types(&param.ty, layout_types);
            if !address_taken.contains(&param.local) {
                local_map.insert(param.local, LocalSlot::Param(index as u32));
            }
            lir_type(&param.ty)
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
            ty: lir_type(&local.ty),
        });
        local_map.insert(mir_id, LocalSlot::Slot(lir_id));
    }

    // Unit-returning functions are void at the LLVM level (DESIGN 2.4).
    record_layout_types(&function.return_ty, layout_types);
    let returns_void = function.return_ty == mir::Type::Unit;
    let return_ty = if returns_void {
        lir::LirType::Void
    } else {
        lir_type(&function.return_ty)
    };

    let mut blocks = Arena::new();
    let mut block_map = HashMap::new();
    for (mir_id, block) in function.body.blocks.iter() {
        let lir_id = blocks.alloc(lir::BasicBlock {
            name: block.name.clone(),
            instructions: Vec::new(),
            terminator: lir::Terminator::Unreachable,
        });
        block_map.insert(mir_id, lir_id);
    }
    let entry = block_map[&function.body.entry];
    let mut lowerer = FunctionLowerer {
        module,
        mir_locals: &function.body.locals,
        global_map,
        storage_globals,
        globals,
        cstr_count,
        layout_types,
        enums,
        array_types,
        td_map,
        local_map,
        locals,
        temps: Arena::new(),
        blocks,
        current: entry,
        block_map,
        current_unwind: None,
        block_count: 0,
        hidden_count: 0,
        mir_return_ty: function.return_ty.clone(),
        returns_void,
        trap_blocks: HashMap::new(),
        current_sealed: false,
        exception_slots: None,
        caught_exception: None,
    };
    for (index, param) in function.params.iter().enumerate() {
        if address_taken.contains(&param.local) {
            let local = lowerer.local_slot(param.local);
            lowerer.blocks[entry]
                .instructions
                .push(lir::Instruction::Store {
                    local,
                    value: lir::Value::Param(index as u32),
                });
        }
    }
    for (mir_id, block) in function.body.blocks.iter() {
        let lir_id = lowerer.block_map[&mir_id];
        lowerer.enter(lir_id);
        lowerer.current_unwind = block.unwind.map(|target| lowerer.block_map[&target]);
        lowerer.lower_statements(&block.statements);
        if !lowerer.current_sealed {
            lowerer.lower_terminator(&block.terminator);
        }
        assert!(lowerer.current_sealed, "every MIR block has a terminator");
    }
    lir::Function {
        gc_effect: match function.gc_effect {
            mir::GcEffect::Managed => lir::GcEffect::Managed,
            mir::GcEffect::NoGc => lir::GcEffect::NoGc,
        },
        symbol: function.symbol.clone(),
        params,
        return_ty,
        locals: lowerer.locals,
        temps: lowerer.temps,
        blocks: lowerer.blocks,
        entry,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum LiveValue {
    Param(u32),
    Local(lir::LocalId),
    Temp(lir::TempId),
}

impl LiveValue {
    fn from_value(value: lir::Value) -> Option<Self> {
        match value {
            lir::Value::Param(index) => Some(Self::Param(index)),
            lir::Value::Local(id) => Some(Self::Local(id)),
            lir::Value::Temp(id) => Some(Self::Temp(id)),
            lir::Value::IntConst(_)
            | lir::Value::BoolConst(_)
            | lir::Value::NullPtr
            | lir::Value::Global(_) => None,
        }
    }

    fn source(self) -> lir::CallerRootSource {
        match self {
            Self::Param(index) => lir::CallerRootSource::Param(index),
            Self::Local(id) => lir::CallerRootSource::Local(id),
            Self::Temp(id) => lir::CallerRootSource::Temp(id),
        }
    }

    fn sort_key(self) -> (u8, u32) {
        match self {
            Self::Param(index) => (0, index),
            Self::Local(id) => (1, id.into_raw().into_u32()),
            Self::Temp(id) => (2, id.into_raw().into_u32()),
        }
    }
}

fn instruction_uses(instruction: &lir::Instruction) -> Vec<lir::Value> {
    match instruction {
        lir::Instruction::BinOp { lhs, rhs, .. } => vec![*lhs, *rhs],
        lir::Instruction::UnaryOp { operand, .. }
        | lir::Instruction::ExtractValue {
            aggregate: operand, ..
        }
        | lir::Instruction::HeapLoad {
            object: operand, ..
        }
        | lir::Instruction::AtomicLoad {
            object: operand, ..
        }
        | lir::Instruction::IntToPtr { value: operand, .. }
        | lir::Instruction::PtrToInt { value: operand, .. }
        | lir::Instruction::RawLoad {
            pointer: operand, ..
        }
        | lir::Instruction::BeginCatch { raw: operand, .. }
        | lir::Instruction::Throw { exception: operand }
        | lir::Instruction::ArrayLen { operand, .. }
        | lir::Instruction::ArrayClone { operand, .. }
        | lir::Instruction::EnumTag { operand, .. }
        | lir::Instruction::EnumField { operand, .. }
        | lir::Instruction::ForeignCallbackRegister {
            closure: operand, ..
        }
        | lir::Instruction::ForeignCallbackOperation {
            callback: operand, ..
        } => vec![*operand],
        lir::Instruction::MakeAggregate { elements, .. }
        | lir::Instruction::ArrayAlloc { elements, .. } => elements.clone(),
        lir::Instruction::Store { value, .. }
        | lir::Instruction::GlobalStore { value, .. }
        | lir::Instruction::NativeGlobalStore { value, .. } => vec![*value],
        lir::Instruction::NativeCall { args, .. }
        | lir::Instruction::Call { args, .. }
        | lir::Instruction::Invoke { args, .. } => args.clone(),
        lir::Instruction::HeapStore { object, value, .. }
        | lir::Instruction::AtomicStore { object, value, .. } => vec![*object, *value],
        lir::Instruction::AtomicCompareExchange {
            object,
            expected,
            replacement,
            ..
        } => vec![*object, *expected, *replacement],
        lir::Instruction::RawStore { pointer, value, .. } => vec![*pointer, *value],
        lir::Instruction::PtrOffset { pointer, bytes, .. } => vec![*pointer, *bytes],
        // Taking a local's address makes its current contents observable to
        // the following raw operation, so keep it live conservatively.
        lir::Instruction::LocalAddress { local, .. } => vec![lir::Value::Local(*local)],
        lir::Instruction::CallIndirect { table, args, .. }
        | lir::Instruction::InvokeIndirect { table, args, .. } => {
            let mut values = Vec::with_capacity(args.len() + 1);
            values.push(*table);
            values.extend(args);
            values
        }
        lir::Instruction::ArrayGet { array, index, .. } => vec![*array, *index],
        lir::Instruction::ArraySet {
            array,
            index,
            value,
            ..
        } => vec![*array, *index, *value],
        lir::Instruction::EnumWrap { fields, .. } => fields.clone(),
        lir::Instruction::GlobalLoad { .. }
        | lir::Instruction::GlobalAddress { .. }
        | lir::Instruction::NativeGlobalLoad { .. }
        | lir::Instruction::NativeGlobalAddress { .. }
        | lir::Instruction::FunctionAddress { .. }
        | lir::Instruction::LandingPad { .. }
        | lir::Instruction::CleanupPad { .. }
        | lir::Instruction::EndCatch => Vec::new(),
    }
}

fn instruction_defs(instruction: &lir::Instruction) -> Vec<LiveValue> {
    let out = match instruction {
        lir::Instruction::BinOp { out, .. }
        | lir::Instruction::UnaryOp { out, .. }
        | lir::Instruction::MakeAggregate { out, .. }
        | lir::Instruction::ExtractValue { out, .. }
        | lir::Instruction::HeapLoad { out, .. }
        | lir::Instruction::AtomicLoad { out, .. }
        | lir::Instruction::AtomicCompareExchange { out, .. }
        | lir::Instruction::GlobalLoad { out, .. }
        | lir::Instruction::GlobalAddress { out, .. }
        | lir::Instruction::NativeGlobalLoad { out, .. }
        | lir::Instruction::NativeGlobalAddress { out, .. }
        | lir::Instruction::FunctionAddress { out, .. }
        | lir::Instruction::IntToPtr { out, .. }
        | lir::Instruction::PtrToInt { out, .. }
        | lir::Instruction::RawLoad { out, .. }
        | lir::Instruction::PtrOffset { out, .. }
        | lir::Instruction::LocalAddress { out, .. }
        | lir::Instruction::BeginCatch { out, .. }
        | lir::Instruction::ArrayAlloc { out, .. }
        | lir::Instruction::ArrayLen { out, .. }
        | lir::Instruction::ArrayGet { out, .. }
        | lir::Instruction::ArrayClone { out, .. }
        | lir::Instruction::EnumWrap { out, .. }
        | lir::Instruction::EnumTag { out, .. }
        | lir::Instruction::EnumField { out, .. }
        | lir::Instruction::ForeignCallbackRegister { out, .. } => Some(*out),
        lir::Instruction::NativeCall { out, .. }
        | lir::Instruction::Call { out, .. }
        | lir::Instruction::CallIndirect { out, .. }
        | lir::Instruction::Invoke { out, .. }
        | lir::Instruction::InvokeIndirect { out, .. }
        | lir::Instruction::ForeignCallbackOperation { out, .. } => *out,
        lir::Instruction::Store { local, .. } => return vec![LiveValue::Local(*local)],
        lir::Instruction::LandingPad { record, raw }
        | lir::Instruction::CleanupPad { record, raw } => {
            return vec![LiveValue::Temp(*record), LiveValue::Temp(*raw)];
        }
        lir::Instruction::GlobalStore { .. }
        | lir::Instruction::NativeGlobalStore { .. }
        | lir::Instruction::HeapStore { .. }
        | lir::Instruction::AtomicStore { .. }
        | lir::Instruction::RawStore { .. }
        | lir::Instruction::ArraySet { .. }
        | lir::Instruction::EndCatch
        | lir::Instruction::Throw { .. } => None,
    };
    out.map_or_else(Vec::new, |out| vec![LiveValue::Temp(out)])
}

fn terminator_uses(terminator: &lir::Terminator, mut use_value: impl FnMut(lir::Value)) {
    match terminator {
        lir::Terminator::CondBr { cond, .. } => use_value(*cond),
        lir::Terminator::Return { value: Some(value) } => use_value(*value),
        lir::Terminator::Resume { exception } => use_value(*exception),
        lir::Terminator::Br(_)
        | lir::Terminator::Return { value: None }
        | lir::Terminator::Unreachable => {}
    }
}

fn block_successors(block: &lir::BasicBlock) -> Vec<lir::BlockId> {
    let mut successors = match block.terminator {
        lir::Terminator::Br(target) => vec![target],
        lir::Terminator::CondBr {
            then_block,
            else_block,
            ..
        } => vec![then_block, else_block],
        lir::Terminator::Return { .. }
        | lir::Terminator::Resume { .. }
        | lir::Terminator::Unreachable => Vec::new(),
    };
    if let Some(
        lir::Instruction::Invoke { normal, unwind, .. }
        | lir::Instruction::InvokeIndirect { normal, unwind, .. },
    ) = block.instructions.last()
    {
        if !successors.contains(normal) {
            successors.push(*normal);
        }
        if !successors.contains(unwind) {
            successors.push(*unwind);
        }
    }
    successors
}

fn shift_scan(scan: &lir::RefScan, base: u64) -> lir::RefScan {
    match scan {
        lir::RefScan::None => lir::RefScan::None,
        lir::RefScan::References(offsets) => {
            lir::RefScan::References(offsets.iter().map(|offset| base + offset).collect())
        }
        lir::RefScan::Sequence(parts) => {
            lir::RefScan::Sequence(parts.iter().map(|part| shift_scan(part, base)).collect())
        }
    }
}

fn lir_size_align(
    ty: &lir::LirType,
    structs: &Arena<lir::StructDef>,
    enums: &Arena<lir::EnumDef>,
) -> (u64, u64) {
    match ty {
        lir::LirType::Void => (0, 1),
        lir::LirType::I1 => (1, 1),
        lir::LirType::I64 | lir::LirType::Ptr(_) => (8, 8),
        lir::LirType::ExceptionRecord => (16, 8),
        lir::LirType::Aggregate(fields) => {
            let (_, size, align) = lir_aggregate_shape(fields, structs, enums);
            (size, align)
        }
        lir::LirType::Struct(id) => (structs[*id].size, structs[*id].align),
        lir::LirType::Enum(id) => repr_shape(&enums[*id].repr),
    }
}

fn lir_aggregate_shape(
    fields: &[lir::LirType],
    structs: &Arena<lir::StructDef>,
    enums: &Arena<lir::EnumDef>,
) -> (Vec<u64>, u64, u64) {
    let mut offsets = Vec::with_capacity(fields.len());
    let mut size = 0u64;
    let mut align = 1u64;
    for field in fields {
        let (field_size, field_align) = lir_size_align(field, structs, enums);
        size = size.next_multiple_of(field_align);
        offsets.push(size);
        size += field_size;
        align = align.max(field_align);
    }
    (offsets, size.next_multiple_of(align), align)
}

fn lir_root_scan(
    ty: &lir::LirType,
    structs: &Arena<lir::StructDef>,
    enums: &Arena<lir::EnumDef>,
    base: u64,
) -> lir::RefScan {
    match ty {
        lir::LirType::Ptr(lir::PointerKind::Managed) => lir::RefScan::References(vec![base]),
        lir::LirType::Aggregate(fields) => {
            let (offsets, _, _) = lir_aggregate_shape(fields, structs, enums);
            sequence(
                fields
                    .iter()
                    .zip(offsets)
                    .map(|(field, offset)| lir_root_scan(field, structs, enums, base + offset)),
            )
        }
        lir::LirType::Struct(id) => sequence(
            structs[*id]
                .fields
                .iter()
                .map(|field| lir_root_scan(&field.ty, structs, enums, base + field.layout.offset)),
        ),
        lir::LirType::Enum(id) => shift_scan(&enums[*id].scan, base),
        lir::LirType::Void
        | lir::LirType::I1
        | lir::LirType::I64
        | lir::LirType::Ptr(_)
        | lir::LirType::ExceptionRecord => lir::RefScan::None,
    }
}

fn live_value_ty(value: LiveValue, function: &lir::Function) -> &lir::LirType {
    match value {
        LiveValue::Param(index) => &function.params[index as usize],
        LiveValue::Local(id) => &function.locals[id].ty,
        LiveValue::Temp(id) => &function.temps[id].ty,
    }
}

fn caller_roots(
    live: &HashSet<LiveValue>,
    function: &lir::Function,
    structs: &Arena<lir::StructDef>,
    enums: &Arena<lir::EnumDef>,
) -> Vec<lir::CallerRoot> {
    let mut live = live.iter().copied().collect::<Vec<_>>();
    live.sort_by_key(|value| value.sort_key());
    live.into_iter()
        .filter_map(|value| {
            let scan = lir_root_scan(live_value_ty(value, function), structs, enums, 0);
            (scan != lir::RefScan::None).then(|| lir::CallerRoot {
                source: value.source(),
                scan,
            })
        })
        .collect()
}

fn annotate_native_call_roots(
    function: &mut lir::Function,
    structs: &Arena<lir::StructDef>,
    enums: &Arena<lir::EnumDef>,
) {
    type NativeCallAnnotation = Option<(Vec<lir::CallerRoot>, lir::RefScan)>;

    let block_count = function.blocks.len();
    let mut uses = vec![HashSet::new(); block_count];
    let mut defs = vec![HashSet::new(); block_count];
    let mut successors = vec![Vec::new(); block_count];

    for (id, block) in function.blocks.iter() {
        let index = id.into_raw().into_u32() as usize;
        let mut block_defs = HashSet::new();
        let mut block_uses = HashSet::new();
        for instruction in &block.instructions {
            for value in instruction_uses(instruction) {
                if let Some(value) = LiveValue::from_value(value)
                    && !block_defs.contains(&value)
                {
                    block_uses.insert(value);
                }
            }
            block_defs.extend(instruction_defs(instruction));
        }
        terminator_uses(&block.terminator, |value| {
            if let Some(value) = LiveValue::from_value(value)
                && !block_defs.contains(&value)
            {
                block_uses.insert(value);
            }
        });
        uses[index] = block_uses;
        defs[index] = block_defs;
        successors[index] = block_successors(block);
    }

    let mut live_in = vec![HashSet::new(); block_count];
    let mut live_out = vec![HashSet::new(); block_count];
    loop {
        let mut changed = false;
        for index in (0..block_count).rev() {
            let mut new_out = HashSet::new();
            for successor in &successors[index] {
                new_out.extend(
                    live_in[successor.into_raw().into_u32() as usize]
                        .iter()
                        .copied(),
                );
            }
            let mut new_in = uses[index].clone();
            new_in.extend(new_out.difference(&defs[index]).copied());
            if new_out != live_out[index] || new_in != live_in[index] {
                live_out[index] = new_out;
                live_in[index] = new_in;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }

    let mut annotations: Vec<Vec<NativeCallAnnotation>> = function
        .blocks
        .iter()
        .map(|(_, block)| vec![None; block.instructions.len()])
        .collect();

    for (id, block) in function.blocks.iter() {
        let block_index = id.into_raw().into_u32() as usize;
        let mut live = live_out[block_index].clone();
        terminator_uses(&block.terminator, |value| {
            if let Some(value) = LiveValue::from_value(value) {
                live.insert(value);
            }
        });
        for (instruction_index, instruction) in block.instructions.iter().enumerate().rev() {
            for definition in instruction_defs(instruction) {
                live.remove(&definition);
            }
            if matches!(
                instruction,
                lir::Instruction::NativeCall { .. }
                    | lir::Instruction::NativeGlobalLoad { .. }
                    | lir::Instruction::NativeGlobalStore { .. }
                    | lir::Instruction::NativeGlobalAddress { .. }
            ) {
                let roots = caller_roots(&live, function, structs, enums);
                let result_scan = match instruction {
                    lir::Instruction::NativeCall { out, .. } => {
                        out.as_ref().map_or(lir::RefScan::None, |out| {
                            lir_root_scan(&function.temps[*out].ty, structs, enums, 0)
                        })
                    }
                    _ => lir::RefScan::None,
                };
                annotations[block_index][instruction_index] = Some((roots, result_scan));
            }
            for value in instruction_uses(instruction) {
                if let Some(value) = LiveValue::from_value(value) {
                    live.insert(value);
                }
            }
        }
    }

    for (id, block) in function.blocks.iter_mut() {
        let block_index = id.into_raw().into_u32() as usize;
        for (instruction_index, instruction) in block.instructions.iter_mut().enumerate() {
            if let lir::Instruction::NativeCall {
                roots, result_scan, ..
            } = instruction
            {
                let (computed_roots, computed_result_scan) = annotations[block_index]
                    [instruction_index]
                    .take()
                    .expect("every native call receives one liveness annotation");
                *roots = computed_roots;
                *result_scan = computed_result_scan;
            } else if let lir::Instruction::NativeGlobalLoad { roots, .. }
            | lir::Instruction::NativeGlobalStore { roots, .. }
            | lir::Instruction::NativeGlobalAddress { roots, .. } = instruction
            {
                let (computed_roots, result_scan) = annotations[block_index][instruction_index]
                    .take()
                    .expect("every native global bridge receives one liveness annotation");
                debug_assert_eq!(result_scan, lir::RefScan::None);
                *roots = computed_roots;
            }
        }
    }
}

/// Where a MIR local lives in LIR: parameters are SSA values, all
/// other locals get stack slots.
#[derive(Clone, Copy)]
enum LocalSlot {
    Slot(lir::LocalId),
    Param(u32),
}

/// The exact element type carried by a concrete intrinsic class application.
fn array_element<'a>(module: &'a mir::Module, ty: &mir::Type) -> &'a mir::Type {
    mir::array_type(module, ty)
        .map(|(_, element)| element)
        .expect("expected an intrinsic array class application")
}

/// The global symbol of the TypeDescriptor a runtime check / box
/// refers to: classes and interfaces have their own; value types are
/// compared / boxed through their boxed class's (`box$<encoded>`, as
/// mir-lower names it); String uses its typed intrinsic TD. `Any` has no TD
/// (mir-lower folds those checks).
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
        | mir::Type::Unit
        | mir::Type::Ptr(_)
        | mir::Type::FunPtr(_) => td_symbol(&format!("box${}", mir::encode_type(module, ty))),
        mir::Type::Function(_) => td_symbol(&format!("function${}", mir::encode_type(module, ty))),
        mir::Type::Any => {
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
    storage_globals: &'a HashMap<mir::GlobalId, StorageGlobal>,
    /// Sink for trap message globals (`scoop.cstr.N`) and
    /// TypeDescriptor reference stubs (`scoop_td_*`).
    globals: &'a mut Arena<lir::Global>,
    cstr_count: &'a mut usize,
    /// Sink for tuple types encountered in value types (meta layouts).
    layout_types: &'a mut Vec<mir::Type>,
    /// Enum definitions with fixed representations (enum value
    /// sizing, e.g. for `scoop_rt_box` payload sizes).
    enums: &'a Arena<lir::EnumDef>,
    /// Complete class-application to array-metadata mapping produced before
    /// any function is lowered.
    array_types: &'a HashMap<mir::ClassId, lir::ArrayTypeId>,
    /// TypeDescriptor reference stubs, deduplicated by symbol.
    td_map: &'a mut HashMap<String, lir::GlobalId>,
    local_map: HashMap<mir::LocalId, LocalSlot>,
    locals: Arena<lir::Local>,
    temps: Arena<lir::Temp>,
    blocks: Arena<lir::BasicBlock>,
    current: lir::BlockId,
    block_map: HashMap<mir::BlockId, lir::BlockId>,
    current_unwind: Option<lir::BlockId>,
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
    /// Function-local spill slots shared by all landing pads. They
    /// avoid phi nodes when an inner catch cleanup forwards the same
    /// exception to an enclosing try's dispatch block.
    exception_slots: Option<(lir::LocalId, lir::LocalId)>,
    caught_exception: Option<lir::LocalId>,
}

impl<'a> FunctionLowerer<'a> {
    fn array_type_id(&self, class: mir::ClassId) -> lir::ArrayTypeId {
        self.array_types[&class]
    }

    fn array_element(&self, class: mir::ClassId) -> &mir::Type {
        array_element(self.module, &mir::Type::Class(class))
    }

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
        self.current_sealed = true;
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
        let raw = self.new_hidden_local(lir::RAW_PTR);
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
        lir_type(ty)
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
            mir::Expr::CaughtException => mir::Type::Any,
            mir::Expr::TupleLiteral(elements) => {
                mir::Type::Tuple(elements.iter().map(|e| self.expr_ty(e)).collect())
            }
            mir::Expr::StructInit { struct_id, .. } => mir::Type::Struct(*struct_id),
            mir::Expr::ArrayLiteral { array_type, .. } => mir::Type::Class(*array_type),
            mir::Expr::ArrayGet { array_type, .. } => self.array_element(*array_type).clone(),
            mir::Expr::ArrayLen { .. } => mir::Type::Int,
            mir::Expr::ArrayClone { target_type, .. } => mir::Type::Class(*target_type),
            mir::Expr::Local(local) => self.mir_locals[*local].ty.clone(),
            mir::Expr::GlobalRead(global) => self.module.globals[*global].ty.clone(),
            mir::Expr::PtrFromUInt { pointee, .. }
            | mir::Expr::PtrCast { pointee, .. }
            | mir::Expr::PtrOffset { pointee, .. }
            | mir::Expr::AddressOf { pointee, .. }
            | mir::Expr::GlobalAddress { pointee, .. } => {
                mir::Type::Ptr(Box::new(pointee.as_ref().clone()))
            }
            mir::Expr::PtrToUInt(_) | mir::Expr::SizeOf(_) | mir::Expr::AlignOf(_) => {
                mir::Type::UInt
            }
            mir::Expr::PtrLoad { pointee, .. } => pointee.as_ref().clone(),
            mir::Expr::PtrStore { .. } => mir::Type::Unit,
            mir::Expr::FunPtrNull(signature) => mir::Type::FunPtr(*signature),
            mir::Expr::FunctionAddress { callback } => {
                mir::Type::FunPtr(self.module.callback_bridges[*callback].signature)
            }
            mir::Expr::ForeignCallbackRegister { bridge, .. } => {
                mir::Type::Struct(self.module.foreign_callback_bridges[*bridge].callback)
            }
            mir::Expr::ForeignCallbackOperation { result_ty, .. } => result_ty.as_ref().clone(),
            mir::Expr::Retype { ty, .. } => ty.as_ref().clone(),
            mir::Expr::FieldAccess { receiver, index } => match self.expr_ty(receiver) {
                mir::Type::Struct(id) => self.module.structs[id].declared_fields()[*index as usize]
                    .ty
                    .clone(),
                mir::Type::Tuple(elements) => elements[*index as usize].clone(),
                mir::Type::Class(id) => self.module.classes[id].declared_fields()[*index as usize]
                    .ty
                    .clone(),
                // mir-lower only emits field accesses on aggregates
                // and class objects.
                _ => unreachable!("field access on a non-aggregate"),
            },
            mir::Expr::AtomicFieldLoad { .. } | mir::Expr::AtomicFieldCompareExchange { .. } => {
                mir::Type::Int
            }
            mir::Expr::VariantConstruct { ty, .. } => ty.clone(),
            mir::Expr::ClassInit { class_id, .. } => mir::Type::Class(*class_id),
            mir::Expr::ClosureAlloc { class, .. } => {
                mir::Type::Function(self.module.closure_classes[*class].function_type)
            }
            mir::Expr::ClosureCapture { class, index, .. } => self.module.closure_classes[*class]
                .captures[*index as usize]
                .ty
                .clone(),
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
                | mir::BinOp::BoolNe => mir::Type::Boolean,
            },
            mir::Expr::Unary { op, .. } => match op {
                mir::UnOp::IntNeg => mir::Type::Int,
                mir::UnOp::BoolNot => mir::Type::Boolean,
            },
        }
    }

    fn lower_statements(&mut self, statements: &'a [mir::Statement]) {
        for statement in statements {
            if self.current_sealed {
                break;
            }
            self.lower_statement(statement);
        }
    }

    fn lower_statement(&mut self, statement: &'a mir::Statement) {
        match &statement.kind {
            mir::StatementKind::Expr(expr) => {
                let ty = self.expr_ty(expr);
                self.lower_expr(expr, &ty);
            }
            mir::StatementKind::Call(effect) => match effect {
                mir::CallEffect::Unit(call) => {
                    self.lower_call(call, &mir::Type::Unit);
                }
                mir::CallEffect::Value { destination, call } => {
                    let ty = self.mir_locals[*destination].ty.clone();
                    let value = self.lower_call(call, &ty);
                    self.push(lir::Instruction::Store {
                        local: self.local_slot(*destination),
                        value,
                    });
                }
            },
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
            mir::StatementKind::GlobalAssign { global, value } => {
                let ty = self.module.globals[*global].ty.clone();
                let value = self.lower_expr(value, &ty);
                match self.storage_globals[global] {
                    StorageGlobal::Local(global) => {
                        self.push(lir::Instruction::GlobalStore { global, value })
                    }
                    StorageGlobal::Native(global) => {
                        self.push(lir::Instruction::NativeGlobalStore {
                            global,
                            value,
                            roots: Vec::new(),
                        })
                    }
                }
            }
            // `m[i] = v`: bounds check and the element store are
            // codegen's job; the element layout comes from the array
            // operand's type.
            mir::StatementKind::ArraySet {
                array_type,
                array,
                index,
                value,
            } => {
                let array_ty = mir::Type::Class(*array_type);
                let element_ty = self.array_element(*array_type).clone();
                let array = self.lower_expr(array, &array_ty);
                let index = self.lower_expr(index, &mir::Type::Int);
                let value = self.lower_expr(value, &element_ty);
                self.push(lir::Instruction::ArraySet {
                    array,
                    index,
                    value,
                    array_type: self.array_type_id(*array_type),
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
                let field_ty = self.module.classes[*class_id].declared_fields()[*index as usize]
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
            mir::StatementKind::AtomicFieldStore {
                object,
                index,
                value,
            } => {
                let object_ty = self.expr_ty(object);
                let mir::Type::Class(class_id) = &object_ty else {
                    unreachable!("an atomic field store targets a class object")
                };
                assert_eq!(
                    self.module.classes[*class_id].declared_fields()[*index as usize].ty,
                    mir::Type::Int,
                    "an atomic state field is a 64-bit Int"
                );
                let (offsets, _, _) =
                    class_shape(self.module, self.enums, &self.module.classes[*class_id]);
                let object = self.lower_expr(object, &object_ty);
                let value = self.lower_expr(value, &mir::Type::Int);
                self.push(lir::Instruction::AtomicStore {
                    object,
                    offset: offsets[*index as usize],
                    value,
                });
            }
            mir::StatementKind::Eh(eh) => self.lower_eh_statement(eh),
        }
    }

    fn lower_eh_statement(&mut self, statement: &mir::EhStatement) {
        match statement {
            mir::EhStatement::LandingPad { cleanup } => {
                let (record_slot, raw_slot) = self.exception_slots();
                let record = self.new_temp(lir::LirType::ExceptionRecord);
                let raw = self.new_temp(lir::RAW_PTR);
                if *cleanup {
                    self.push(lir::Instruction::CleanupPad { record, raw });
                } else {
                    self.push(lir::Instruction::LandingPad { record, raw });
                }
                self.push(lir::Instruction::Store {
                    local: record_slot,
                    value: lir::Value::Temp(record),
                });
                self.push(lir::Instruction::Store {
                    local: raw_slot,
                    value: lir::Value::Temp(raw),
                });
            }
            mir::EhStatement::BeginCatch => {
                let (_, raw_slot) = self.exception_slots();
                let exception = self.new_temp(lir::MANAGED_PTR);
                self.push(lir::Instruction::BeginCatch {
                    out: exception,
                    raw: lir::Value::Local(raw_slot),
                });
                let slot = match self.caught_exception {
                    Some(slot) => slot,
                    None => {
                        let slot = self.new_hidden_local(lir::MANAGED_PTR);
                        self.caught_exception = Some(slot);
                        slot
                    }
                };
                self.push(lir::Instruction::Store {
                    local: slot,
                    value: lir::Value::Temp(exception),
                });
            }
            mir::EhStatement::EndCatch => self.push(lir::Instruction::EndCatch),
        }
    }

    fn lower_terminator(&mut self, terminator: &mir::Terminator) {
        match terminator {
            mir::Terminator::Goto(target) => {
                self.seal(lir::Terminator::Br(self.block_map[target]));
            }
            mir::Terminator::Branch {
                cond,
                then_block,
                else_block,
            } => {
                let cond = self.lower_expr(cond, &mir::Type::Boolean);
                self.seal(lir::Terminator::CondBr {
                    cond,
                    then_block: self.block_map[then_block],
                    else_block: self.block_map[else_block],
                });
            }
            mir::Terminator::Return { value } => {
                let value = match (self.returns_void, value) {
                    (true, None) => None,
                    (true, Some(value)) => {
                        self.lower_expr(value, &mir::Type::Unit);
                        None
                    }
                    (false, Some(value)) => {
                        let ty = self.mir_return_ty.clone();
                        Some(self.lower_expr(value, &ty))
                    }
                    (false, None) => unreachable!("non-Unit return without a value"),
                };
                self.seal(lir::Terminator::Return { value });
            }
            mir::Terminator::Throw { exception, unwind } => {
                let ty = self.expr_ty(exception);
                let value = self.lower_expr(exception, &ty);
                if let Some(unwind) = unwind {
                    let normal = self.new_block("throw.normal");
                    self.push(lir::Instruction::Invoke {
                        out: None,
                        symbol: THROW_SYMBOL.to_string(),
                        args: vec![value],
                        normal,
                        unwind: self.block_map[unwind],
                    });
                    self.seal(lir::Terminator::Br(normal));
                    self.enter(normal);
                    self.seal(lir::Terminator::Unreachable);
                } else {
                    self.push(lir::Instruction::Throw { exception: value });
                    self.seal(lir::Terminator::Unreachable);
                }
            }
            mir::Terminator::Rethrow { unwind } => match unwind {
                Some(unwind) => {
                    let normal = self.new_block("rethrow.normal");
                    self.push(lir::Instruction::Invoke {
                        out: None,
                        symbol: RETHROW_SYMBOL.to_string(),
                        args: Vec::new(),
                        normal,
                        unwind: self.block_map[unwind],
                    });
                    self.seal(lir::Terminator::Br(normal));
                    self.enter(normal);
                    self.seal(lir::Terminator::Unreachable);
                }
                None => {
                    self.push(lir::Instruction::Call {
                        out: None,
                        symbol: RETHROW_SYMBOL.to_string(),
                        args: Vec::new(),
                    });
                    self.seal(lir::Terminator::Unreachable);
                }
            },
            mir::Terminator::Resume => {
                let (record, _) = self.exception_slots();
                self.seal(lir::Terminator::Resume {
                    exception: lir::Value::Local(record),
                });
            }
            mir::Terminator::Trap { message } => {
                let message = self.module.strings[*message].value.clone();
                let trap = self.trap_block(&message);
                self.seal(lir::Terminator::Br(trap));
            }
            mir::Terminator::Unreachable => self.seal(lir::Terminator::Unreachable),
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
            mir::Expr::CaughtException => lir::Value::Local(
                self.caught_exception
                    .expect("CaughtException must be dominated by BeginCatch"),
            ),
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
                    .declared_fields()
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
                let field_types: Vec<mir::Type> = def
                    .declared_fields()
                    .iter()
                    .map(|field| field.ty.clone())
                    .collect();
                assert_eq!(
                    args.len(),
                    field_types.len(),
                    "a ClassInit initializes every flattened field"
                );
                let td = self.td_ref(&mir::Type::Class(*class_id));
                let out = self.new_temp(lir::MANAGED_PTR);
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
            mir::Expr::ClosureAlloc { class, captures } => {
                let def = &self.module.closure_classes[*class];
                let (capture_offsets, size, _, _) = closure_shape(self.module, self.enums, def);
                let capture_types: Vec<_> = def
                    .captures
                    .iter()
                    .map(|capture| capture.ty.clone())
                    .collect();
                assert_eq!(
                    captures.len(),
                    capture_types.len(),
                    "ClosureAlloc initializes every capture field"
                );
                let td = lir::Value::Global(self.td_global(td_symbol(&def.name)));
                let out = self.new_temp(lir::MANAGED_PTR);
                self.push(lir::Instruction::Call {
                    out: Some(out),
                    symbol: ALLOC_SYMBOL.to_string(),
                    args: vec![td, lir::Value::IntConst(size as i64)],
                });
                let invoke_function = self.module.closure_invoke_functions[def.invoke].function;
                let invoke_symbol = self.module.functions[invoke_function].symbol.clone();
                let invoke = self.new_temp(lir::CODE_PTR);
                self.push(lir::Instruction::FunctionAddress {
                    out: invoke,
                    symbol: invoke_symbol,
                });
                self.push(lir::Instruction::HeapStore {
                    object: lir::Value::Temp(out),
                    offset: 16,
                    value: lir::Value::Temp(invoke),
                });
                for ((capture, capture_ty), offset) in
                    captures.iter().zip(&capture_types).zip(capture_offsets)
                {
                    let value = self.lower_expr(capture, capture_ty);
                    self.push(lir::Instruction::HeapStore {
                        object: lir::Value::Temp(out),
                        offset,
                        value,
                    });
                }
                lir::Value::Temp(out)
            }
            mir::Expr::ClosureCapture {
                closure,
                class,
                index,
            } => {
                let def = &self.module.closure_classes[*class];
                let (capture_offsets, _, _, _) = closure_shape(self.module, self.enums, def);
                let closure_ty = self.expr_ty(closure);
                let closure = self.lower_expr(closure, &closure_ty);
                let out_ty = self.value_type(ty);
                let out = self.load_at_offset(closure, capture_offsets[*index as usize], out_ty);
                lir::Value::Temp(out)
            }
            // Every operation consumes the exact array application carried by
            // MIR. The LIR value itself is just a managed pointer.
            mir::Expr::ArrayLiteral {
                array_type,
                elements,
            } => {
                let element_ty = self.array_element(*array_type).clone();
                let elements: Vec<lir::Value> = elements
                    .iter()
                    .map(|element| self.lower_expr(element, &element_ty))
                    .collect();
                let out_ty = self.value_type(ty);
                let out = self.new_temp(out_ty);
                self.push(lir::Instruction::ArrayAlloc {
                    out,
                    elements,
                    array_type: self.array_type_id(*array_type),
                });
                lir::Value::Temp(out)
            }
            mir::Expr::ArrayGet {
                array_type,
                array,
                index,
            } => {
                let array_ty = mir::Type::Class(*array_type);
                let array = self.lower_expr(array, &array_ty);
                let index = self.lower_expr(index, &mir::Type::Int);
                let out_ty = self.value_type(ty);
                let out = self.new_temp(out_ty);
                self.push(lir::Instruction::ArrayGet {
                    out,
                    array,
                    index,
                    array_type: self.array_type_id(*array_type),
                });
                lir::Value::Temp(out)
            }
            mir::Expr::ArrayLen {
                array_type,
                operand,
            } => {
                let operand_ty = mir::Type::Class(*array_type);
                let operand = self.lower_expr(operand, &operand_ty);
                let out = self.new_temp(lir::LirType::I64);
                self.push(lir::Instruction::ArrayLen {
                    out,
                    operand,
                    array_type: self.array_type_id(*array_type),
                });
                lir::Value::Temp(out)
            }
            mir::Expr::ArrayClone {
                source_type,
                target_type,
                operand,
            } => {
                let operand_ty = mir::Type::Class(*source_type);
                let operand = self.lower_expr(operand, &operand_ty);
                let out_ty = self.value_type(ty);
                let out = self.new_temp(out_ty);
                self.push(lir::Instruction::ArrayClone {
                    out,
                    operand,
                    array_type: self.array_type_id(*target_type),
                });
                lir::Value::Temp(out)
            }
            mir::Expr::Local(local) => self.local_value(*local),
            mir::Expr::GlobalRead(global) => {
                let out_ty = self.value_type(ty);
                let out = self.new_temp(out_ty);
                match self.storage_globals[global] {
                    StorageGlobal::Local(global) => {
                        self.push(lir::Instruction::GlobalLoad { out, global })
                    }
                    StorageGlobal::Native(global) => {
                        self.push(lir::Instruction::NativeGlobalLoad {
                            out,
                            global,
                            roots: Vec::new(),
                        })
                    }
                }
                lir::Value::Temp(out)
            }
            mir::Expr::PtrFromUInt { operand, .. } => {
                let value = self.lower_expr(operand, &mir::Type::UInt);
                let out = self.new_temp(lir::RAW_PTR);
                self.push(lir::Instruction::IntToPtr { out, value });
                lir::Value::Temp(out)
            }
            mir::Expr::PtrToUInt(operand) => {
                let pointer_ty = self.expr_ty(operand);
                let value = self.lower_expr(operand, &pointer_ty);
                let out = self.new_temp(lir::LirType::I64);
                self.push(lir::Instruction::PtrToInt { out, value });
                lir::Value::Temp(out)
            }
            mir::Expr::PtrCast { operand, .. } => {
                let source_ty = self.expr_ty(operand);
                self.lower_expr(operand, &source_ty)
            }
            mir::Expr::PtrLoad {
                pointer,
                pointee,
                offset,
            } => {
                let pointer_ty = self.expr_ty(pointer);
                let pointer = self.lower_expr(pointer, &pointer_ty);
                let pointer = if let Some(offset) = offset {
                    let offset = self.lower_expr(offset, &mir::Type::Int);
                    self.offset_pointer(pointer, pointee, offset, false)
                } else {
                    pointer
                };
                let enum_shape = |id: mir::EnumId| repr_shape(&self.enums[enum_def_id(id)].repr);
                let (_, align) = size_align(self.module, &enum_shape, pointee);
                let out_ty = self.value_type(pointee);
                let out = self.new_temp(out_ty);
                self.push(lir::Instruction::RawLoad {
                    out,
                    pointer,
                    align,
                });
                lir::Value::Temp(out)
            }
            mir::Expr::PtrStore {
                pointer,
                pointee,
                offset,
                value,
            } => {
                let pointer_ty = self.expr_ty(pointer);
                let pointer = self.lower_expr(pointer, &pointer_ty);
                let pointer = if let Some(offset) = offset {
                    let offset = self.lower_expr(offset, &mir::Type::Int);
                    self.offset_pointer(pointer, pointee, offset, false)
                } else {
                    pointer
                };
                let value = self.lower_expr(value, pointee);
                let enum_shape = |id: mir::EnumId| repr_shape(&self.enums[enum_def_id(id)].repr);
                let (_, align) = size_align(self.module, &enum_shape, pointee);
                self.push(lir::Instruction::RawStore {
                    pointer,
                    value,
                    align,
                });
                self.unit_value()
            }
            mir::Expr::PtrOffset {
                pointer,
                pointee,
                offset,
                subtract,
            } => {
                let pointer_ty = self.expr_ty(pointer);
                let pointer = self.lower_expr(pointer, &pointer_ty);
                let offset = self.lower_expr(offset, &mir::Type::Int);
                self.offset_pointer(pointer, pointee, offset, *subtract)
            }
            mir::Expr::AddressOf { local, .. } => {
                let local = self.local_slot(*local);
                let out = self.new_temp(lir::RAW_PTR);
                self.push(lir::Instruction::LocalAddress { out, local });
                lir::Value::Temp(out)
            }
            mir::Expr::GlobalAddress { global, .. } => {
                let out = self.new_temp(lir::RAW_PTR);
                match self.storage_globals[global] {
                    StorageGlobal::Local(global) => {
                        self.push(lir::Instruction::GlobalAddress { out, global })
                    }
                    StorageGlobal::Native(global) => {
                        self.push(lir::Instruction::NativeGlobalAddress {
                            out,
                            global,
                            roots: Vec::new(),
                        })
                    }
                }
                lir::Value::Temp(out)
            }
            mir::Expr::SizeOf(value_ty) => {
                let enum_shape = |id: mir::EnumId| repr_shape(&self.enums[enum_def_id(id)].repr);
                let (size, _) = size_align(self.module, &enum_shape, value_ty);
                lir::Value::IntConst(size as i64)
            }
            mir::Expr::AlignOf(value_ty) => {
                let enum_shape = |id: mir::EnumId| repr_shape(&self.enums[enum_def_id(id)].repr);
                let (_, align) = size_align(self.module, &enum_shape, value_ty);
                lir::Value::IntConst(align as i64)
            }
            mir::Expr::FunPtrNull(_) => lir::Value::NullPtr,
            mir::Expr::FunctionAddress { callback } => {
                let out = self.new_temp(lir::CODE_PTR);
                self.push(lir::Instruction::FunctionAddress {
                    out,
                    symbol: format!("scoop_c_callback_{}", callback.into_raw().into_u32()),
                });
                lir::Value::Temp(out)
            }
            mir::Expr::ForeignCallbackRegister { bridge, closure } => {
                let closure_ty = self.expr_ty(closure);
                let closure = self.lower_expr(closure, &closure_ty);
                let out_ty = self.value_type(ty);
                let out = self.new_temp(out_ty);
                self.push(lir::Instruction::ForeignCallbackRegister {
                    out,
                    bridge: la_arena::Idx::from_raw(bridge.into_raw()),
                    closure,
                });
                lir::Value::Temp(out)
            }
            mir::Expr::ForeignCallbackOperation {
                operation,
                callback,
                ..
            } => {
                let callback_ty = self.expr_ty(callback);
                let callback = self.lower_expr(callback, &callback_ty);
                let operation = match operation {
                    mir::ForeignCallbackOperation::Retain => lir::ForeignCallbackOperation::Retain,
                    mir::ForeignCallbackOperation::Release => {
                        lir::ForeignCallbackOperation::Release
                    }
                    mir::ForeignCallbackOperation::State => lir::ForeignCallbackOperation::State,
                    mir::ForeignCallbackOperation::Failure => {
                        lir::ForeignCallbackOperation::Failure
                    }
                };
                if operation == lir::ForeignCallbackOperation::Release {
                    self.push(lir::Instruction::ForeignCallbackOperation {
                        out: None,
                        operation,
                        callback,
                    });
                    self.unit_value()
                } else {
                    let out_ty = self.value_type(ty);
                    let out = self.new_temp(out_ty);
                    self.push(lir::Instruction::ForeignCallbackOperation {
                        out: Some(out),
                        operation,
                        callback,
                    });
                    lir::Value::Temp(out)
                }
            }
            mir::Expr::Retype { operand, ty } => self.lower_expr(operand, ty),
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
            mir::Expr::AtomicFieldLoad { object, index } => {
                let object_ty = self.expr_ty(object);
                let mir::Type::Class(class_id) = &object_ty else {
                    unreachable!("an atomic field load targets a class object")
                };
                assert_eq!(
                    self.module.classes[*class_id].declared_fields()[*index as usize].ty,
                    mir::Type::Int,
                    "an atomic state field is a 64-bit Int"
                );
                let (offsets, _, _) =
                    class_shape(self.module, self.enums, &self.module.classes[*class_id]);
                let object = self.lower_expr(object, &object_ty);
                let out = self.new_temp(lir::LirType::I64);
                self.push(lir::Instruction::AtomicLoad {
                    out,
                    object,
                    offset: offsets[*index as usize],
                });
                lir::Value::Temp(out)
            }
            mir::Expr::AtomicFieldCompareExchange {
                object,
                index,
                expected,
                replacement,
            } => {
                let object_ty = self.expr_ty(object);
                let mir::Type::Class(class_id) = &object_ty else {
                    unreachable!("an atomic compare-exchange targets a class object")
                };
                assert_eq!(
                    self.module.classes[*class_id].declared_fields()[*index as usize].ty,
                    mir::Type::Int,
                    "an atomic state field is a 64-bit Int"
                );
                let (offsets, _, _) =
                    class_shape(self.module, self.enums, &self.module.classes[*class_id]);
                let object = self.lower_expr(object, &object_ty);
                let expected = self.lower_expr(expected, &mir::Type::Int);
                let replacement = self.lower_expr(replacement, &mir::Type::Int);
                let out = self.new_temp(lir::LirType::I64);
                self.push(lir::Instruction::AtomicCompareExchange {
                    out,
                    object,
                    offset: offsets[*index as usize],
                    expected,
                    replacement,
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
                let out = self.new_temp(lir::MANAGED_PTR);
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
            mir::Expr::Binary { op, lhs, rhs } => {
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

    fn offset_pointer(
        &mut self,
        pointer: lir::Value,
        pointee: &mir::Type,
        offset: lir::Value,
        subtract: bool,
    ) -> lir::Value {
        let enum_shape = |id: mir::EnumId| repr_shape(&self.enums[enum_def_id(id)].repr);
        let (size, _) = size_align(self.module, &enum_shape, pointee);
        let bytes = if size == 1 {
            offset
        } else {
            let out = self.new_temp(lir::LirType::I64);
            self.push(lir::Instruction::BinOp {
                out,
                op: lir::BinOp::Mul,
                lhs: offset,
                rhs: lir::Value::IntConst(size as i64),
            });
            lir::Value::Temp(out)
        };
        let bytes = if subtract {
            let out = self.new_temp(lir::LirType::I64);
            self.push(lir::Instruction::UnaryOp {
                out,
                op: lir::UnOp::Neg,
                operand: bytes,
            });
            lir::Value::Temp(out)
        } else {
            bytes
        };
        let out = self.new_temp(lir::RAW_PTR);
        self.push(lir::Instruction::PtrOffset {
            out,
            pointer,
            bytes,
        });
        lir::Value::Temp(out)
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
            address_kind: lir::PointerKind::Raw,
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
            address_kind: lir::PointerKind::Metadata,
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

    fn lower_call(&mut self, call: &mir::Call, result_ty: &mir::Type) -> lir::Value {
        match call.target.callee {
            mir::Callee::Extern(id) => {
                assert!(matches!(call.target.kind, mir::CallKind::Direct));
                let extern_ = &self.module.extern_functions[id];
                let parameter_types = extern_.params.clone();
                let returns_unit = extern_.return_type == mir::Type::Unit;
                let args = call
                    .args
                    .iter()
                    .zip(&parameter_types)
                    .map(|(arg, ty)| self.lower_expr(arg, ty))
                    .collect::<Vec<_>>();
                let function = lir::ExternFunctionId::from_raw(id.into_raw());
                match extern_.abi {
                    mir::ExternAbi::C => {
                        let result = (!returns_unit).then(|| {
                            let ty = self.value_type(result_ty);
                            self.new_temp(ty)
                        });
                        let mut bridge_args = Vec::with_capacity(args.len());
                        for (value, ty) in args.into_iter().zip(parameter_types) {
                            let ty = self.value_type(&ty);
                            let local = self.new_hidden_local(ty);
                            self.push(lir::Instruction::Store { local, value });
                            let address = self.new_temp(lir::RAW_PTR);
                            self.push(lir::Instruction::LocalAddress {
                                out: address,
                                local,
                            });
                            bridge_args.push(lir::Value::Temp(address));
                        }
                        self.push(lir::Instruction::NativeCall {
                            out: result,
                            function,
                            effect: lir::NativeCallEffect::NativeSafe,
                            args: bridge_args,
                            roots: Vec::new(),
                            result_scan: lir::RefScan::None,
                        });
                        result.map_or_else(|| self.unit_value(), lir::Value::Temp)
                    }
                    mir::ExternAbi::Scoop => {
                        if returns_unit {
                            self.push(lir::Instruction::NativeCall {
                                out: None,
                                function,
                                effect: lir::NativeCallEffect::NativeBorrowed,
                                args,
                                roots: Vec::new(),
                                result_scan: lir::RefScan::None,
                            });
                            self.unit_value()
                        } else {
                            let ty = self.value_type(result_ty);
                            let out = self.new_temp(ty);
                            self.push(lir::Instruction::NativeCall {
                                out: Some(out),
                                function,
                                effect: lir::NativeCallEffect::NativeBorrowed,
                                args,
                                roots: Vec::new(),
                                result_scan: lir::RefScan::None,
                            });
                            lir::Value::Temp(out)
                        }
                    }
                }
            }
            mir::Callee::FunctionBridge(function_type) => {
                let signature = self.module.function_types[function_type].clone();
                let mut parameter_types = Vec::with_capacity(call.args.len());
                parameter_types.push(mir::Type::Any);
                parameter_types.extend(signature.parameter_types);
                for arg in call.args.iter().skip(parameter_types.len()) {
                    parameter_types.push(self.expr_ty(arg));
                }
                assert_eq!(
                    call.args.len(),
                    parameter_types.len(),
                    "only suspend function bridges add a hidden continuation argument"
                );
                let args: Vec<lir::Value> = call
                    .args
                    .iter()
                    .zip(&parameter_types)
                    .map(|(arg, ty)| self.lower_expr(arg, ty))
                    .collect();
                let td = self.load_at_offset(args[0], 0, lir::METADATA_PTR);
                let target_td = self.td_ref(&mir::Type::Function(function_type));
                let table = self.new_temp(lir::METADATA_PTR);
                self.push(lir::Instruction::Call {
                    out: Some(table),
                    symbol: mir::RuntimeFn::ITableLookup.symbol().to_string(),
                    args: vec![lir::Value::Temp(td), target_td],
                });
                self.finish_indirect(
                    table,
                    0,
                    args,
                    !signature.is_suspend && signature.return_type == mir::Type::Unit,
                    result_ty,
                )
            }
            mir::Callee::Closure(function_type) => {
                let signature = self.module.function_types[function_type].clone();
                let mut parameter_types = Vec::with_capacity(call.args.len());
                parameter_types.push(mir::Type::Function(function_type));
                parameter_types.extend(signature.parameter_types);
                for arg in call.args.iter().skip(parameter_types.len()) {
                    parameter_types.push(self.expr_ty(arg));
                }
                assert_eq!(
                    call.args.len(),
                    parameter_types.len(),
                    "only suspend closure calls add a hidden continuation argument"
                );
                let args: Vec<lir::Value> = call
                    .args
                    .iter()
                    .zip(&parameter_types)
                    .map(|(arg, ty)| self.lower_expr(arg, ty))
                    .collect();
                self.finish_closure(
                    args[0],
                    args,
                    !signature.is_suspend && signature.return_type == mir::Type::Unit,
                    result_ty,
                )
            }
            mir::Callee::User(_) | mir::Callee::Monomorphized(_) => {
                let id = match call.target.callee {
                    mir::Callee::User(id) => id,
                    mir::Callee::Monomorphized(instance) => {
                        self.module.meta.instances[instance].function
                    }
                    mir::Callee::CoroutineSuspend { .. } | mir::Callee::Runtime(_) => {
                        unreachable!("matched a local callee above")
                    }
                    mir::Callee::Closure(_) | mir::Callee::FunctionBridge(_) => {
                        unreachable!("handled above")
                    }
                    mir::Callee::Extern(_) => unreachable!("handled above"),
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
                        let td = self.load_at_offset(args[0], 0, lir::METADATA_PTR);
                        let vtable =
                            self.load_at_offset(lir::Value::Temp(td), 5 * 8, lir::METADATA_PTR);
                        self.finish_indirect(vtable, slot, args, returns_unit, result_ty)
                    }
                    // itable dispatch: `scoop_rt_itable_lookup(td,
                    // iface_td)` finds the interface's table by its
                    // TypeDescriptor key.
                    mir::CallKind::Interface { interface, slot } => {
                        let td = self.load_at_offset(args[0], 0, lir::METADATA_PTR);
                        let iface_td = self.td_ref(&mir::Type::Interface(interface));
                        let table = self.new_temp(lir::METADATA_PTR);
                        self.push(lir::Instruction::Call {
                            out: Some(table),
                            symbol: mir::RuntimeFn::ITableLookup.symbol().to_string(),
                            args: vec![lir::Value::Temp(td), iface_td],
                        });
                        self.finish_indirect(table, slot, args, returns_unit, result_ty)
                    }
                    mir::CallKind::Closure { .. } => {
                        unreachable!("closure calls have no statically selected user callee")
                    }
                    mir::CallKind::FunctionBridge { .. } => {
                        unreachable!("function bridge calls have no static user callee")
                    }
                }
            }
            mir::Callee::CoroutineSuspend { .. } => {
                unreachable!("coroutine state-machine lowering removes suspend markers")
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
                    mir::RuntimeFn::MaterializeException => vec![mir::Type::Any],
                    // The M6 runtime functions are emitted by dedicated
                    // Box / IsInstance / dispatch lowerings, never as plain
                    // MIR calls.
                    mir::RuntimeFn::Box
                    | mir::RuntimeFn::IsInstance
                    | mir::RuntimeFn::ITableLookup => {
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
                        self.call_with_result(symbol, args, lir::MANAGED_PTR)
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
                    mir::RuntimeFn::Unpin
                    | mir::RuntimeFn::ReleaseHandle
                    | mir::RuntimeFn::MaterializeException => {
                        self.call_with_result(symbol, args, lir::MANAGED_PTR)
                    }
                    mir::RuntimeFn::GcCollect => {
                        self.push(lir::Instruction::Call {
                            out: None,
                            symbol,
                            args,
                        });
                        self.unit_value()
                    }
                    mir::RuntimeFn::Box
                    | mir::RuntimeFn::IsInstance
                    | mir::RuntimeFn::ITableLookup => {
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
        if let Some(unwind) = self.current_unwind {
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
        if let Some(unwind) = self.current_unwind {
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

    /// A managed closure call through the code pointer already loaded from
    /// the closure object. Its unwind behavior is identical to direct and
    /// table-indirect managed calls.
    fn finish_closure(
        &mut self,
        closure: lir::Value,
        args: Vec<lir::Value>,
        returns_unit: bool,
        result_ty: &mir::Type,
    ) -> lir::Value {
        if let Some(unwind) = self.current_unwind {
            let normal = self.new_block("invoke.normal");
            let out = if returns_unit {
                None
            } else {
                let ty = self.value_type(result_ty);
                Some(self.new_temp(ty))
            };
            self.push(lir::Instruction::InvokeIndirect {
                out,
                table: closure,
                slot: 2,
                args,
                normal,
                unwind,
            });
            self.seal(lir::Terminator::Br(normal));
            self.enter(normal);
            return out.map_or_else(|| self.unit_value(), lir::Value::Temp);
        }
        if returns_unit {
            self.push(lir::Instruction::CallIndirect {
                out: None,
                table: closure,
                slot: 2,
                args,
            });
            self.unit_value()
        } else {
            let ty = self.value_type(result_ty);
            let out = self.new_temp(ty);
            self.push(lir::Instruction::CallIndirect {
                out: Some(out),
                table: closure,
                slot: 2,
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
        extern_functions: Arena<mir::ExternFunction>,
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
                extern_functions: Arena::new(),
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
            let payload_gc_free = self.type_gc_free(&payload);
            self.enums.alloc(mir::EnumDef {
                name: name.to_string(),
                gc_free: payload_gc_free,
                variants: vec![
                    mir::VariantDef {
                        name: "Some".to_string(),
                        gc_free: payload_gc_free,
                        fields: vec![mir::Field {
                            name: "_1".to_string(),
                            ty: payload,
                        }],
                    },
                    mir::VariantDef {
                        name: "None".to_string(),
                        gc_free: true,
                        fields: Vec::new(),
                    },
                ],
            })
        }

        fn type_gc_free(&self, ty: &mir::Type) -> bool {
            match ty {
                mir::Type::Unit
                | mir::Type::Int
                | mir::Type::UInt
                | mir::Type::Boolean
                | mir::Type::Ptr(_)
                | mir::Type::FunPtr(_) => true,
                mir::Type::String
                | mir::Type::Class(_)
                | mir::Type::Interface(_)
                | mir::Type::Any
                | mir::Type::Function(_) => false,
                mir::Type::Struct(id) => self.structs[*id].gc_free,
                mir::Type::Enum(id, _) => self.enums[*id].gc_free,
                mir::Type::Tuple(elements) => {
                    elements.iter().all(|element| self.type_gc_free(element))
                }
            }
        }

        fn string(&mut self, value: &str) -> mir::StringConstId {
            let symbol = format!("scoop.str.{}", self.strings.len());
            self.strings.alloc(mir::StringConst {
                value: value.to_string(),
                symbol,
            })
        }

        fn managed_scoop_extern(
            &mut self,
            source_name: &str,
            native_symbol: &str,
            params: Vec<mir::Type>,
            return_type: mir::Type,
        ) -> mir::ExternFunctionId {
            self.extern_functions.alloc(mir::ExternFunction {
                source_name: source_name.to_string(),
                native_symbol: native_symbol.to_string(),
                library: String::new(),
                abi: mir::ExternAbi::Scoop,
                calling_convention: mir::CallingConvention::Cdecl,
                gc_effect: mir::GcEffect::Managed,
                params,
                return_type,
            })
        }

        fn strukt(&mut self, name: &str, fields: &[(&str, mir::Type)]) -> mir::StructId {
            let gc_free = fields.iter().all(|(_, ty)| self.type_gc_free(ty));
            self.structs.alloc(mir::StructDef {
                name: name.to_string(),
                gc_free,
                representation: mir::StructRepresentation::Declared {
                    c_layout: None,
                    interior_mutable: false,
                    fields: fields
                        .iter()
                        .map(|(name, ty)| mir::Field {
                            name: name.to_string(),
                            ty: ty.clone(),
                        })
                        .collect(),
                },
            })
        }

        fn c_strukt(
            &mut self,
            name: &str,
            aligned: u8,
            packed: u8,
            interior_mutable: bool,
            fields: &[(&str, mir::Type)],
        ) -> mir::StructId {
            let gc_free = fields.iter().all(|(_, ty)| self.type_gc_free(ty));
            self.structs.alloc(mir::StructDef {
                name: name.to_string(),
                gc_free,
                representation: mir::StructRepresentation::Declared {
                    c_layout: Some(mir::CLayout { aligned, packed }),
                    interior_mutable,
                    fields: fields
                        .iter()
                        .map(|(name, ty)| mir::Field {
                            name: name.to_string(),
                            ty: ty.clone(),
                        })
                        .collect(),
                },
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
                representation: mir::ClassRepresentation::Declared {
                    fields: fields
                        .iter()
                        .map(|(name, ty)| mir::Field {
                            name: name.to_string(),
                            ty: ty.clone(),
                        })
                        .collect(),
                    base_class: base,
                },
                interfaces: Vec::new(),
                vtable,
                itables,
            })
        }

        fn array_class(
            &mut self,
            name: &str,
            kind: mir::ArrayKind,
            element: mir::Type,
        ) -> mir::ClassId {
            self.classes.alloc(mir::ClassDef {
                modifier: mir::ClassModifier::Final,
                name: name.to_string(),
                representation: mir::ClassRepresentation::Intrinsic(match kind {
                    mir::ArrayKind::Immutable => {
                        mir::IntrinsicTypeRepresentation::Array { element }
                    }
                    mir::ArrayKind::Mutable => {
                        mir::IntrinsicTypeRepresentation::MutableArray { element }
                    }
                }),
                interfaces: Vec::new(),
                vtable: Vec::new(),
                itables: Vec::new(),
            })
        }

        fn array(&mut self, name: &str, element: mir::Type) -> mir::Type {
            mir::Type::Class(self.array_class(name, mir::ArrayKind::Immutable, element))
        }

        fn mutable_array(&mut self, name: &str, element: mir::Type) -> mir::Type {
            mir::Type::Class(self.array_class(name, mir::ArrayKind::Mutable, element))
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
                gc_effect: mir::GcEffect::Managed,
                name: name.to_string(),
                symbol: symbol.to_string(),
                params,
                return_ty,
                body: mir::Body::unreachable(Arena::new()),
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
            let mut blocks = Arena::new();
            let terminator = if return_ty == mir::Type::Unit {
                mir::Terminator::Return { value: None }
            } else {
                mir::Terminator::Unreachable
            };
            let entry = blocks.alloc(mir::BasicBlock {
                name: "entry".to_string(),
                statements,
                terminator,
                unwind: None,
            });
            let id = self.functions.alloc(mir::Function {
                gc_effect: mir::GcEffect::Managed,
                name: name.to_string(),
                symbol: symbol.to_string(),
                params,
                return_ty,
                body: mir::Body {
                    locals,
                    blocks,
                    entry,
                },
            });
            self.top_level.push(id);
            id
        }

        fn user_fn_body(
            &mut self,
            name: &str,
            symbol: &str,
            params: Vec<mir::Param>,
            return_ty: mir::Type,
            body: mir::Body,
        ) -> mir::FunctionId {
            let id = self.functions.alloc(mir::Function {
                gc_effect: mir::GcEffect::Managed,
                name: name.to_string(),
                symbol: symbol.to_string(),
                params,
                return_ty,
                body,
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

        fn finish(mut self, entry: mir::FunctionId) -> mir::Module {
            for (name, representation) in [
                ("Int", mir::IntrinsicTypeRepresentation::Int),
                ("UInt", mir::IntrinsicTypeRepresentation::UInt),
                ("Boolean", mir::IntrinsicTypeRepresentation::Boolean),
            ] {
                self.structs.alloc(mir::StructDef {
                    name: name.to_string(),
                    gc_free: true,
                    representation: mir::StructRepresentation::Intrinsic(representation),
                });
            }
            self.classes.alloc(mir::ClassDef {
                modifier: mir::ClassModifier::Final,
                name: "String".to_string(),
                representation: mir::ClassRepresentation::Intrinsic(
                    mir::IntrinsicTypeRepresentation::String,
                ),
                interfaces: Vec::new(),
                vtable: Vec::new(),
                itables: Vec::new(),
            });
            mir::Module {
                functions: self.functions,
                extern_functions: self.extern_functions,
                globals: Arena::new(),
                callback_bridges: Arena::new(),
                foreign_callback_adapters: Arena::new(),
                foreign_callback_bridges: Arena::new(),
                function_types: Arena::new(),
                closure_classes: Arena::new(),
                closure_invoke_functions: Arena::new(),
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

    fn array_metadata<'a>(module: &'a lir::Module, name: &str) -> &'a lir::ArrayType {
        module
            .meta
            .arrays
            .iter()
            .find_map(|(_, array)| (array.type_descriptor.name == name).then_some(array))
            .unwrap_or_else(|| panic!("missing array metadata for {name}"))
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

    #[test]
    fn no_gc_effect_is_preserved_in_lir() {
        let mut builder = Builder::new();
        let main = builder.main(Arena::new(), Vec::new());
        builder.functions[main].gc_effect = mir::GcEffect::NoGc;
        let module = lower(&builder.finish(main));
        assert_eq!(module.functions[0].gc_effect, lir::GcEffect::NoGc);
        assert!(lir::dump(&module).contains("-> void <no-gc>"));
    }

    fn runtime_call(function: mir::RuntimeFn, args: Vec<mir::Expr>) -> mir::Call {
        mir::Call {
            target: mir::CallTarget {
                kind: mir::CallKind::Direct,
                callee: mir::Callee::Runtime(function),
            },
            args,
        }
    }

    fn extern_call(function: mir::ExternFunctionId, args: Vec<mir::Expr>) -> mir::Call {
        mir::Call {
            target: mir::CallTarget {
                kind: mir::CallKind::Direct,
                callee: mir::Callee::Extern(function),
            },
            args,
        }
    }

    fn user_call(function: mir::FunctionId) -> mir::Call {
        mir::Call {
            target: mir::CallTarget {
                kind: mir::CallKind::Direct,
                callee: mir::Callee::User(function),
            },
            args: Vec::new(),
        }
    }

    fn call_stmt(call: mir::Call) -> mir::Statement {
        stmt(mir::StatementKind::Call(mir::CallEffect::Unit(call)))
    }

    fn call_value(destination: mir::LocalId, call: mir::Call) -> mir::Statement {
        stmt(mir::StatementKind::Call(mir::CallEffect::Value {
            destination,
            call,
        }))
    }

    /// `main` writes `"hello, world"` (core's managed `write` extern)
    /// then calls `helper()`, which writes `"!"`.
    fn hello_world() -> mir::Module {
        let mut b = Builder::new();
        let hello = b.string("hello, world");
        let bang = b.string("!");
        let write = b.managed_scoop_extern(
            "write",
            "scoop_rt_write",
            vec![mir::Type::String],
            mir::Type::Unit,
        );
        let helper = b.user_fn(
            "helper",
            "scoop.helper",
            Arena::new(),
            vec![call_stmt(extern_call(
                write,
                vec![mir::Expr::StringConst(bang)],
            ))],
        );
        let main = b.main(
            Arena::new(),
            vec![
                call_stmt(extern_call(write, vec![mir::Expr::StringConst(hello)])),
                call_stmt(user_call(helper)),
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
                lir::GlobalInit::Storage { .. } => unreachable!("hello has no storage globals"),
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

        // The source declaration's typed intrinsic identity survives through
        // MIR and LIR. String metadata is a required singleton, not a layout
        // or descriptor that codegen has to rediscover by name.
        assert_eq!(
            module.meta.string.layout.kind,
            lir::LayoutKind::Intrinsic(lir::IntrinsicTypeRepresentation::String)
        );
        assert_eq!(
            module.meta.string.type_descriptor.symbol,
            lir::STRING_TD_SYMBOL
        );
        assert!(module.meta.string.type_descriptor.vtable.is_empty());
        assert!(
            module
                .meta
                .type_descriptors
                .iter()
                .all(|descriptor| descriptor.symbol != lir::STRING_TD_SYMBOL)
        );
        for representation in [
            lir::IntrinsicTypeRepresentation::Int,
            lir::IntrinsicTypeRepresentation::UInt,
            lir::IntrinsicTypeRepresentation::Boolean,
        ] {
            assert!(module.meta.layouts.iter().any(|layout| {
                layout.kind == lir::LayoutKind::Intrinsic(representation.clone())
            }));
        }

        // Golden dump locks the output structure.
        let expected = "\
Module
  global @scoop.str.0 = \"hello, world\"
  global @scoop.str.1 = \"!\"
  extern ef0 write @scoop_rt_write(ptr<managed>) -> {} <scoop managed nounwind>
  fun @scoop.helper() -> void
  block entry
    native_call[native-borrowed] extern0(global1)
    t0 = aggregate () : {}
    ret
  fun @scoop_main() -> void
  block entry
    native_call[native-borrowed] extern0(global0)
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
        let write = b.managed_scoop_extern(
            "write",
            "scoop_rt_write",
            vec![mir::Type::String],
            mir::Type::Unit,
        );
        let mut blocks = Arena::new();
        let entry = cfg_block(&mut blocks, "entry");
        let then_block = cfg_block(&mut blocks, "if.then.1");
        let else_block = cfg_block(&mut blocks, "if.else.2");
        let merge = cfg_block(&mut blocks, "if.merge.3");
        set_cfg_block(
            &mut blocks,
            entry,
            Vec::new(),
            mir::Terminator::Branch {
                cond: mir::Expr::BoolLiteral(true),
                then_block,
                else_block,
            },
            None,
        );
        set_cfg_block(
            &mut blocks,
            then_block,
            vec![call_stmt(extern_call(
                write,
                vec![mir::Expr::StringConst(ok)],
            ))],
            mir::Terminator::Goto(merge),
            None,
        );
        set_cfg_block(
            &mut blocks,
            else_block,
            vec![call_stmt(extern_call(
                write,
                vec![mir::Expr::StringConst(ng)],
            ))],
            mir::Terminator::Goto(merge),
            None,
        );
        set_cfg_block(
            &mut blocks,
            merge,
            Vec::new(),
            mir::Terminator::Return { value: None },
            None,
        );
        let main = b.user_fn_body(
            "main",
            mir::ENTRY_SYMBOL,
            Vec::new(),
            mir::Type::Unit,
            mir::Body {
                locals: Arena::new(),
                blocks,
                entry,
            },
        );
        let module = lower(&b.finish(main));

        let expected = "\
Module
  global @scoop.str.0 = \"ok\"
  global @scoop.str.1 = \"ng\"
  extern ef0 write @scoop_rt_write(ptr<managed>) -> {} <scoop managed nounwind>
  fun @scoop_main() -> void
  block entry
    cbr true then @if.then.1 else @if.else.2
  block if.then.1
    native_call[native-borrowed] extern0(global0)
    t0 = aggregate () : {}
    br @if.merge.3
  block if.else.2
    native_call[native-borrowed] extern0(global1)
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
        let mut blocks = Arena::new();
        let entry = cfg_block(&mut blocks, "entry");
        let cond = cfg_block(&mut blocks, "while.cond.1");
        let body = cfg_block(&mut blocks, "while.body.2");
        let exit = cfg_block(&mut blocks, "while.exit.3");
        set_cfg_block(
            &mut blocks,
            entry,
            vec![val_decl(n, mir::Expr::IntLiteral(0))],
            mir::Terminator::Goto(cond),
            None,
        );
        set_cfg_block(
            &mut blocks,
            cond,
            Vec::new(),
            mir::Terminator::Branch {
                cond: binary(
                    mir::BinOp::IntLt,
                    mir::Expr::Local(n),
                    mir::Expr::IntLiteral(3),
                ),
                then_block: body,
                else_block: exit,
            },
            None,
        );
        set_cfg_block(
            &mut blocks,
            body,
            vec![assign(
                n,
                binary(
                    mir::BinOp::IntAdd,
                    mir::Expr::Local(n),
                    mir::Expr::IntLiteral(1),
                ),
            )],
            mir::Terminator::Goto(cond),
            None,
        );
        set_cfg_block(
            &mut blocks,
            exit,
            Vec::new(),
            mir::Terminator::Return { value: None },
            None,
        );
        let main = b.user_fn_body(
            "main",
            mir::ENTRY_SYMBOL,
            Vec::new(),
            mir::Type::Unit,
            mir::Body {
                locals,
                blocks,
                entry,
            },
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
        let lhs = locals.alloc(local("$call.1", mir::Type::Boolean));
        let rhs = locals.alloc(local("$call.2", mir::Type::Boolean));
        let result = locals.alloc(local("b", mir::Type::Boolean));
        let mut blocks = Arena::new();
        let entry = cfg_block(&mut blocks, "entry");
        let rhs_block = cfg_block(&mut blocks, "logic.rhs.1");
        let short_block = cfg_block(&mut blocks, "logic.short.2");
        let merge = cfg_block(&mut blocks, "logic.merge.3");
        set_cfg_block(
            &mut blocks,
            entry,
            vec![call_value(lhs, string_eq(s0, s1))],
            mir::Terminator::Branch {
                cond: mir::Expr::Local(lhs),
                then_block: rhs_block,
                else_block: short_block,
            },
            None,
        );
        set_cfg_block(
            &mut blocks,
            rhs_block,
            vec![
                call_value(rhs, string_eq(s2, s3)),
                assign(result, mir::Expr::Local(rhs)),
            ],
            mir::Terminator::Goto(merge),
            None,
        );
        set_cfg_block(
            &mut blocks,
            short_block,
            vec![assign(result, mir::Expr::BoolLiteral(false))],
            mir::Terminator::Goto(merge),
            None,
        );
        set_cfg_block(
            &mut blocks,
            merge,
            Vec::new(),
            mir::Terminator::Return { value: None },
            None,
        );
        let main = b.user_fn_body(
            "main",
            mir::ENTRY_SYMBOL,
            Vec::new(),
            mir::Type::Unit,
            mir::Body {
                locals,
                blocks,
                entry,
            },
        );
        let module = lower(&b.finish(main));

        let expected = "\
Module
  global @scoop.str.0 = \"a\"
  global @scoop.str.1 = \"b\"
  global @scoop.str.2 = \"c\"
  global @scoop.str.3 = \"d\"
  fun @scoop_main() -> void
    local %0 $call.1: i1
    local %1 $call.2: i1
    local %2 b: i1
  block entry
    t0 = call @scoop_rt_string_eq(global0, global1) : i1
    store t0 -> local0
    cbr local0 then @logic.rhs.1 else @logic.short.2
  block logic.rhs.1
    t1 = call @scoop_rt_string_eq(global2, global3) : i1
    store t1 -> local1
    store local1 -> local2
    br @logic.merge.3
  block logic.short.2
    store false -> local2
    br @logic.merge.3
  block logic.merge.3
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
        let mut blocks = Arena::new();
        let entry = cfg_block(&mut blocks, "entry");
        let rhs = cfg_block(&mut blocks, "logic.rhs.1");
        let short = cfg_block(&mut blocks, "logic.short.2");
        let merge = cfg_block(&mut blocks, "logic.merge.3");
        set_cfg_block(
            &mut blocks,
            entry,
            vec![
                val_decl(x, mir::Expr::BoolLiteral(true)),
                val_decl(y, mir::Expr::BoolLiteral(false)),
            ],
            mir::Terminator::Branch {
                cond: mir::Expr::Local(x),
                then_block: short,
                else_block: rhs,
            },
            None,
        );
        set_cfg_block(
            &mut blocks,
            rhs,
            vec![assign(result, mir::Expr::Local(y))],
            mir::Terminator::Goto(merge),
            None,
        );
        set_cfg_block(
            &mut blocks,
            short,
            vec![assign(result, mir::Expr::BoolLiteral(true))],
            mir::Terminator::Goto(merge),
            None,
        );
        set_cfg_block(
            &mut blocks,
            merge,
            Vec::new(),
            mir::Terminator::Return { value: None },
            None,
        );
        let main = b.user_fn_body(
            "main",
            mir::ENTRY_SYMBOL,
            Vec::new(),
            mir::Type::Unit,
            mir::Body {
                locals,
                blocks,
                entry,
            },
        );
        let module = lower(&b.finish(main));

        let expected = "\
Module
  fun @scoop_main() -> void
    local %0 x: i1
    local %1 y: i1
    local %2 b: i1
  block entry
    store true -> local0
    store false -> local1
    cbr local0 then @logic.short.2 else @logic.rhs.1
  block logic.rhs.1
    store local1 -> local2
    br @logic.merge.3
  block logic.short.2
    store true -> local2
    br @logic.merge.3
  block logic.merge.3
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
                call_value(
                    s,
                    runtime_call(
                        mir::RuntimeFn::StringConcat,
                        vec![mir::Expr::StringConst(s0), mir::Expr::StringConst(s1)],
                    ),
                ),
                call_value(
                    e,
                    runtime_call(
                        mir::RuntimeFn::StringEq,
                        vec![mir::Expr::StringConst(s0), mir::Expr::StringConst(s1)],
                    ),
                ),
                call_value(
                    i,
                    runtime_call(mir::RuntimeFn::IntToString, vec![mir::Expr::IntLiteral(42)]),
                ),
                call_value(
                    o,
                    runtime_call(
                        mir::RuntimeFn::BoolToString,
                        vec![mir::Expr::BoolLiteral(true)],
                    ),
                ),
                call_stmt(user_call(helper)),
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
        assert_eq!(function.temps[*concat_out].ty, lir::MANAGED_PTR);
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
        assert_eq!(function.temps[*its_out].ty, lir::MANAGED_PTR);
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
        assert_eq!(function.temps[*bts_out].ty, lir::MANAGED_PTR);
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
    fn struct_values_keep_named_lir_identity() {
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
                lir::LirType::Struct(struct_def_id(point)),
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
            lir::LirType::Struct(struct_def_id(point))
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
        let _c = b.class("C", None, &[("u", mir::Type::UInt)], empty_vtable(), vec![]);
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

        // The String singleton is structurally separate; the remaining typed
        // intrinsic layouts stay in declaration order with ordinary layouts.
        let names: Vec<&str> = module
            .meta
            .layouts
            .iter()
            .map(|l| l.name.as_str())
            .collect();
        assert_eq!(
            module.meta.string.layout.kind,
            lir::LayoutKind::Intrinsic(lir::IntrinsicTypeRepresentation::String)
        );
        assert_eq!(
            names,
            [
                "S",
                "Outer",
                "Int",
                "UInt",
                "Boolean",
                "(String, Int)",
                "(Boolean, Int)"
            ]
        );

        let string = &module.meta.string.layout;
        assert_eq!((string.size, string.align), (24, 8));
        assert!(string.fields.is_empty());

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

    fn body_with_terminator(
        locals: Arena<mir::Local>,
        statements: Vec<mir::Statement>,
        terminator: mir::Terminator,
    ) -> mir::Body {
        let mut blocks = Arena::new();
        let entry = blocks.alloc(mir::BasicBlock {
            name: "entry".to_string(),
            statements,
            terminator,
            unwind: None,
        });
        mir::Body {
            locals,
            blocks,
            entry,
        }
    }

    fn returning_body(locals: Arena<mir::Local>, value: mir::Expr) -> mir::Body {
        body_with_terminator(
            locals,
            Vec::new(),
            mir::Terminator::Return { value: Some(value) },
        )
    }

    fn cfg_block(blocks: &mut Arena<mir::BasicBlock>, name: &str) -> mir::BlockId {
        blocks.alloc(mir::BasicBlock {
            name: name.to_string(),
            statements: Vec::new(),
            terminator: mir::Terminator::Unreachable,
            unwind: None,
        })
    }

    fn set_cfg_block(
        blocks: &mut Arena<mir::BasicBlock>,
        block: mir::BlockId,
        statements: Vec<mir::Statement>,
        terminator: mir::Terminator,
        unwind: Option<mir::BlockId>,
    ) {
        blocks[block].statements = statements;
        blocks[block].terminator = terminator;
        blocks[block].unwind = unwind;
    }

    fn cfg_block_named(body: &mir::Body, name: &str) -> mir::BlockId {
        body.blocks
            .iter()
            .find_map(|(id, block)| (block.name == name).then_some(id))
            .unwrap_or_else(|| panic!("missing MIR block `{name}`"))
    }

    fn single_catch_body(
        locals: Arena<mir::Local>,
        catch_local: mir::LocalId,
        catch_ty: mir::Type,
        body_statements: Vec<mir::Statement>,
        body_terminator: Option<mir::Terminator>,
        catch_statements: Vec<mir::Statement>,
    ) -> mir::Body {
        let mut blocks = Arena::new();
        let entry = cfg_block(&mut blocks, "entry");
        let unwind = cfg_block(&mut blocks, "try.unwind.1");
        let dispatch = cfg_block(&mut blocks, "try.dispatch.2");
        let handler_pad = cfg_block(&mut blocks, "try.handler_pad.3");
        let handler_cleanup = cfg_block(&mut blocks, "try.handler_cleanup.4");
        let exit_pad = cfg_block(&mut blocks, "try.exit_pad.5");
        let exit_cleanup = cfg_block(&mut blocks, "try.exit_cleanup.6");
        let end = cfg_block(&mut blocks, "try.end.7");
        let try_body = cfg_block(&mut blocks, "try.body.8");
        let catch = cfg_block(&mut blocks, "try.catch.9");
        let next = cfg_block(&mut blocks, "try.next.10");

        set_cfg_block(
            &mut blocks,
            entry,
            Vec::new(),
            mir::Terminator::Goto(try_body),
            None,
        );
        set_cfg_block(
            &mut blocks,
            unwind,
            vec![stmt(mir::StatementKind::Eh(mir::EhStatement::LandingPad {
                cleanup: false,
            }))],
            mir::Terminator::Goto(dispatch),
            None,
        );
        set_cfg_block(
            &mut blocks,
            dispatch,
            vec![stmt(mir::StatementKind::Eh(mir::EhStatement::BeginCatch))],
            mir::Terminator::Branch {
                cond: mir::Expr::IsInstance {
                    operand: Box::new(mir::Expr::CaughtException),
                    check_ty: Box::new(catch_ty.clone()),
                },
                then_block: catch,
                else_block: next,
            },
            None,
        );
        set_cfg_block(
            &mut blocks,
            handler_pad,
            vec![stmt(mir::StatementKind::Eh(mir::EhStatement::LandingPad {
                cleanup: true,
            }))],
            mir::Terminator::Goto(handler_cleanup),
            None,
        );
        set_cfg_block(
            &mut blocks,
            handler_cleanup,
            vec![stmt(mir::StatementKind::Eh(mir::EhStatement::EndCatch))],
            mir::Terminator::Resume,
            None,
        );
        set_cfg_block(
            &mut blocks,
            exit_pad,
            vec![stmt(mir::StatementKind::Eh(mir::EhStatement::LandingPad {
                cleanup: true,
            }))],
            mir::Terminator::Goto(exit_cleanup),
            None,
        );
        set_cfg_block(
            &mut blocks,
            exit_cleanup,
            vec![stmt(mir::StatementKind::Eh(mir::EhStatement::EndCatch))],
            mir::Terminator::Resume,
            None,
        );
        set_cfg_block(
            &mut blocks,
            end,
            Vec::new(),
            mir::Terminator::Return { value: None },
            None,
        );
        set_cfg_block(
            &mut blocks,
            try_body,
            body_statements,
            body_terminator.unwrap_or(mir::Terminator::Goto(end)),
            Some(unwind),
        );
        let mut catch_body = vec![val_decl(
            catch_local,
            mir::Expr::Retype {
                operand: Box::new(mir::Expr::CaughtException),
                ty: Box::new(catch_ty),
            },
        )];
        catch_body.extend(catch_statements);
        catch_body.push(stmt(mir::StatementKind::Eh(mir::EhStatement::EndCatch)));
        set_cfg_block(
            &mut blocks,
            catch,
            catch_body,
            mir::Terminator::Goto(end),
            Some(handler_pad),
        );
        set_cfg_block(
            &mut blocks,
            next,
            Vec::new(),
            mir::Terminator::Rethrow {
                unwind: Some(exit_pad),
            },
            None,
        );

        mir::Body {
            locals,
            blocks,
            entry,
        }
    }

    #[test]
    fn function_signatures_params_and_calls() {
        let mut b = Builder::new();
        // fun add(x: Int, y: Int): Int { return x + y }
        let mut locals = Arena::new();
        let x = locals.alloc(local("x", mir::Type::Int));
        let y = locals.alloc(local("y", mir::Type::Int));
        let add = b.user_fn_body(
            "add",
            "scoop.add",
            vec![param("x", mir::Type::Int, x), param("y", mir::Type::Int, y)],
            mir::Type::Int,
            returning_body(
                locals,
                binary(mir::BinOp::IntAdd, mir::Expr::Local(x), mir::Expr::Local(y)),
            ),
        );
        // main: val r = add(40, 2)
        let mut main_locals = Arena::new();
        let r = main_locals.alloc(local("r", mir::Type::Int));
        let main = b.main(
            main_locals,
            vec![call_value(
                r,
                mir::Call {
                    target: mir::CallTarget {
                        kind: mir::CallKind::Direct,
                        callee: mir::Callee::User(add),
                    },
                    args: vec![mir::Expr::IntLiteral(40), mir::Expr::IntLiteral(2)],
                },
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
        let result = locals.alloc(local("$call.1", mir::Type::String));
        b.user_fn_body(
            "tostring.I",
            "scoop.tostring.I",
            vec![param("this", mir::Type::Any, this)],
            mir::Type::String,
            body_with_terminator(
                locals,
                vec![call_value(
                    result,
                    runtime_call(
                        mir::RuntimeFn::IntToString,
                        vec![mir::Expr::Unbox(Box::new(mir::Expr::Local(this)))],
                    ),
                )],
                mir::Terminator::Return {
                    value: Some(mir::Expr::Local(result)),
                },
            ),
        );
        let main = b.main(Arena::new(), vec![]);
        let module = lower(&b.finish(main));

        let expected = "\
Module
  fun @scoop.tostring.I(ptr<managed>) -> ptr<managed>
    local %0 $call.1: ptr<managed>
  block entry
    t0 = heap_load param0 +16 : i64
    t1 = call @scoop_rt_int_to_string(t0) : ptr<managed>
    store t1 -> local0
    ret local0
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
        let mut blocks = Arena::new();
        let entry = cfg_block(&mut blocks, "entry");
        let then_block = cfg_block(&mut blocks, "if.then.1");
        let merge = cfg_block(&mut blocks, "if.merge.2");
        set_cfg_block(
            &mut blocks,
            entry,
            Vec::new(),
            mir::Terminator::Branch {
                cond: mir::Expr::BoolLiteral(true),
                then_block,
                else_block: merge,
            },
            None,
        );
        set_cfg_block(
            &mut blocks,
            then_block,
            Vec::new(),
            mir::Terminator::Return {
                value: Some(mir::Expr::Local(x)),
            },
            None,
        );
        set_cfg_block(
            &mut blocks,
            merge,
            Vec::new(),
            mir::Terminator::Return {
                value: Some(mir::Expr::IntLiteral(0)),
            },
            None,
        );
        let f = b.user_fn_body(
            "f",
            "scoop.f",
            vec![param("x", mir::Type::Int, x)],
            mir::Type::Int,
            mir::Body {
                locals,
                blocks,
                entry,
            },
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
    local %2 p: ptr<managed>
    local %3 o2: enum0
  block entry
    t0 = enum_wrap e0 v1 () : enum0
    store t0 -> local0
    t1 = enum_tag e0 local0 : i64
    store t1 -> local1
    t2 = enum_field e0 v0 f0 local0 : ptr<managed>
    store t2 -> local2
    t3 = enum_wrap e0 v0 (local2) : enum0
    store t3 -> local3
    ret
  layout String size=24 align=8 refs=[]
  layout Int size=8 align=8 refs=[]
  layout Boolean size=1 align=1 refs=[]
  layout Option$S size=8 align=8 enum-scan=refs[0]
  entry @scoop_main
";
        assert_eq!(lir::dump(&module), expected);
    }

    #[test]
    fn option_of_raw_pointer_uses_a_niche_without_gc_scanning() {
        let mut builder = Builder::new();
        let option = builder.option_enum("Option$P", mir::Type::Ptr(Box::new(mir::Type::Int)));
        let main = builder.main(Arena::new(), Vec::new());
        let module = lower(&builder.finish(main));

        assert!(matches!(
            edef(&module, option).repr,
            lir::EnumRepr::Niche { payload_variant: 0 }
        ));
        let layout = module
            .meta
            .layouts
            .iter()
            .find(|layout| layout.name == "Option$P")
            .expect("raw pointer option layout");
        let lir::LayoutKind::Enum { scan } = &layout.kind else {
            panic!("Option<Ptr<Int>> must retain its enum layout identity")
        };
        assert_eq!(*scan, lir::RefScan::None);
    }

    #[test]
    fn option_of_int_uses_the_tagged_representation() {
        // Option<Int>: the `{ i64 tag, [8 x i8] payload }` tagged
        // form — size 16, align 8.
        let module = lower(&option_round_trip("Option$I", mir::Type::Int));

        let expected = "\
Module
  enum Option$I tagged size=16 align=8 variants=(i64)@8+8 ()@8+0
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
  layout Option$I size=16 align=8 enum-scan=none
  entry @scoop_main
";
        assert_eq!(lir::dump(&module), expected);
    }

    #[test]
    fn niche_detection_requires_option_isomorphic_pointer_shape() {
        let mut b = Builder::new();
        let option_s = b.option_enum("Option$S", mir::Type::String);
        let array_int = b.array("Array<Int>", mir::Type::Int);
        let option_array = b.option_enum("Option$Array$I", array_int);
        let option_i = b.option_enum("Option$I", mir::Type::Int);
        // Reversed declaration order: the payload variant comes second.
        let flip = b.enums.alloc(mir::EnumDef {
            name: "Flip".to_string(),
            gc_free: false,
            variants: vec![
                mir::VariantDef {
                    name: "Naught".to_string(),
                    gc_free: true,
                    fields: Vec::new(),
                },
                mir::VariantDef {
                    name: "Value".to_string(),
                    gc_free: false,
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
            gc_free: true,
            variants: vec![
                mir::VariantDef {
                    name: "Pair".to_string(),
                    gc_free: true,
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
                    gc_free: true,
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
            edef(&module, option_array).repr,
            lir::EnumRepr::Niche { payload_variant: 0 }
        ));
        assert!(matches!(
            edef(&module, flip).repr,
            lir::EnumRepr::Niche { payload_variant: 1 }
        ));
        let lir::EnumRepr::Tagged {
            variants,
            size,
            align,
        } = &edef(&module, option_i).repr
        else {
            panic!("Option<Int> must use the tagged representation")
        };
        assert_eq!(variants[0].fields, [lir::LirType::I64]);
        assert_eq!(variants[0].slot_offset, variants[1].slot_offset);
        assert!(variants.iter().all(|variant| variant.gc_free));
        assert_eq!((*size, *align), (16, 8));
        let lir::EnumRepr::Tagged { variants, .. } = &edef(&module, pair_or_none).repr else {
            panic!("PairOrNone must be tagged")
        };
        assert_eq!(variants[0].slot_offset, variants[1].slot_offset);
        assert!(variants.iter().all(|variant| variant.gc_free));
    }

    #[test]
    fn c_layout_keeps_packing_alignment_offsets_and_identity() {
        let mut b = Builder::new();
        let inner = b.c_strukt(
            "Inner",
            8,
            1,
            false,
            &[("flag", mir::Type::Boolean), ("value", mir::Type::Int)],
        );
        let outer = b.c_strukt(
            "Outer",
            16,
            2,
            true,
            &[
                ("tag", mir::Type::Boolean),
                ("inner", mir::Type::Struct(inner)),
                ("tail", mir::Type::Int),
            ],
        );
        let wrapped = b.enums.alloc(mir::EnumDef {
            name: "Wrapped".to_string(),
            gc_free: true,
            variants: vec![
                mir::VariantDef {
                    name: "Value".to_string(),
                    gc_free: true,
                    fields: vec![mir::Field {
                        name: "value".to_string(),
                        ty: mir::Type::Struct(outer),
                    }],
                },
                mir::VariantDef {
                    name: "Empty".to_string(),
                    gc_free: true,
                    fields: Vec::new(),
                },
                mir::VariantDef {
                    name: "Number".to_string(),
                    gc_free: true,
                    fields: vec![mir::Field {
                        name: "value".to_string(),
                        ty: mir::Type::Int,
                    }],
                },
            ],
        });
        let outer_array = b.array("Array<Outer>", mir::Type::Struct(outer));
        let wrapped_array = b.array("Array<Wrapped>", mir::Type::Enum(wrapped, Vec::new()));
        let mut locals = Arena::new();
        locals.alloc(local("values", outer_array));
        locals.alloc(local("wrapped", wrapped_array));
        let main = b.main(locals, vec![]);
        let module = lower(&b.finish(main));

        let inner_def = &module.structs[struct_def_id(inner)];
        assert_eq!((inner_def.size, inner_def.align), (16, 8));
        assert_eq!(
            inner_def
                .fields
                .iter()
                .map(|field| field.layout)
                .collect::<Vec<_>>(),
            [
                lir::FieldLayout {
                    offset: 0,
                    access_align: 1,
                },
                lir::FieldLayout {
                    offset: 1,
                    access_align: 1,
                },
            ]
        );

        let outer_def = &module.structs[struct_def_id(outer)];
        assert_eq!(
            outer_def.fields[1].ty,
            lir::LirType::Struct(struct_def_id(inner))
        );
        assert_eq!((outer_def.size, outer_def.align), (32, 16));
        assert_eq!(
            outer_def
                .fields
                .iter()
                .map(|field| field.layout)
                .collect::<Vec<_>>(),
            [
                lir::FieldLayout {
                    offset: 0,
                    access_align: 1,
                },
                lir::FieldLayout {
                    offset: 2,
                    access_align: 2,
                },
                lir::FieldLayout {
                    offset: 18,
                    access_align: 2,
                },
            ]
        );
        assert!(outer_def.interior_mutable);

        let outer_layout = module
            .meta
            .layouts
            .iter()
            .find(|layout| layout.name == "Outer")
            .expect("Outer layout");
        assert_eq!((outer_layout.size, outer_layout.align), (32, 16));
        assert_eq!(
            outer_layout.fields,
            outer_def
                .fields
                .iter()
                .map(|field| field.layout)
                .collect::<Vec<_>>()
        );
        assert!(outer_layout.interior_mutable);
        let array_layout = array_metadata(&module, "Array<Outer>");
        assert_eq!(
            (array_layout.element_size, array_layout.element_align),
            (32, 16)
        );
        let wrapped_layout = module
            .meta
            .layouts
            .iter()
            .find(|layout| layout.name == "Wrapped")
            .expect("enum layout");
        assert_eq!((wrapped_layout.size, wrapped_layout.align), (48, 16));
        let wrapped_array = array_metadata(&module, "Array<Wrapped>");
        assert_eq!(
            (wrapped_array.element_size, wrapped_array.element_align),
            (48, 16)
        );
        assert!(lir::dump(&module).contains(
            "layout-meta Outer c-layout(aligned=16,packed=2) fields=[0@1,2@2,18@2] interior-mutable=true"
        ));
    }

    #[test]
    fn recursive_scans_preserve_tagged_enums_in_aggregates_and_arrays() {
        let mut b = Builder::new();
        // enum Msg { Text(String), Pair(Boolean, String), Empty }
        let msg = b.enums.alloc(mir::EnumDef {
            name: "Msg".to_string(),
            gc_free: false,
            variants: vec![
                mir::VariantDef {
                    name: "Text".to_string(),
                    gc_free: false,
                    fields: vec![mir::Field {
                        name: "value".to_string(),
                        ty: mir::Type::String,
                    }],
                },
                mir::VariantDef {
                    name: "Pair".to_string(),
                    gc_free: false,
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
                    gc_free: true,
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
        // Nested { flag: Boolean @0, msg: Msg @8 }. Msg's fixed ref
        // offsets compose without retaining or reading its tag.
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
            empty_vtable(),
            vec![],
        );
        let messages_array = b.array("Array<Msg>", msg_ty.clone());
        let nested_array = b.array("Array<Nested>", mir::Type::Struct(nested));
        let mut locals = Arena::new();
        locals.alloc(local("messages", messages_array));
        locals.alloc(local("nestedValues", nested_array));
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

        // Msg has two ref-bearing variants, so Text and Pair receive
        // disjoint slots. Empty is the zero-sized shared pure region.
        let msg_layout = by_name("Msg");
        assert_eq!((msg_layout.size, msg_layout.align), (32, 8));
        let lir::LayoutKind::Enum { scan } = &msg_layout.kind else {
            panic!("an enum layout keeps fixed scan offsets")
        };
        assert_eq!(*scan, lir::RefScan::References(vec![8, 24]));
        let lir::EnumRepr::Tagged { variants, .. } = &edef(&module, msg).repr else {
            panic!("Msg is tagged")
        };
        assert_ne!(variants[0].slot_offset, variants[1].slot_offset);
        assert_eq!(variants[2].slot_offset, 8);

        // The niche layout: the payload variant is the reference
        // itself; the unit variant has none.
        let option_layout = by_name("Option$S");
        assert_eq!((option_layout.size, option_layout.align), (8, 8));
        let lir::LayoutKind::Enum { scan } = &option_layout.kind else {
            panic!("an enum layout keeps fixed scan offsets")
        };
        assert_eq!(*scan, lir::RefScan::References(vec![0]));

        // S { o: Option<String> }: the niche value at offset 0 is the
        // struct's reference field.
        let s_layout = by_name("S");
        assert_eq!((s_layout.size, s_layout.align), (8, 8));
        assert_eq!(plain_refs(s_layout), [0]);

        let nested_layout = by_name("Nested");
        assert_eq!((nested_layout.size, nested_layout.align), (40, 8));
        assert_eq!(
            nested_layout.kind,
            lir::LayoutKind::Plain {
                scan: lir::RefScan::References(vec![16, 32]),
            }
        );

        let holder_td = module
            .meta
            .type_descriptors
            .iter()
            .find(|td| td.name == "Holder")
            .expect("Holder TypeDescriptor");
        assert_eq!((holder_td.size, holder_td.align), (64, 8));
        assert_eq!(holder_td.scan, lir::RefScan::References(vec![16, 40, 56]));

        let array_scan = |name: &str| array_metadata(&module, name).type_descriptor.scan.clone();
        assert_eq!(
            array_scan("Array<Msg>"),
            lir::RefScan::References(vec![8, 24])
        );
        assert_eq!(
            array_scan("Array<Nested>"),
            lir::RefScan::References(vec![16, 32])
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
        let cond = || {
            binary(
                mir::BinOp::IntEq,
                mir::Expr::EnumTag(Box::new(mir::Expr::Local(o))),
                mir::Expr::IntLiteral(0),
            )
        };
        let init = |result| {
            val_decl(
                result,
                mir::Expr::EnumField {
                    operand: Box::new(mir::Expr::Local(o)),
                    variant: 0,
                    index: 0,
                },
            )
        };
        let mut blocks = Arena::new();
        let entry = cfg_block(&mut blocks, "entry");
        let then1 = cfg_block(&mut blocks, "if.then.1");
        let else1 = cfg_block(&mut blocks, "if.else.2");
        let merge1 = cfg_block(&mut blocks, "if.merge.3");
        let then2 = cfg_block(&mut blocks, "if.then.5");
        let else2 = cfg_block(&mut blocks, "if.else.6");
        let merge2 = cfg_block(&mut blocks, "if.merge.7");
        set_cfg_block(
            &mut blocks,
            entry,
            Vec::new(),
            mir::Terminator::Branch {
                cond: cond(),
                then_block: then1,
                else_block: else1,
            },
            None,
        );
        set_cfg_block(
            &mut blocks,
            then1,
            vec![init(uw1)],
            mir::Terminator::Goto(merge1),
            None,
        );
        set_cfg_block(
            &mut blocks,
            else1,
            Vec::new(),
            mir::Terminator::Trap { message },
            None,
        );
        set_cfg_block(
            &mut blocks,
            merge1,
            Vec::new(),
            mir::Terminator::Branch {
                cond: cond(),
                then_block: then2,
                else_block: else2,
            },
            None,
        );
        set_cfg_block(
            &mut blocks,
            then2,
            vec![init(uw2)],
            mir::Terminator::Goto(merge2),
            None,
        );
        set_cfg_block(
            &mut blocks,
            else2,
            Vec::new(),
            mir::Terminator::Trap { message },
            None,
        );
        set_cfg_block(
            &mut blocks,
            merge2,
            Vec::new(),
            mir::Terminator::Return {
                value: Some(binary(
                    mir::BinOp::IntAdd,
                    mir::Expr::Local(uw1),
                    mir::Expr::Local(uw2),
                )),
            },
            None,
        );
        let f = b.user_fn_body(
            "f",
            "scoop.f",
            vec![param("o", option_ty, o)],
            mir::Type::Int,
            mir::Body {
                locals,
                blocks,
                entry,
            },
        );
        let _ = f;
        let main = b.main(Arena::new(), Vec::new());
        let module = lower(&b.finish(main));

        // Both `!!` share the one trap block of the function.
        let expected = "\
Module
  global @scoop.str.0 = \"unwrap on None (function f)\"
  global @scoop.cstr.0 = c\"unwrap on None (function f)\"
  enum Option$I tagged size=16 align=8 variants=(i64)@8+8 ()@8+0
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
    br @unwrap.trap.1
  block if.merge.3
    t3 = enum_tag e0 param0 : i64
    t4 = Eq t3, 0 : i1
    cbr t4 then @if.then.5 else @if.else.6
  block if.then.5
    t5 = enum_field e0 v0 f0 param0 : i64
    store t5 -> local1
    br @if.merge.7
  block if.else.6
    br @unwrap.trap.1
  block if.merge.7
    t6 = Add local0, local1 : i64
    ret t6
  block unwrap.trap.1
    call @scoop_rt_trap(global1)
    unreachable
  fun @scoop_main() -> void
  block entry
    ret
  layout String size=24 align=8 refs=[]
  layout Int size=8 align=8 refs=[]
  layout Boolean size=1 align=1 refs=[]
  layout Option$I size=16 align=8 enum-scan=none
  entry @scoop_main
";
        assert_eq!(lir::dump(&module), expected);
    }

    #[test]
    fn array_nodes_become_array_instructions() {
        // val a = [1, 2]; val x = a[0]; val n = a.size
        // val m = MutableArray(a); m[0] = 40
        let mut b = Builder::new();
        let array_int = b.array("Array<Int>", mir::Type::Int);
        let mutable_int = b.mutable_array("MutableArray<Int>", mir::Type::Int);
        let mir::Type::Class(array_class) = array_int else {
            unreachable!()
        };
        let mir::Type::Class(mutable_class) = mutable_int else {
            unreachable!()
        };
        let mut locals = Arena::new();
        let a = locals.alloc(local("a", mir::Type::Class(array_class)));
        let x = locals.alloc(local("x", mir::Type::Int));
        let n = locals.alloc(local("n", mir::Type::Int));
        let m = locals.alloc(local("m", mir::Type::Class(mutable_class)));
        let main = b.main(
            locals,
            vec![
                val_decl(
                    a,
                    mir::Expr::ArrayLiteral {
                        array_type: array_class,
                        elements: vec![mir::Expr::IntLiteral(1), mir::Expr::IntLiteral(2)],
                    },
                ),
                val_decl(
                    x,
                    mir::Expr::ArrayGet {
                        array_type: array_class,
                        array: Box::new(mir::Expr::Local(a)),
                        index: Box::new(mir::Expr::IntLiteral(0)),
                    },
                ),
                val_decl(
                    n,
                    mir::Expr::ArrayLen {
                        array_type: array_class,
                        operand: Box::new(mir::Expr::Local(a)),
                    },
                ),
                val_decl(
                    m,
                    mir::Expr::ArrayClone {
                        source_type: array_class,
                        target_type: mutable_class,
                        operand: Box::new(mir::Expr::Local(a)),
                    },
                ),
                stmt(mir::StatementKind::ArraySet {
                    array_type: mutable_class,
                    array: mir::Expr::Local(m),
                    index: mir::Expr::IntLiteral(0),
                    value: mir::Expr::IntLiteral(40),
                }),
            ],
        );
        let module = lower(&b.finish(main));

        // Both nominal applications have managed-pointer storage, while every
        // instruction references its complete typed metadata record.
        let expected = "\
Module
  fun @scoop_main() -> void
    local %0 a: ptr<managed>
    local %1 x: i64
    local %2 n: i64
    local %3 m: ptr<managed>
  block entry
    t0 = array_alloc array0 (1, 2) : ptr<managed>
    store t0 -> local0
    t1 = array_get array0 local0 0 : i64
    store t1 -> local1
    t2 = array_len array0 local0 : i64
    store t2 -> local2
    t3 = array_clone array1 local0 : ptr<managed>
    store t3 -> local3
    array_set array1 local3 0 40
    ret
  array-type array0 Array<Int> kind=immutable element=i64 size=8 align=8 scan=none td=@scoop_td_Array<Int>
  array-type array1 MutableArray<Int> kind=mutable element=i64 size=8 align=8 scan=none td=@scoop_td_MutableArray<Int>
  layout String size=24 align=8 refs=[]
  layout Int size=8 align=8 refs=[]
  layout Boolean size=1 align=1 refs=[]
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
        let array_int = b.array("Array<Int>", mir::Type::Int);
        let array_string = b.array("Array<String>", mir::Type::String);
        let array_option = b.array("Array<Option$S<String>>", option_string);
        let array_point = b.array("Array<Point>", mir::Type::Struct(point));
        let array_nested = b.array("Array<Array<Int>>", array_int.clone());
        let mut locals = Arena::new();
        let _ints = locals.alloc(local("ints", array_int.clone()));
        let _strings = locals.alloc(local("strings", array_string));
        let _options = locals.alloc(local("options", array_option));
        let _points = locals.alloc(local("points", array_point));
        let _nested = locals.alloc(local("nested", array_nested));
        let main = b.main(locals, vec![]);
        let module = lower(&b.finish(main));

        let array_layout = |name: &str| {
            let array = array_metadata(&module, name);
            (
                array.element_size,
                array.element_align,
                array.type_descriptor.scan.clone(),
            )
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
        let array_int = b.array("Array<Int>", mir::Type::Int);
        let _holder = b.strukt("Holder", &[("flag", mir::Type::Boolean), ("xs", array_int)]);
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
        // The complete intrinsic class application exists independently of
        // whether a function contains an array instruction.
        assert_eq!(
            array_metadata(&module, "Array<Int>").element,
            lir::LirType::I64
        );
    }

    // ---- M6: reference types ----

    fn empty_vtable() -> Vec<mir::TableSlot> {
        Vec::new()
    }

    #[test]
    fn virtual_calls_load_the_vtable_and_call_indirect() {
        let mut b = Builder::new();
        let c = b.class("C", None, &[], empty_vtable(), vec![]);
        // `C.m(this: C): Int { return 1 }`.
        let mut method_locals = Arena::new();
        let this = method_locals.alloc(local("this", mir::Type::Class(c)));
        let m = b.user_fn_body(
            "C.m",
            "scoop.C.m",
            vec![param("this", mir::Type::Class(c), this)],
            mir::Type::Int,
            returning_body(method_locals, mir::Expr::IntLiteral(1)),
        );
        b.classes[c].vtable.push(mir::TableSlot::Function(m));
        // main: `val p: C; val r = p.m()` (the first ordinary virtual slot).
        let mut locals = Arena::new();
        let p = locals.alloc(local("p", mir::Type::Class(c)));
        let r = locals.alloc(local("r", mir::Type::Int));
        let main = b.main(
            locals,
            vec![call_value(
                r,
                mir::Call {
                    target: mir::CallTarget {
                        kind: mir::CallKind::Virtual { slot: 0 },
                        callee: mir::Callee::User(m),
                    },
                    args: vec![mir::Expr::Local(p)],
                },
            )],
        );
        let module = lower(&b.finish(main));

        // The receiver's object header (index 0) holds the TD; its
        // vtable pointer is ScoopTypeDescriptor field 5; the callee is
        // vtable[0].
        let expected = "\
Module
  fun @scoop.C.m(ptr<managed>) -> i64
  block entry
    ret 1
  fun @scoop_main() -> void
    local %0 p: ptr<managed>
    local %1 r: i64
  block entry
    t0 = heap_load local0 +0 : ptr<metadata>
    t1 = heap_load t0 +40 : ptr<metadata>
    t2 = call_indirect t1[0](local0) : i64
    store t2 -> local1
    ret
  td C @scoop_td_C size=16 vtable=1 itables=0
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
            vec![call_value(
                r,
                mir::Call {
                    target: mir::CallTarget {
                        kind: mir::CallKind::Interface {
                            interface: iface,
                            slot: 1,
                        },
                        callee: mir::Callee::User(label),
                    },
                    args: vec![mir::Expr::Local(i)],
                },
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
    local %0 i: ptr<managed>
    local %1 r: i64
  block entry
    t0 = heap_load local0 +0 : ptr<metadata>
    t1 = call @scoop_rt_itable_lookup(t0, global0) : ptr<metadata>
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
        let mut base_vtable = empty_vtable();
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
        let mut derived_vtable = empty_vtable();
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
        assert_eq!(base_td.vtable, ["scoop.Base.m"]);
        assert_eq!(base_td.itables.len(), 1);
        assert_eq!(base_td.itables[0].interface_symbol, "scoop_td_I");
        assert_eq!(base_td.itables[0].slots, ["scoop.Base.m"]);

        assert_eq!(derived_td.symbol, "scoop_td_Derived");
        assert_eq!(derived_td.parent.as_deref(), Some("scoop_td_Base"));
        // header 16 + Int @16 + String @24 → size 32; the String is
        // the one reference.
        assert_eq!((derived_td.size, derived_td.align), (32, 8));
        assert_eq!(derived_td.scan, lir::RefScan::References(vec![24]));
        assert_eq!(derived_td.vtable, ["scoop.Derived.m", "scoop.Derived.m2"]);
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
            empty_vtable(),
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
            empty_vtable(),
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
            empty_vtable(),
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
    local %0 a: ptr<managed>
    local %1 v: struct0
    local %2 chk: i1
  block entry
    t0 = aggregate (1) : struct0
    t1 = call @scoop_rt_box(global0, t0, 8) : ptr<managed>
    store t1 -> local0
    t2 = heap_load local0 +16 : struct0
    store t2 -> local1
    t3 = call @scoop_rt_is_instance(local0, global0) : i1
    store t3 -> local2
    ret
  td box$S @scoop_td_box$S size=24 vtable=0 itables=0
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
        // The MIR shapes produced by M12's ordinary GC wrappers:
        // `_pin` / `_getGcHandle` return
        // the runtime's raw word into the handle struct, `unpin` /
        // `releaseGcHandle` unwrap field 0 for the reverse call, and
        // the hooks are a void call / an i64 result.
        let mut b = Builder::new();
        let pinned_ptr = b.strukt("PinnedPtr$S", &[("raw", mir::Type::UInt)]);
        let gc_handle = b.strukt("GcHandle$S", &[("raw", mir::Type::UInt)]);
        let mut locals = Arena::new();
        let v = locals.alloc(local("v", mir::Type::String));
        let raw_pin = locals.alloc(local("$call.1", mir::Type::UInt));
        let h = locals.alloc(local("h", mir::Type::Struct(pinned_ptr)));
        let gc1 = locals.alloc(local("$gc.1", mir::Type::String));
        let p = locals.alloc(local("p", mir::Type::String));
        let raw_handle = locals.alloc(local("$call.2", mir::Type::UInt));
        let gh = locals.alloc(local("gh", mir::Type::Struct(gc_handle)));
        let gc2 = locals.alloc(local("$gc.2", mir::Type::String));
        let p2 = locals.alloc(local("p2", mir::Type::String));
        let n = locals.alloc(local("n", mir::Type::UInt));
        let main = b.main(
            locals,
            vec![
                call_value(
                    raw_pin,
                    runtime_call(mir::RuntimeFn::Pin, vec![mir::Expr::Local(v)]),
                ),
                val_decl(
                    h,
                    mir::Expr::StructInit {
                        struct_id: pinned_ptr,
                        args: vec![mir::Expr::Local(raw_pin)],
                    },
                ),
                call_value(
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
                call_value(
                    raw_handle,
                    runtime_call(mir::RuntimeFn::GetHandle, vec![mir::Expr::Local(v)]),
                ),
                val_decl(
                    gh,
                    mir::Expr::StructInit {
                        struct_id: gc_handle,
                        args: vec![mir::Expr::Local(raw_handle)],
                    },
                ),
                call_value(
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
                call_stmt(runtime_call(mir::RuntimeFn::GcCollect, vec![])),
                call_value(n, runtime_call(mir::RuntimeFn::GcStats, vec![])),
            ],
        );
        let module = lower(&b.finish(main));

        let expected = "\
Module
  fun @scoop_main() -> void
    local %0 v: ptr<managed>
    local %1 $call.1: i64
    local %2 h: struct0
    local %3 $gc.1: ptr<managed>
    local %4 p: ptr<managed>
    local %5 $call.2: i64
    local %6 gh: struct1
    local %7 $gc.2: ptr<managed>
    local %8 p2: ptr<managed>
    local %9 n: i64
  block entry
    t0 = call @scoop_rt_pin(local0) : i64
    store t0 -> local1
    t1 = aggregate (local1) : struct0
    store t1 -> local2
    t2 = extract local2, 0 : i64
    t3 = call @scoop_rt_unpin(t2) : ptr<managed>
    store t3 -> local3
    store local3 -> local4
    t4 = call @scoop_rt_get_handle(local0) : i64
    store t4 -> local5
    t5 = aggregate (local5) : struct1
    store t5 -> local6
    t6 = extract local6, 0 : i64
    t7 = call @scoop_rt_release_handle(t6) : ptr<managed>
    store t7 -> local7
    store local7 -> local8
    call @scoop_rt_gc_collect()
    t8 = aggregate () : {}
    t9 = call @scoop_rt_gc_stats() : i64
    store t9 -> local9
    ret
  layout String size=24 align=8 refs=[]
  layout Int size=8 align=8 refs=[]
  layout Boolean size=1 align=1 refs=[]
  layout PinnedPtr$S size=8 align=8 refs=[]
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
            empty_vtable(),
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
        assert_eq!(function.temps[*out].ty, lir::MANAGED_PTR);
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
            empty_vtable(),
            vec![],
        );
        let mut ctor_locals = Arena::new();
        let x = ctor_locals.alloc(local("x", mir::Type::Int));
        let s = ctor_locals.alloc(local("s", mir::Type::String));
        let ctor = b.user_fn_body(
            "ctor.Point",
            "scoop.ctor.Point",
            vec![
                param("x", mir::Type::Int, x),
                param("s", mir::Type::String, s),
            ],
            mir::Type::Class(point),
            returning_body(
                ctor_locals,
                mir::Expr::ClassInit {
                    class_id: point,
                    args: vec![mir::Expr::Local(x), mir::Expr::Local(s)],
                },
            ),
        );
        let mut locals = Arena::new();
        let p = locals.alloc(local("p", mir::Type::Class(point)));
        let main = b.main(
            locals,
            vec![call_value(
                p,
                mir::Call {
                    target: mir::CallTarget {
                        kind: mir::CallKind::Direct,
                        callee: mir::Callee::User(ctor),
                    },
                    args: vec![mir::Expr::IntLiteral(1), mir::Expr::StringConst(str_x)],
                },
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
  fun @scoop.ctor.Point(i64, ptr<managed>) -> ptr<managed>
  block entry
    t0 = call @scoop_rt_alloc(global1, 32) : ptr<managed>
    heap_store t0 +16 param0
    heap_store t0 +24 param1
    ret t0
  fun @scoop_main() -> void
    local %0 p: ptr<managed>
  block entry
    t0 = call @scoop.ctor.Point(1, global0) : ptr<managed>
    store t0 -> local0
    ret
  td Point @scoop_td_Point size=32 vtable=0 itables=0
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
            empty_vtable(),
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
            vec![call_stmt(runtime_call(
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
        let main = b.user_fn_body(
            "main",
            mir::ENTRY_SYMBOL,
            Vec::new(),
            mir::Type::Unit,
            single_catch_body(
                locals,
                e,
                mir::Type::Class(my_error),
                vec![call_stmt(user_call(helper))],
                None,
                vec![call_stmt(user_call(handled))],
            ),
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
    local %0 e: ptr<managed>
    local %1 $sc.1: exception_record
    local %2 $sc.2: ptr<raw>
    local %3 $sc.3: ptr<managed>
  block entry
    br @try.body.8
  block try.unwind.1
    (t0, t1) = landingpad : (exception_record, ptr<raw>)
    store t0 -> local1
    store t1 -> local2
    br @try.dispatch.2
  block try.dispatch.2
    t2 = begin_catch local2 : ptr<managed>
    store t2 -> local3
    t3 = call @scoop_rt_is_instance(local3, global0) : i1
    cbr t3 then @try.catch.9 else @try.next.10
  block try.handler_pad.3
    (t4, t5) = cleanup_pad : (exception_record, ptr<raw>)
    store t4 -> local1
    store t5 -> local2
    br @try.handler_cleanup.4
  block try.handler_cleanup.4
    end_catch
    resume local1
  block try.exit_pad.5
    (t6, t7) = cleanup_pad : (exception_record, ptr<raw>)
    store t6 -> local1
    store t7 -> local2
    br @try.exit_cleanup.6
  block try.exit_cleanup.6
    end_catch
    resume local1
  block try.end.7
    ret
  block try.body.8
    invoke @scoop.helper() normal @invoke.normal.1 unwind @try.unwind.1
    br @invoke.normal.1
  block try.catch.9
    store local3 -> local0
    invoke @scoop.handled() normal @invoke.normal.2 unwind @try.handler_pad.3
    br @invoke.normal.2
  block try.next.10
    invoke @scoop_rt_rethrow() normal @rethrow.normal.3 unwind @try.exit_pad.5
    br @rethrow.normal.3
  block invoke.normal.1
    t8 = aggregate () : {}
    br @try.end.7
  block invoke.normal.2
    t9 = aggregate () : {}
    end_catch
    br @try.end.7
  block rethrow.normal.3
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
        let mut body = single_catch_body(
            locals,
            e,
            mir::Type::Class(my_error),
            vec![call_stmt(user_call(helper))],
            None,
            vec![call_stmt(user_call(handled))],
        );
        let try_body = cfg_block_named(&body, "try.body.8");
        let catch = cfg_block_named(&body, "try.catch.9");
        let next = cfg_block_named(&body, "try.next.10");
        let handler_cleanup = cfg_block_named(&body, "try.handler_cleanup.4");
        let exit_pad = cfg_block_named(&body, "try.exit_pad.5");
        let end = cfg_block_named(&body, "try.end.7");
        let normal_finally = cfg_block(&mut body.blocks, "scope.normal_finally");
        let catch_finally = cfg_block(&mut body.blocks, "scope.catch_finally");
        body.blocks[try_body].terminator = mir::Terminator::Goto(normal_finally);
        set_cfg_block(
            &mut body.blocks,
            normal_finally,
            vec![call_stmt(user_call(cleanup))],
            mir::Terminator::Goto(end),
            None,
        );
        assert!(matches!(
            body.blocks[catch]
                .statements
                .pop()
                .map(|statement| statement.kind),
            Some(mir::StatementKind::Eh(mir::EhStatement::EndCatch))
        ));
        body.blocks[catch].terminator = mir::Terminator::Goto(catch_finally);
        set_cfg_block(
            &mut body.blocks,
            catch_finally,
            vec![
                stmt(mir::StatementKind::Eh(mir::EhStatement::EndCatch)),
                call_stmt(user_call(cleanup)),
            ],
            mir::Terminator::Goto(end),
            None,
        );
        body.blocks[next].statements = vec![call_stmt(user_call(cleanup))];
        body.blocks[next].unwind = Some(exit_pad);
        body.blocks[handler_cleanup]
            .statements
            .push(call_stmt(user_call(cleanup)));
        let main = b.user_fn_body("main", mir::ENTRY_SYMBOL, Vec::new(), mir::Type::Unit, body);
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
        let mut locals = Arena::new();
        let result = locals.alloc(local("$return.1", mir::Type::Int));
        let mut blocks = Arena::new();
        let entry = cfg_block(&mut blocks, "entry");
        let unwind = cfg_block(&mut blocks, "try.unwind.1");
        let dispatch = cfg_block(&mut blocks, "try.dispatch.2");
        let exit_pad = cfg_block(&mut blocks, "try.exit_pad.3");
        let exit_cleanup = cfg_block(&mut blocks, "try.exit_cleanup.4");
        let end = cfg_block(&mut blocks, "try.end.5");
        let try_body = cfg_block(&mut blocks, "try.body.6");
        let return_finally = cfg_block(&mut blocks, "scope.7");
        let rethrow_finally = cfg_block(&mut blocks, "scope.8");
        set_cfg_block(
            &mut blocks,
            entry,
            Vec::new(),
            mir::Terminator::Goto(try_body),
            None,
        );
        set_cfg_block(
            &mut blocks,
            unwind,
            vec![stmt(mir::StatementKind::Eh(mir::EhStatement::LandingPad {
                cleanup: false,
            }))],
            mir::Terminator::Goto(dispatch),
            None,
        );
        set_cfg_block(
            &mut blocks,
            dispatch,
            vec![stmt(mir::StatementKind::Eh(mir::EhStatement::BeginCatch))],
            mir::Terminator::Goto(rethrow_finally),
            None,
        );
        set_cfg_block(
            &mut blocks,
            exit_pad,
            vec![stmt(mir::StatementKind::Eh(mir::EhStatement::LandingPad {
                cleanup: true,
            }))],
            mir::Terminator::Goto(exit_cleanup),
            None,
        );
        set_cfg_block(
            &mut blocks,
            exit_cleanup,
            vec![stmt(mir::StatementKind::Eh(mir::EhStatement::EndCatch))],
            mir::Terminator::Resume,
            None,
        );
        set_cfg_block(
            &mut blocks,
            end,
            Vec::new(),
            mir::Terminator::Unreachable,
            None,
        );
        set_cfg_block(
            &mut blocks,
            try_body,
            vec![val_decl(result, mir::Expr::IntLiteral(1))],
            mir::Terminator::Goto(return_finally),
            Some(unwind),
        );
        set_cfg_block(
            &mut blocks,
            return_finally,
            vec![call_stmt(user_call(cleanup))],
            mir::Terminator::Return {
                value: Some(mir::Expr::Local(result)),
            },
            None,
        );
        set_cfg_block(
            &mut blocks,
            rethrow_finally,
            vec![call_stmt(user_call(cleanup))],
            mir::Terminator::Rethrow {
                unwind: Some(exit_pad),
            },
            Some(exit_pad),
        );
        let f = b.user_fn_body(
            "f",
            "scoop.f",
            Vec::new(),
            mir::Type::Int,
            mir::Body {
                locals,
                blocks,
                entry,
            },
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
    local %0 $return.1: i64
    local %1 $sc.1: exception_record
    local %2 $sc.2: ptr<raw>
    local %3 $sc.3: ptr<managed>
  block entry
    br @try.body.6
  block try.unwind.1
    (t0, t1) = landingpad : (exception_record, ptr<raw>)
    store t0 -> local1
    store t1 -> local2
    br @try.dispatch.2
  block try.dispatch.2
    t2 = begin_catch local2 : ptr<managed>
    store t2 -> local3
    br @scope.8
  block try.exit_pad.3
    (t3, t4) = cleanup_pad : (exception_record, ptr<raw>)
    store t3 -> local1
    store t4 -> local2
    br @try.exit_cleanup.4
  block try.exit_cleanup.4
    end_catch
    resume local1
  block try.end.5
    unreachable
  block try.body.6
    store 1 -> local0
    br @scope.7
  block scope.7
    call @scoop.cleanup()
    t5 = aggregate () : {}
    ret local0
  block scope.8
    invoke @scoop.cleanup() normal @invoke.normal.1 unwind @try.exit_pad.3
    br @invoke.normal.1
  block invoke.normal.1
    t6 = aggregate () : {}
    invoke @scoop_rt_rethrow() normal @rethrow.normal.2 unwind @try.exit_pad.3
    br @rethrow.normal.2
  block rethrow.normal.2
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
        let make = b.user_fn_body(
            "makeError",
            "scoop.makeError",
            Vec::new(),
            mir::Type::Class(my_error),
            returning_body(
                std::mem::take(&mut ctor_locals),
                mir::Expr::ClassInit {
                    class_id: my_error,
                    args: Vec::new(),
                },
            ),
        );
        let mut main_locals = Arena::new();
        let exception = main_locals.alloc(local("$call.1", mir::Type::Class(my_error)));
        let main = b.user_fn_body(
            "main",
            mir::ENTRY_SYMBOL,
            Vec::new(),
            mir::Type::Unit,
            body_with_terminator(
                main_locals,
                vec![call_value(
                    exception,
                    mir::Call {
                        target: mir::CallTarget {
                            kind: mir::CallKind::Direct,
                            callee: mir::Callee::User(make),
                        },
                        args: Vec::new(),
                    },
                )],
                mir::Terminator::Throw {
                    exception: mir::Expr::Local(exception),
                    unwind: None,
                },
            ),
        );
        let module = lower(&b.finish(main));

        // Outside a try the throw is the `Throw` instruction ending
        // the block; the callee stays a plain call.
        let expected = "\
Module
  global @scoop_td_MyError = c\"\"
  fun @scoop.makeError() -> ptr<managed>
  block entry
    t0 = call @scoop_rt_alloc(global0, 16) : ptr<managed>
    ret t0
  fun @scoop_main() -> void
    local %0 $call.1: ptr<managed>
  block entry
    t0 = call @scoop.makeError() : ptr<managed>
    store t0 -> local0
    throw local0
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
        let body = single_catch_body(
            locals,
            e,
            mir::Type::Class(my_error),
            Vec::new(),
            None,
            Vec::new(),
        );
        let try_body = body
            .blocks
            .iter()
            .find_map(|(id, block)| (block.name == "try.body.8").then_some(id))
            .expect("try body block");
        let unwind = body.blocks[try_body].unwind;
        let mut body = body;
        body.blocks[try_body].terminator = mir::Terminator::Throw {
            exception: mir::Expr::Local(e),
            unwind,
        };
        let main = b.user_fn_body("main", mir::ENTRY_SYMBOL, Vec::new(), mir::Type::Unit, body);
        let module = lower(&b.finish(main));

        let function = module
            .functions
            .iter()
            .find(|f| f.symbol == mir::ENTRY_SYMBOL)
            .expect("the entry function");
        // The explicit MIR entry jumps into the try body. That block
        // ends with the invoke; its unwind target starts with the
        // landingpad.
        let entry = function
            .blocks
            .iter()
            .map(|(_, block)| block)
            .find(|block| block.name == "try.body.8")
            .expect("the try body");
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
        let mut body = single_catch_body(
            locals,
            e2_local,
            mir::Type::Class(e2),
            Vec::new(),
            None,
            vec![call_stmt(user_call(c))],
        );
        let outer_body = cfg_block_named(&body, "try.body.8");
        let outer_unwind = cfg_block_named(&body, "try.unwind.1");
        let outer_dispatch = cfg_block_named(&body, "try.dispatch.2");
        let outer_end = cfg_block_named(&body, "try.end.7");
        let inner_unwind = cfg_block(&mut body.blocks, "try.unwind.inner");
        let inner_dispatch = cfg_block(&mut body.blocks, "try.dispatch.inner");
        let inner_handler_pad = cfg_block(&mut body.blocks, "try.handler_pad.inner");
        let inner_handler_cleanup = cfg_block(&mut body.blocks, "try.handler_cleanup.inner");
        let inner_exit_pad = cfg_block(&mut body.blocks, "try.exit_pad.inner");
        let inner_exit_cleanup = cfg_block(&mut body.blocks, "try.exit_cleanup.inner");
        let inner_end = cfg_block(&mut body.blocks, "try.end.inner");
        let inner_body = cfg_block(&mut body.blocks, "try.body.inner");
        let inner_catch = cfg_block(&mut body.blocks, "try.catch.inner");
        let inner_next = cfg_block(&mut body.blocks, "try.next.inner");
        body.blocks[outer_body].terminator = mir::Terminator::Goto(inner_body);
        set_cfg_block(
            &mut body.blocks,
            inner_unwind,
            vec![stmt(mir::StatementKind::Eh(mir::EhStatement::LandingPad {
                cleanup: false,
            }))],
            mir::Terminator::Goto(inner_dispatch),
            None,
        );
        set_cfg_block(
            &mut body.blocks,
            inner_dispatch,
            vec![stmt(mir::StatementKind::Eh(mir::EhStatement::BeginCatch))],
            mir::Terminator::Branch {
                cond: mir::Expr::IsInstance {
                    operand: Box::new(mir::Expr::CaughtException),
                    check_ty: Box::new(mir::Type::Class(e1)),
                },
                then_block: inner_catch,
                else_block: inner_next,
            },
            None,
        );
        set_cfg_block(
            &mut body.blocks,
            inner_handler_pad,
            vec![stmt(mir::StatementKind::Eh(mir::EhStatement::LandingPad {
                cleanup: false,
            }))],
            mir::Terminator::Goto(inner_handler_cleanup),
            None,
        );
        set_cfg_block(
            &mut body.blocks,
            inner_handler_cleanup,
            vec![stmt(mir::StatementKind::Eh(mir::EhStatement::EndCatch))],
            mir::Terminator::Goto(outer_dispatch),
            Some(outer_unwind),
        );
        set_cfg_block(
            &mut body.blocks,
            inner_exit_pad,
            vec![stmt(mir::StatementKind::Eh(mir::EhStatement::LandingPad {
                cleanup: false,
            }))],
            mir::Terminator::Goto(inner_exit_cleanup),
            None,
        );
        set_cfg_block(
            &mut body.blocks,
            inner_exit_cleanup,
            vec![stmt(mir::StatementKind::Eh(mir::EhStatement::EndCatch))],
            mir::Terminator::Goto(outer_dispatch),
            Some(outer_unwind),
        );
        set_cfg_block(
            &mut body.blocks,
            inner_end,
            Vec::new(),
            mir::Terminator::Goto(outer_end),
            Some(outer_unwind),
        );
        set_cfg_block(
            &mut body.blocks,
            inner_body,
            vec![call_stmt(user_call(a))],
            mir::Terminator::Goto(inner_end),
            Some(inner_unwind),
        );
        set_cfg_block(
            &mut body.blocks,
            inner_catch,
            vec![
                val_decl(
                    e1_local,
                    mir::Expr::Retype {
                        operand: Box::new(mir::Expr::CaughtException),
                        ty: Box::new(mir::Type::Class(e1)),
                    },
                ),
                call_stmt(user_call(bb)),
                stmt(mir::StatementKind::Eh(mir::EhStatement::EndCatch)),
            ],
            mir::Terminator::Goto(inner_end),
            Some(inner_handler_pad),
        );
        set_cfg_block(
            &mut body.blocks,
            inner_next,
            Vec::new(),
            mir::Terminator::Rethrow {
                unwind: Some(inner_exit_pad),
            },
            None,
        );
        let main = b.user_fn_body("main", mir::ENTRY_SYMBOL, Vec::new(), mir::Type::Unit, body);
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
