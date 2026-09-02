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
//! - A TypeDescriptor operand is passed as `Value::TypeDescriptor`
//!   carrying a typed `TypeDescriptorRef`. Descriptor identity is
//!   therefore independent of its final linker symbol and cannot be
//!   confused with an ordinary global. Codegen resolves that typed
//!   reference directly from `LirMeta::type_descriptors`.
//!   For `scoop_rt_box` codegen materializes the by-value aggregate
//!   payload behind a stack pointer (the "临时 alloca 取地址" of the
//!   lowering contract).
//!
//! A `mir::ExprKind::ClassInit` (only ever produced inside mir-lower's
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

use std::collections::HashMap;

use la_arena::Arena;
use scoop_lir as lir;

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
            scan: lir::RefScan::None,
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
    let (extern_functions, extern_function_refs) = lower_extern_functions(module);
    let (storage_globals, native_globals, native_global_bridges) =
        lower_globals(module, &mut globals, &structs, &enums);
    let callback_bridges = lower_callback_bridges(module);
    let foreign_callback_bridges = lower_foreign_callback_bridges(module);
    let local_function_map = module
        .top_level
        .iter()
        .enumerate()
        .map(|(index, id)| {
            (
                *id,
                lir::LocalFunctionId::from_u32(
                    u32::try_from(index).expect("the LIR function list fits its typed id"),
                ),
            )
        })
        .collect::<HashMap<_, _>>();
    let (type_descriptors, type_descriptor_refs, well_known_type_descriptors) =
        type_descriptors(module, &enums, &local_function_map);
    let (arrays, array_type_map) = array_types(module, &enums, &type_descriptor_refs);

    // Tuple types encountered while mapping value types, in
    // first-appearance order; each one gets a meta layout.
    let mut layout_types = Vec::new();
    // Trap message globals (`scoop.cstr.N`), numbered in creation order.
    let mut cstr_count = 0usize;
    let mut safepoint_ids = safepoints::SafepointIds::default();
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
                &structs,
                &enums,
                &array_type_map,
                &type_descriptor_refs,
                &local_function_map,
                &extern_function_refs,
                &mut safepoint_ids,
            )
        })
        .collect();
    for function in &mut functions {
        safepoints::complete_function(function, &structs, &enums, &mut safepoint_ids);
    }

    let (layouts, well_known_layouts) = layouts(module, &enums, &layout_types);
    lir::Module {
        globals,
        structs,
        enums,
        functions,
        extern_functions,
        native_globals,
        native_global_bridges,
        callback_bridges,
        foreign_callback_bridges,
        entry_symbol: module.functions[module.entry].symbol.clone(),
        meta: lir::LirMeta {
            well_known_layouts,
            well_known_type_descriptors,
            arrays,
            layouts,
            type_descriptors,
            external_type_descriptors: Arena::new(),
            external_callables: Arena::new(),
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
    structs: &Arena<lir::StructDef>,
    enums: &Arena<lir::EnumDef>,
) -> (
    HashMap<mir::GlobalId, StorageGlobal>,
    Arena<lir::NativeGlobal>,
    lir::NativeGlobalBridges,
) {
    let mut map = HashMap::new();
    let mut native = Arena::new();
    let mut bridges = lir::NativeGlobalBridges::default();
    for (id, global) in module.globals.iter() {
        let storage = match &global.storage {
            mir::GlobalStorage::Local {
                thread_local,
                initializer,
            } => {
                let lir_id = globals.alloc(lir::Global {
                    symbol: global.symbol.clone(),
                    address_kind: lir::PointerKind::Raw,
                    scan: safepoints::root_scan(&lir_type(&global.ty), structs, enums, 0),
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
                let get = bridges.gets.alloc(lir::NativeGlobalGetBridge {
                    symbol: format!("scoop_c_global_get_{raw}"),
                });
                let address = bridges.addresses.alloc(lir::NativeGlobalAddressBridge {
                    symbol: format!("scoop_c_global_address_{raw}"),
                });
                let access = if global.mutable {
                    let set = bridges.sets.alloc(lir::NativeGlobalSetBridge {
                        symbol: format!("scoop_c_global_set_{raw}"),
                    });
                    lir::NativeGlobalAccess::Mutable { get, set, address }
                } else {
                    lir::NativeGlobalAccess::ReadOnly { get, address }
                };
                let lir_id = native.alloc(lir::NativeGlobal {
                    source_name: global.name.clone(),
                    native_symbol: native_symbol.clone(),
                    library: library.clone(),
                    ty: lir_type(&global.ty),
                    c_type: c_ffi_type(module, &global.ty),
                    thread_local: *thread_local,
                    access,
                });
                StorageGlobal::Native(lir_id)
            }
        };
        map.insert(id, storage);
    }
    (map, native, bridges)
}

fn lower_constant(value: &mir::ConstantValue) -> lir::ConstantValue {
    match value {
        mir::ConstantValue::Int(value) => lir::ConstantValue::Int(*value),
        mir::ConstantValue::Bool(value) => lir::ConstantValue::Bool(*value),
        mir::ConstantValue::NullPtr => lir::ConstantValue::NullPointer(lir::PointerKind::Raw),
        mir::ConstantValue::NullFunPtr => lir::ConstantValue::NullPointer(lir::PointerKind::Code),
        mir::ConstantValue::Struct { struct_id, fields } => lir::ConstantValue::Struct {
            struct_id: struct_def_id(*struct_id),
            fields: fields.iter().map(lower_constant).collect(),
        },
    }
}

#[derive(Clone, Copy)]
enum LoweredExternFunctionRef {
    C(lir::CExternFunctionRef),
    Scoop(lir::ScoopExternFunctionRef),
}

fn lower_extern_functions(
    module: &mir::Module,
) -> (
    lir::ExternFunctions,
    HashMap<mir::ExternFunctionId, LoweredExternFunctionRef>,
) {
    let mut functions = lir::ExternFunctions::default();
    let mut references = HashMap::new();
    for (id, extern_) in module.extern_functions.iter() {
        let declaration = lir::ExternFunctionDeclaration {
            source_name: extern_.source_name.clone(),
            native_symbol: extern_.native_symbol.clone(),
            library: extern_.library.clone(),
            calling_convention: match extern_.calling_convention {
                mir::CallingConvention::Cdecl => lir::CallingConvention::Cdecl,
            },
            params: extern_.params.iter().map(lir_type).collect(),
            return_type: lir_type(&extern_.return_type),
        };
        let reference = match extern_.abi {
            mir::ExternAbi::C => LoweredExternFunctionRef::C(
                functions.alloc_c(lir::CExternFunction {
                    declaration,
                    bridge_symbol: format!("scoop_c_bridge_{}", id.into_raw().into_u32()),
                    params: extern_
                        .params
                        .iter()
                        .map(|ty| c_ffi_type(module, ty))
                        .collect(),
                    return_type: c_ffi_type(module, &extern_.return_type),
                }),
            ),
            mir::ExternAbi::Scoop => {
                LoweredExternFunctionRef::Scoop(functions.alloc_scoop(lir::ScoopExternFunction {
                    declaration,
                    gc_effect: match extern_.gc_effect {
                        mir::GcEffect::Managed => lir::GcEffect::Managed,
                        mir::GcEffect::NoGc => lir::GcEffect::NoGc,
                    },
                }))
            }
        };
        references.insert(id, reference);
    }
    (functions, references)
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

mod locals;
mod metadata;
mod safepoints;

use metadata::*;

fn lower_runtime_function(function: mir::RuntimeFn) -> lir::RuntimeFunction {
    match function {
        mir::RuntimeFn::Box => lir::RuntimeFunction::Managed(lir::ManagedRuntimeFunction::Box),
        mir::RuntimeFn::IsInstance => {
            lir::RuntimeFunction::NoGc(lir::NoGcRuntimeFunction::IsInstance)
        }
        mir::RuntimeFn::ITableLookup => {
            lir::RuntimeFunction::NoGc(lir::NoGcRuntimeFunction::ITableLookup)
        }
        mir::RuntimeFn::Pin => lir::RuntimeFunction::NoGc(lir::NoGcRuntimeFunction::Pin),
        mir::RuntimeFn::Unpin => lir::RuntimeFunction::NoGc(lir::NoGcRuntimeFunction::Unpin),
        mir::RuntimeFn::GetHandle => {
            lir::RuntimeFunction::NoGc(lir::NoGcRuntimeFunction::GetHandle)
        }
        mir::RuntimeFn::ReleaseHandle => {
            lir::RuntimeFunction::NoGc(lir::NoGcRuntimeFunction::ReleaseHandle)
        }
        mir::RuntimeFn::GcCollect => {
            lir::RuntimeFunction::Managed(lir::ManagedRuntimeFunction::GcCollect)
        }
        mir::RuntimeFn::GcStats => lir::RuntimeFunction::NoGc(lir::NoGcRuntimeFunction::GcStats),
        mir::RuntimeFn::MaterializeException => {
            lir::RuntimeFunction::Managed(lir::ManagedRuntimeFunction::MaterializeException)
        }
        mir::RuntimeFn::StringConcat => {
            lir::RuntimeFunction::Managed(lir::ManagedRuntimeFunction::StringConcat)
        }
        mir::RuntimeFn::Trap => lir::RuntimeFunction::NoGc(lir::NoGcRuntimeFunction::Trap),
    }
}

mod function;
use function::lower_function;
#[cfg(test)]
mod tests;
