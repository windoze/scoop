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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CallProtocol {
    Managed,
    NoGc,
}

#[derive(Clone, Copy)]
enum NativeCallDestination {
    Safe(lir::NativeSafeCallDestination),
    Borrowed(lir::NativeBorrowedCallDestination),
}

/// A fully typed call shape before its protocol-specific destination is bound.
/// This is lowering-local scratch state and can never enter LIR output.
enum PendingTypedCall {
    Void {
        signature: lir::VoidCallSignatureId,
        args: Vec<lir::Value>,
    },
    Direct {
        signature: lir::DirectCallSignatureId,
        out: lir::TempId,
        args: Vec<lir::Value>,
    },
    IndirectResult {
        signature: lir::IndirectResultCallSignatureId,
        storage: lir::LocalId,
        args: Vec<lir::Value>,
    },
}

impl PendingTypedCall {
    fn result_scan<'a>(&self, targets: &'a lir::CallTargets) -> &'a lir::RefScan {
        match self {
            Self::Void { .. } => &lir::RefScan::None,
            Self::Direct { signature, .. } => &targets.direct_signatures[*signature].result_scan,
            Self::IndirectResult { signature, .. } => {
                &targets.indirect_result_signatures[*signature].result.scan
            }
        }
    }
}

fn bind_typed_call<Destination: Copy>(
    targets: &mut lir::ProtocolCallTargets<Destination>,
    destination: Destination,
    call: PendingTypedCall,
) -> lir::TypedCall<Destination> {
    match call {
        PendingTypedCall::Void { signature, args } => {
            let target = targets.void.alloc(lir::CallTarget {
                destination,
                signature,
            });
            lir::TypedCall::Void { target, args }
        }
        PendingTypedCall::Direct {
            signature,
            out,
            args,
        } => {
            let target = targets.direct.alloc(lir::CallTarget {
                destination,
                signature,
            });
            lir::TypedCall::Direct { target, out, args }
        }
        PendingTypedCall::IndirectResult {
            signature,
            storage,
            args,
        } => {
            let target = targets.indirect_result.alloc(lir::CallTarget {
                destination,
                signature,
            });
            lir::TypedCall::IndirectResult {
                target,
                storage,
                args,
            }
        }
    }
}

fn runtime_call_protocol(function: lir::RuntimeFunction) -> CallProtocol {
    match function {
        lir::RuntimeFunction::Managed(_) => CallProtocol::Managed,
        lir::RuntimeFunction::NoGc(_) => CallProtocol::NoGc,
    }
}

/// Map a primitive MIR binary operator onto its LIR opcode. Operand and result
/// types come exclusively from the typed MIR expressions.
fn binary_op(op: mir::BinOp) -> lir::BinOp {
    match op {
        mir::BinOp::IntAdd => lir::BinOp::Add,
        mir::BinOp::IntSub => lir::BinOp::Sub,
        mir::BinOp::IntMul => lir::BinOp::Mul,
        mir::BinOp::IntDiv => lir::BinOp::SDiv,
        mir::BinOp::IntLt => lir::BinOp::Lt,
        mir::BinOp::IntLe => lir::BinOp::Le,
        mir::BinOp::IntGt => lir::BinOp::Gt,
        mir::BinOp::IntGe => lir::BinOp::Ge,
        mir::BinOp::IntEq | mir::BinOp::BoolEq => lir::BinOp::Eq,
        mir::BinOp::IntNe | mir::BinOp::BoolNe => lir::BinOp::Ne,
    }
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
    structs: &Arena<lir::StructDef>,
    enums: &Arena<lir::EnumDef>,
    array_types: &'a HashMap<mir::ClassId, lir::ArrayTypeId>,
    type_descriptors: &'a TypeDescriptorRefs,
    local_function_map: &'a HashMap<mir::FunctionId, lir::LocalFunctionId>,
    extern_function_refs: &'a HashMap<mir::ExternFunctionId, LoweredExternFunctionRef>,
    safepoint_ids: &'a mut safepoints::SafepointIds,
) -> lir::Function {
    // Parameters stay SSA values unless `addressOf` requires stable storage.
    // Address-taken parameters are copied once into a method-local slot.
    let address_taken = locals::address_taken(function);
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
        structs,
        enums,
        array_types,
        type_descriptors,
        local_function_map,
        extern_function_refs,
        safepoint_ids,
        local_map,
        locals,
        temps: Arena::new(),
        blocks,
        current: entry,
        block_map,
        current_unwind: None,
        block_count: 0,
        hidden_count: 0,
        returns_void,
        call_targets: lir::CallTargets::default(),
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
        call_targets: lowerer.call_targets,
        locals: lowerer.locals,
        temps: lowerer.temps,
        blocks: lowerer.blocks,
        entry,
    }
}

// Safepoint placement and liveness live in safepoints.rs.

/// Where a MIR local lives in LIR: parameters are SSA values, all
/// other locals get stack slots.
#[derive(Clone, Copy)]
enum LocalSlot {
    Slot(lir::LocalId),
    Param(u32),
}

/// Per-function lowering state: locals, temps, and the basic blocks
/// built so far. Invariant: the `current` block is always unsealed
/// (its terminator is a placeholder); a block is sealed exactly when
/// control flow leaves it. `current_sealed` tracks whether the current
/// block was already sealed (by a `return`, or by a trap call), so
/// structured control flow does not seal it again with a branch.
struct FunctionLowerer<'a> {
    module: &'a mir::Module,
    /// Locals of the MIR function being lowered (for local storage and parameters).
    mir_locals: &'a Arena<mir::Local>,
    global_map: &'a HashMap<mir::StringConstId, lir::GlobalId>,
    storage_globals: &'a HashMap<mir::GlobalId, StorageGlobal>,
    /// Sink for ordinary globals such as trap-message C strings.
    globals: &'a mut Arena<lir::Global>,
    cstr_count: &'a mut usize,
    /// Sink for tuple types encountered in value types (meta layouts).
    layout_types: &'a mut Vec<mir::Type>,
    /// Complete value layouts used to classify return conventions and scans.
    structs: &'a Arena<lir::StructDef>,
    /// Enum definitions with fixed representations (enum value
    /// sizing, e.g. for `scoop_rt_box` payload sizes).
    enums: &'a Arena<lir::EnumDef>,
    /// Complete class-application to array-metadata mapping produced before
    /// any function is lowered.
    array_types: &'a HashMap<mir::ClassId, lir::ArrayTypeId>,
    /// Complete typed TypeDescriptor graph built before body lowering.
    type_descriptors: &'a TypeDescriptorRefs,
    local_function_map: &'a HashMap<mir::FunctionId, lir::LocalFunctionId>,
    extern_function_refs: &'a HashMap<mir::ExternFunctionId, LoweredExternFunctionRef>,
    safepoint_ids: &'a mut safepoints::SafepointIds,
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
    returns_void: bool,
    call_targets: lir::CallTargets,
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

    fn next_safepoint(&mut self) -> lir::SafepointId {
        self.safepoint_ids.allocate()
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
                self.lower_expr(expr);
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
                let value = self.lower_expr(init);
                self.push(lir::Instruction::Store {
                    local: self.local_slot(*local),
                    value,
                });
            }
            mir::StatementKind::Assign { local, value } => {
                let value = self.lower_expr(value);
                self.push(lir::Instruction::Store {
                    local: self.local_slot(*local),
                    value,
                });
            }
            mir::StatementKind::GlobalAssign { global, value } => {
                let value = self.lower_expr(value);
                match *self
                    .storage_globals
                    .get(global)
                    .expect("every MIR global has storage")
                {
                    StorageGlobal::Local(global) => {
                        self.push(lir::Instruction::GlobalStore { global, value })
                    }
                    StorageGlobal::Native(global) => {
                        let safepoint = self.next_safepoint();
                        self.push(lir::Instruction::NativeGlobalStore {
                            global,
                            value,
                            safepoint,
                            roots: lir::NativeSafeRootSet::default(),
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
                let array = self.lower_expr(array);
                let index = self.lower_expr(index);
                let value = self.lower_expr(value);
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
                let object_ty = object.ty.clone();
                let mir::Type::Class(class_id) = &object_ty else {
                    unreachable!("a field store targets a class object")
                };
                let (offsets, _, _) =
                    class_shape(self.module, self.enums, &self.module.classes[*class_id]);
                let offset = offsets[*index as usize];
                let object = self.lower_expr(object);
                let value = self.lower_expr(value);
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
                let object_ty = object.ty.clone();
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
                let object = self.lower_expr(object);
                let value = self.lower_expr(value);
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
                let cond = self.lower_expr(cond);
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
                        self.lower_expr(value);
                        None
                    }
                    (false, Some(value)) => Some(self.lower_expr(value)),
                    (false, None) => unreachable!("non-Unit return without a value"),
                };
                self.seal(lir::Terminator::Return { value });
            }
            mir::Terminator::Throw { exception, unwind } => {
                let value = self.lower_expr(exception);
                if let Some(unwind) = unwind {
                    let normal = self.new_block("throw.normal");
                    let (call, _) =
                        self.typed_call(vec![lir::MANAGED_PTR], lir::LirType::Void, vec![value]);
                    let site = self.invoke_site(
                        lir::CallDestination::Runtime(lir::RuntimeFunction::NoGc(
                            lir::NoGcRuntimeFunction::Throw,
                        )),
                        CallProtocol::NoGc,
                        call,
                        normal,
                        self.block_map[unwind],
                    );
                    self.push(lir::Instruction::Invoke { site });
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
                    let (call, _) = self.typed_call(Vec::new(), lir::LirType::Void, Vec::new());
                    let site = self.invoke_site(
                        lir::CallDestination::Runtime(lir::RuntimeFunction::NoGc(
                            lir::NoGcRuntimeFunction::Rethrow,
                        )),
                        CallProtocol::NoGc,
                        call,
                        normal,
                        self.block_map[unwind],
                    );
                    self.push(lir::Instruction::Invoke { site });
                    self.seal(lir::Terminator::Br(normal));
                    self.enter(normal);
                    self.seal(lir::Terminator::Unreachable);
                }
                None => {
                    self.emit_plain_call(
                        lir::CallDestination::Runtime(lir::RuntimeFunction::NoGc(
                            lir::NoGcRuntimeFunction::Rethrow,
                        )),
                        CallProtocol::NoGc,
                        Vec::new(),
                        lir::LirType::Void,
                        Vec::new(),
                    );
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

    /// Lower a fully typed MIR expression, appending its instructions to
    /// the current block and returning the value it evaluates to.
    fn lower_expr(&mut self, expr: &mir::Expr) -> lir::Value {
        let ty = &expr.ty;
        match &expr.kind {
            mir::ExprKind::StringConst(id) => lir::Value::Global(self.global_map[id]),
            mir::ExprKind::IntLiteral(value) => lir::Value::IntConst(*value),
            mir::ExprKind::BoolLiteral(value) => lir::Value::BoolConst(*value),
            mir::ExprKind::UnitLiteral => self.unit_value(),
            mir::ExprKind::CaughtException => lir::Value::Local(
                self.caught_exception
                    .expect("CaughtException must be dominated by BeginCatch"),
            ),
            mir::ExprKind::TupleLiteral(elements) => {
                let mir::Type::Tuple(element_types) = ty else {
                    unreachable!("a tuple literal has a tuple type")
                };
                assert_eq!(elements.len(), element_types.len(), "tuple literal arity");
                let elements: Vec<lir::Value> = elements
                    .iter()
                    .map(|element| self.lower_expr(element))
                    .collect();
                self.make_aggregate(ty, elements)
            }
            mir::ExprKind::StructInit { struct_id, args } => {
                let field_count = self.module.structs[*struct_id].declared_fields().len();
                assert_eq!(args.len(), field_count, "struct initializer arity");
                let args: Vec<lir::Value> = args.iter().map(|arg| self.lower_expr(arg)).collect();
                self.make_aggregate(ty, args)
            }
            // Raw class construction (only ever inside mir-lower's
            // generated ctor functions): `scoop_rt_alloc(td, size)`,
            // then one heap store per flattened field (the header is
            // followed by naturally aligned fields at fixed byte
            // offsets).
            mir::ExprKind::ClassInit { class_id, args } => {
                let def = &self.module.classes[*class_id];
                let (field_offsets, size, _) = class_shape(self.module, self.enums, def);
                assert_eq!(
                    args.len(),
                    def.declared_fields().len(),
                    "a ClassInit initializes every flattened field"
                );
                let td = self.td_ref(&mir::Type::Class(*class_id));
                let object = self.emit_plain_call(
                    lir::CallDestination::Runtime(lir::RuntimeFunction::Managed(
                        lir::ManagedRuntimeFunction::Alloc,
                    )),
                    CallProtocol::Managed,
                    vec![lir::METADATA_PTR, lir::LirType::I64],
                    lir::MANAGED_PTR,
                    vec![td, lir::Value::IntConst(size as i64)],
                );
                for (arg, offset) in args.iter().zip(field_offsets) {
                    let value = self.lower_expr(arg);
                    self.push(lir::Instruction::HeapStore {
                        object,
                        offset,
                        value,
                    });
                }
                object
            }
            mir::ExprKind::ClosureAlloc { class, captures } => {
                let def = &self.module.closure_classes[*class];
                let (capture_offsets, size, _, _) = closure_shape(self.module, self.enums, def);
                assert_eq!(
                    captures.len(),
                    def.captures.len(),
                    "ClosureAlloc initializes every capture field"
                );
                let td = lir::Value::TypeDescriptor(self.type_descriptors.for_closure(*class));
                let object = self.emit_plain_call(
                    lir::CallDestination::Runtime(lir::RuntimeFunction::Managed(
                        lir::ManagedRuntimeFunction::Alloc,
                    )),
                    CallProtocol::Managed,
                    vec![lir::METADATA_PTR, lir::LirType::I64],
                    lir::MANAGED_PTR,
                    vec![td, lir::Value::IntConst(size as i64)],
                );
                let invoke_function = self.module.closure_invoke_functions[def.invoke].function;
                let invoke_symbol = self.module.functions[invoke_function].symbol.clone();
                let invoke = self.new_temp(lir::CODE_PTR);
                self.push(lir::Instruction::FunctionAddress {
                    out: invoke,
                    symbol: invoke_symbol,
                });
                self.push(lir::Instruction::HeapStore {
                    object,
                    offset: 16,
                    value: lir::Value::Temp(invoke),
                });
                for (capture, offset) in captures.iter().zip(capture_offsets) {
                    let value = self.lower_expr(capture);
                    self.push(lir::Instruction::HeapStore {
                        object,
                        offset,
                        value,
                    });
                }
                object
            }
            mir::ExprKind::ClosureCapture {
                closure,
                class,
                index,
            } => {
                let def = &self.module.closure_classes[*class];
                let (capture_offsets, _, _, _) = closure_shape(self.module, self.enums, def);
                let closure = self.lower_expr(closure);
                let out_ty = self.value_type(ty);
                let out = self.load_at_offset(closure, capture_offsets[*index as usize], out_ty);
                lir::Value::Temp(out)
            }
            // Every operation consumes the exact array application carried by
            // MIR. The LIR value itself is just a managed pointer.
            mir::ExprKind::ArrayLiteral {
                array_type,
                elements,
            } => {
                let elements: Vec<lir::Value> = elements
                    .iter()
                    .map(|element| self.lower_expr(element))
                    .collect();
                let out_ty = self.value_type(ty);
                let out = self.new_temp(out_ty);
                let safepoint = self.next_safepoint();
                self.push(lir::Instruction::ArrayAlloc {
                    out,
                    elements,
                    array_type: self.array_type_id(*array_type),
                    safepoint,
                    live: lir::StatepointLiveSet::default(),
                });
                lir::Value::Temp(out)
            }
            mir::ExprKind::ArrayGet {
                array_type,
                array,
                index,
            } => {
                let array = self.lower_expr(array);
                let index = self.lower_expr(index);
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
            mir::ExprKind::ArrayLen {
                array_type,
                operand,
            } => {
                let operand = self.lower_expr(operand);
                let out = self.new_temp(lir::LirType::I64);
                self.push(lir::Instruction::ArrayLen {
                    out,
                    operand,
                    array_type: self.array_type_id(*array_type),
                });
                lir::Value::Temp(out)
            }
            mir::ExprKind::ArrayClone {
                source_type: _,
                target_type,
                operand,
            } => {
                let operand = self.lower_expr(operand);
                let out_ty = self.value_type(ty);
                let out = self.new_temp(out_ty);
                let safepoint = self.next_safepoint();
                self.push(lir::Instruction::ArrayClone {
                    out,
                    operand,
                    array_type: self.array_type_id(*target_type),
                    safepoint,
                    live: lir::StatepointLiveSet::default(),
                });
                lir::Value::Temp(out)
            }
            mir::ExprKind::Local(local) => self.local_value(*local),
            mir::ExprKind::GlobalRead(global) => {
                let out_ty = self.value_type(ty);
                let out = self.new_temp(out_ty);
                match *self
                    .storage_globals
                    .get(global)
                    .expect("every MIR global has storage")
                {
                    StorageGlobal::Local(global) => {
                        self.push(lir::Instruction::GlobalLoad { out, global })
                    }
                    StorageGlobal::Native(global) => {
                        let safepoint = self.next_safepoint();
                        self.push(lir::Instruction::NativeGlobalLoad {
                            out,
                            global,
                            safepoint,
                            roots: lir::NativeSafeRootSet::default(),
                        })
                    }
                }
                lir::Value::Temp(out)
            }
            mir::ExprKind::PtrFromUInt { operand, .. } => {
                let value = self.lower_expr(operand);
                let out = self.new_temp(lir::RAW_PTR);
                self.push(lir::Instruction::IntToPtr { out, value });
                lir::Value::Temp(out)
            }
            mir::ExprKind::PtrToUInt(operand) => {
                let value = self.lower_expr(operand);
                let out = self.new_temp(lir::LirType::I64);
                self.push(lir::Instruction::PtrToInt { out, value });
                lir::Value::Temp(out)
            }
            mir::ExprKind::PtrCast { operand, .. } => self.lower_expr(operand),
            mir::ExprKind::PtrLoad {
                pointer,
                pointee,
                offset,
            } => {
                let pointer = self.lower_expr(pointer);
                let pointer = if let Some(offset) = offset {
                    let offset = self.lower_expr(offset);
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
            mir::ExprKind::PtrStore {
                pointer,
                pointee,
                offset,
                value,
            } => {
                let pointer = self.lower_expr(pointer);
                let pointer = if let Some(offset) = offset {
                    let offset = self.lower_expr(offset);
                    self.offset_pointer(pointer, pointee, offset, false)
                } else {
                    pointer
                };
                let value = self.lower_expr(value);
                let enum_shape = |id: mir::EnumId| repr_shape(&self.enums[enum_def_id(id)].repr);
                let (_, align) = size_align(self.module, &enum_shape, pointee);
                self.push(lir::Instruction::RawStore {
                    pointer,
                    value,
                    align,
                });
                self.unit_value()
            }
            mir::ExprKind::PtrOffset {
                pointer,
                pointee,
                offset,
                subtract,
            } => {
                let pointer = self.lower_expr(pointer);
                let offset = self.lower_expr(offset);
                self.offset_pointer(pointer, pointee, offset, *subtract)
            }
            mir::ExprKind::AddressOf { local, .. } => {
                let local = self.local_slot(*local);
                let out = self.new_temp(lir::RAW_PTR);
                self.push(lir::Instruction::LocalAddress { out, local });
                lir::Value::Temp(out)
            }
            mir::ExprKind::GlobalAddress { global, .. } => {
                let out = self.new_temp(lir::RAW_PTR);
                match *self
                    .storage_globals
                    .get(global)
                    .expect("every MIR global has storage")
                {
                    StorageGlobal::Local(global) => {
                        self.push(lir::Instruction::GlobalAddress { out, global })
                    }
                    StorageGlobal::Native(global) => {
                        let safepoint = self.next_safepoint();
                        self.push(lir::Instruction::NativeGlobalAddress {
                            out,
                            global,
                            safepoint,
                            roots: lir::NativeSafeRootSet::default(),
                        })
                    }
                }
                lir::Value::Temp(out)
            }
            mir::ExprKind::SizeOf(value_ty) => {
                let enum_shape = |id: mir::EnumId| repr_shape(&self.enums[enum_def_id(id)].repr);
                let (size, _) = size_align(self.module, &enum_shape, value_ty);
                lir::Value::IntConst(size as i64)
            }
            mir::ExprKind::AlignOf(value_ty) => {
                let enum_shape = |id: mir::EnumId| repr_shape(&self.enums[enum_def_id(id)].repr);
                let (_, align) = size_align(self.module, &enum_shape, value_ty);
                lir::Value::IntConst(align as i64)
            }
            mir::ExprKind::FunPtrNull(_) => lir::Value::NullPointer(lir::PointerKind::Code),
            mir::ExprKind::FunctionAddress { callback } => {
                let out = self.new_temp(lir::CODE_PTR);
                self.push(lir::Instruction::FunctionAddress {
                    out,
                    symbol: format!("scoop_c_callback_{}", callback.into_raw().into_u32()),
                });
                lir::Value::Temp(out)
            }
            mir::ExprKind::ForeignCallbackRegister { bridge, closure } => {
                let closure = self.lower_expr(closure);
                let out_ty = self.value_type(ty);
                let out = self.new_temp(out_ty);
                self.push(lir::Instruction::ForeignCallbackRegister {
                    out,
                    bridge: la_arena::Idx::from_raw(bridge.into_raw()),
                    closure,
                });
                lir::Value::Temp(out)
            }
            mir::ExprKind::ForeignCallbackOperation {
                operation,
                callback,
                ..
            } => {
                let callback = self.lower_expr(callback);
                match operation {
                    mir::ForeignCallbackOperation::Release => {
                        self.push(lir::Instruction::ForeignCallbackOperation(
                            lir::ForeignCallbackOperation::Release { callback },
                        ));
                        self.unit_value()
                    }
                    mir::ForeignCallbackOperation::Retain => {
                        let out_ty = self.value_type(ty);
                        let out = self.new_temp(out_ty);
                        self.push(lir::Instruction::ForeignCallbackOperation(
                            lir::ForeignCallbackOperation::Retain { out, callback },
                        ));
                        lir::Value::Temp(out)
                    }
                    mir::ForeignCallbackOperation::State => {
                        let out_ty = self.value_type(ty);
                        let out = self.new_temp(out_ty);
                        self.push(lir::Instruction::ForeignCallbackOperation(
                            lir::ForeignCallbackOperation::State { out, callback },
                        ));
                        lir::Value::Temp(out)
                    }
                    mir::ForeignCallbackOperation::Failure => {
                        let out_ty = self.value_type(ty);
                        let out = self.new_temp(out_ty);
                        self.push(lir::Instruction::ForeignCallbackOperation(
                            lir::ForeignCallbackOperation::Failure { out, callback },
                        ));
                        lir::Value::Temp(out)
                    }
                }
            }
            mir::ExprKind::Retype { operand, .. } => self.lower_expr(operand),
            mir::ExprKind::FieldAccess { receiver, index } => {
                let receiver_ty = receiver.ty.clone();
                let receiver = self.lower_expr(receiver);
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
            mir::ExprKind::AtomicFieldLoad { object, index } => {
                let object_ty = object.ty.clone();
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
                let object = self.lower_expr(object);
                let out = self.new_temp(lir::LirType::I64);
                self.push(lir::Instruction::AtomicLoad {
                    out,
                    object,
                    offset: offsets[*index as usize],
                });
                lir::Value::Temp(out)
            }
            mir::ExprKind::AtomicFieldCompareExchange {
                object,
                index,
                expected,
                replacement,
            } => {
                let object_ty = object.ty.clone();
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
                let object = self.lower_expr(object);
                let expected = self.lower_expr(expected);
                let replacement = self.lower_expr(replacement);
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
            // `scoop_rt_box(td, payload, size, scan)` (runtime spec 2.3): LIR
            // materializes the payload storage and carries its complete
            // recursive scan program into the managed runtime entry.
            mir::ExprKind::Box(operand) => {
                let payload_ty = operand.ty.clone();
                record_layout_types(&payload_ty, self.layout_types);
                let payload = self.lower_expr(operand);
                let payload_lir_type = self.value_type(&payload_ty);
                let payload_storage = self.new_hidden_local(payload_lir_type);
                self.push(lir::Instruction::Store {
                    local: payload_storage,
                    value: payload,
                });
                let payload_address = self.new_temp(lir::RAW_PTR);
                self.push(lir::Instruction::LocalAddress {
                    out: payload_address,
                    local: payload_storage,
                });
                let td = self.td_ref(&payload_ty);
                let enum_shape = |id: mir::EnumId| repr_shape(&self.enums[enum_def_id(id)].repr);
                let (size, _) = size_align(self.module, &enum_shape, &payload_ty);
                let payload_scan = self.call_targets.root_scans.alloc(ref_scan(
                    self.module,
                    self.enums,
                    &payload_ty,
                    0,
                ));
                self.emit_plain_call(
                    lir::CallDestination::Runtime(lir::RuntimeFunction::Managed(
                        lir::ManagedRuntimeFunction::Box,
                    )),
                    CallProtocol::Managed,
                    vec![
                        lir::METADATA_PTR,
                        lir::RAW_PTR,
                        lir::LirType::I64,
                        lir::METADATA_PTR,
                    ],
                    lir::MANAGED_PTR,
                    vec![
                        td,
                        lir::Value::Temp(payload_address),
                        lir::Value::IntConst(size as i64),
                        lir::Value::RootScan(payload_scan),
                    ],
                )
            }
            // The payload sits right behind the 16-byte object header:
            // byte offset 16 of the boxed object (see the module docs).
            mir::ExprKind::Unbox(operand) => {
                let object = self.lower_expr(operand);
                let ty = self.value_type(ty);
                let out = self.load_at_offset(object, 16, ty);
                lir::Value::Temp(out)
            }
            // `scoop_rt_is_instance(obj, td)` (runtime spec 2.3).
            mir::ExprKind::IsInstance { operand, check_ty } => {
                let object = self.lower_expr(operand);
                let td = self.td_ref(check_ty);
                self.emit_plain_call(
                    lir::CallDestination::Runtime(lir::RuntimeFunction::NoGc(
                        lir::NoGcRuntimeFunction::IsInstance,
                    )),
                    CallProtocol::NoGc,
                    vec![lir::MANAGED_PTR, lir::METADATA_PTR],
                    lir::LirType::I1,
                    vec![object, td],
                )
            }
            // mir-lower expands `as` / `as?` into runtime checks plus
            // Option wrapping; the node never reaches LIR.
            mir::ExprKind::Cast { .. } => unreachable!("mir-lower expands casts before LIR"),
            // The enum operations map onto the corresponding LIR
            // instructions; the concrete representation (niche pointer
            // or tagged union) is fixed by the `EnumDef`, so codegen
            // translates them mechanically.
            mir::ExprKind::VariantConstruct { variant, fields } => {
                let mir::Type::Enum(enum_id, _) = ty else {
                    unreachable!("a variant construction has an enum type")
                };
                let field_count = self.module.enums[*enum_id].variants[*variant as usize]
                    .fields
                    .len();
                assert_eq!(fields.len(), field_count, "enum variant field arity");
                let fields: Vec<lir::Value> =
                    fields.iter().map(|field| self.lower_expr(field)).collect();
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
            mir::ExprKind::EnumTag(operand) => {
                let operand_ty = operand.ty.clone();
                let mir::Type::Enum(enum_id, _) = &operand_ty else {
                    unreachable!("a tag read's operand is an enum value")
                };
                let operand = self.lower_expr(operand);
                let out = self.new_temp(lir::LirType::I64);
                self.push(lir::Instruction::EnumTag {
                    out,
                    enum_id: enum_def_id(*enum_id),
                    operand,
                });
                lir::Value::Temp(out)
            }
            mir::ExprKind::EnumField {
                operand,
                variant,
                index,
            } => {
                let operand_ty = operand.ty.clone();
                let mir::Type::Enum(enum_id, _) = &operand_ty else {
                    unreachable!("an enum field read's operand is an enum value")
                };
                let enum_id = *enum_id;
                let operand = self.lower_expr(operand);
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
            mir::ExprKind::Binary { op, lhs, rhs } => {
                let lir_op = binary_op(*op);
                let lhs = self.lower_expr(lhs);
                let rhs = self.lower_expr(rhs);
                let out_ty = self.value_type(ty);
                let out = self.new_temp(out_ty);
                self.push(lir::Instruction::BinOp {
                    out,
                    op: lir_op,
                    lhs,
                    rhs,
                });
                lir::Value::Temp(out)
            }
            mir::ExprKind::Unary { op, operand } => {
                let lir_op = match op {
                    mir::UnOp::IntNeg => lir::UnOp::Neg,
                    mir::UnOp::BoolNot => lir::UnOp::Not,
                };
                let operand = self.lower_expr(operand);
                let out_ty = self.value_type(ty);
                let out = self.new_temp(out_ty);
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
            scan: lir::RefScan::None,
            init: lir::GlobalInit::CString(message.to_string()),
        });
        let block = self.new_block("unwrap.trap");
        // Fill the trap block out of line; the caller seals the
        // suspended current block with the branch.
        let saved = self.current;
        let saved_sealed = self.current_sealed;
        self.enter(block);
        let (call, result) = self.typed_call(
            vec![lir::RAW_PTR],
            lir::LirType::Void,
            vec![lir::Value::Global(global)],
        );
        assert!(result.is_none(), "trap has no value result");
        let site = self.call_site(
            lir::CallDestination::Runtime(lir::RuntimeFunction::NoGc(
                lir::NoGcRuntimeFunction::Trap,
            )),
            CallProtocol::NoGc,
            call,
        );
        self.push(lir::Instruction::Call { site });
        self.seal(lir::Terminator::Unreachable);
        self.trap_blocks.insert(message.to_string(), block);
        self.current = saved;
        self.current_sealed = saved_sealed;
        block
    }

    fn td_ref(&self, ty: &mir::Type) -> lir::Value {
        lir::Value::TypeDescriptor(self.type_descriptors.for_type(ty))
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

    fn typed_call(
        &mut self,
        parameter_types: Vec<lir::LirType>,
        result_type: lir::LirType,
        args: Vec<lir::Value>,
    ) -> (PendingTypedCall, Option<lir::Value>) {
        assert_eq!(parameter_types.len(), args.len(), "typed call arity");
        let calling_convention = lir::CallingConvention::Cdecl;
        if result_type == lir::LirType::Void {
            let signature = self
                .call_targets
                .void_signatures
                .alloc(lir::VoidCallSignature {
                    params: parameter_types,
                    calling_convention,
                });
            return (PendingTypedCall::Void { signature, args }, None);
        }

        let result_scan = safepoints::root_scan(&result_type, self.structs, self.enums, 0);
        if uses_indirect_result(self.enums, &result_type) {
            let signature = self.call_targets.indirect_result_signatures.alloc(
                lir::IndirectResultCallSignature {
                    params: parameter_types,
                    result: lir::ResultStorage {
                        ty: result_type.clone(),
                        scan: result_scan,
                    },
                    calling_convention,
                },
            );
            let storage = self.new_hidden_local(result_type);
            return (
                PendingTypedCall::IndirectResult {
                    signature,
                    storage,
                    args,
                },
                Some(lir::Value::Local(storage)),
            );
        }

        let signature = self
            .call_targets
            .direct_signatures
            .alloc(lir::DirectCallSignature {
                params: parameter_types,
                result: result_type.clone(),
                result_scan,
                calling_convention,
            });
        let out = self.new_temp(result_type);
        (
            PendingTypedCall::Direct {
                signature,
                out,
                args,
            },
            Some(lir::Value::Temp(out)),
        )
    }

    fn call_site(
        &mut self,
        destination: lir::CallDestination,
        protocol: CallProtocol,
        call: PendingTypedCall,
    ) -> lir::CallSite {
        match protocol {
            CallProtocol::Managed => {
                let destination = lir::ManagedCallDestination::from_view(destination)
                    .expect("managed protocol requires a managed destination");
                lir::CallSite::Managed(lir::ManagedCallSite {
                    call: bind_typed_call(
                        &mut self.call_targets.managed_targets,
                        destination,
                        call,
                    ),
                    safepoint: self.safepoint_ids.allocate(),
                    live: lir::StatepointLiveSet::default(),
                })
            }
            CallProtocol::NoGc => {
                let destination = lir::NoGcCallDestination::from_view(destination)
                    .expect("NoGC protocol requires a NoGC destination");
                lir::CallSite::NoGc(lir::NoGcCallSite {
                    call: bind_typed_call(&mut self.call_targets.no_gc_targets, destination, call),
                })
            }
        }
    }

    fn native_call_site(
        &mut self,
        destination: NativeCallDestination,
        call: PendingTypedCall,
    ) -> lir::CallSite {
        match destination {
            NativeCallDestination::Safe(destination) => {
                assert_eq!(
                    call.result_scan(&self.call_targets),
                    &lir::RefScan::None,
                    "native-safe C ABI results must be GC-free"
                );
                lir::CallSite::NativeSafe(lir::NativeSafeCallSite {
                    call: bind_typed_call(
                        &mut self.call_targets.native_safe_targets,
                        destination,
                        call,
                    ),
                    safepoint: self.safepoint_ids.allocate(),
                    roots: lir::NativeSafeRootSet::default(),
                })
            }
            NativeCallDestination::Borrowed(destination) => {
                let result_scan = call.result_scan(&self.call_targets).clone();
                let result = match lir::NonEmptyRefScan::new(result_scan) {
                    None => lir::NativeBorrowedResultRoot::GcFree,
                    Some(scan) => {
                        let storage = match &call {
                            PendingTypedCall::Void { .. } => {
                                unreachable!("void native call cannot have a result root")
                            }
                            PendingTypedCall::Direct { signature, .. } => {
                                let ty = self.call_targets.direct_signatures[*signature]
                                    .result
                                    .clone();
                                self.new_hidden_local(ty)
                            }
                            PendingTypedCall::IndirectResult { storage, .. } => *storage,
                        };
                        lir::NativeBorrowedResultRoot::Rooted { storage, scan }
                    }
                };
                lir::CallSite::NativeBorrowed(lir::NativeBorrowedCallSite {
                    call: bind_typed_call(
                        &mut self.call_targets.native_borrowed_targets,
                        destination,
                        call,
                    ),
                    safepoint: self.safepoint_ids.allocate(),
                    roots: lir::NativeBorrowedRootSet::new(Vec::new(), result),
                })
            }
        }
    }

    fn invoke_site(
        &mut self,
        destination: lir::CallDestination,
        protocol: CallProtocol,
        call: PendingTypedCall,
        normal: lir::BlockId,
        unwind: lir::BlockId,
    ) -> lir::InvokeSite {
        match protocol {
            CallProtocol::Managed => {
                let destination = lir::ManagedCallDestination::from_view(destination)
                    .expect("managed protocol requires a managed destination");
                lir::InvokeSite::Managed(lir::ManagedInvokeSite {
                    call: bind_typed_call(
                        &mut self.call_targets.managed_targets,
                        destination,
                        call,
                    ),
                    safepoint: self.safepoint_ids.allocate(),
                    roots: lir::ExceptionalRootSet::default(),
                    normal,
                    unwind,
                })
            }
            CallProtocol::NoGc => {
                let destination = lir::NoGcCallDestination::from_view(destination)
                    .expect("NoGC protocol requires a NoGC destination");
                lir::InvokeSite::NoGc(lir::NoGcInvokeSite {
                    call: bind_typed_call(&mut self.call_targets.no_gc_targets, destination, call),
                    normal,
                    unwind,
                })
            }
        }
    }

    fn dispatch_destination(
        &mut self,
        table: lir::Value,
        kind: lir::DispatchKind,
        index: u32,
    ) -> lir::CallDestination {
        let slot = self
            .call_targets
            .dispatch_slots
            .alloc(lir::DispatchSlot { kind, index });
        lir::CallDestination::Dispatch { table, slot }
    }

    fn emit_managed_call(
        &mut self,
        destination: lir::CallDestination,
        protocol: CallProtocol,
        parameter_types: Vec<lir::LirType>,
        result_type: lir::LirType,
        args: Vec<lir::Value>,
    ) -> lir::Value {
        assert!(matches!(
            protocol,
            CallProtocol::Managed | CallProtocol::NoGc
        ));
        let (call, value) = self.typed_call(parameter_types, result_type, args);
        if let Some(unwind) = self.current_unwind {
            let normal = self.new_block("invoke.normal");
            let site = self.invoke_site(destination, protocol, call, normal, unwind);
            self.push(lir::Instruction::Invoke { site });
            self.seal(lir::Terminator::Br(normal));
            self.enter(normal);
        } else {
            let site = self.call_site(destination, protocol, call);
            self.push(lir::Instruction::Call { site });
        }
        value.unwrap_or_else(|| self.unit_value())
    }

    fn emit_plain_call(
        &mut self,
        destination: lir::CallDestination,
        protocol: CallProtocol,
        parameter_types: Vec<lir::LirType>,
        result_type: lir::LirType,
        args: Vec<lir::Value>,
    ) -> lir::Value {
        assert!(matches!(
            protocol,
            CallProtocol::Managed | CallProtocol::NoGc
        ));
        let (call, value) = self.typed_call(parameter_types, result_type, args);
        let site = self.call_site(destination, protocol, call);
        self.push(lir::Instruction::Call { site });
        value.unwrap_or_else(|| self.unit_value())
    }

    fn emit_native_call(
        &mut self,
        destination: NativeCallDestination,
        parameter_types: Vec<lir::LirType>,
        result_type: lir::LirType,
        args: Vec<lir::Value>,
    ) -> lir::Value {
        let (call, value) = self.typed_call(parameter_types, result_type, args);
        let site = self.native_call_site(destination, call);
        self.push(lir::Instruction::Call { site });
        value.unwrap_or_else(|| self.unit_value())
    }

    fn emit_native_storage_call(
        &mut self,
        destination: NativeCallDestination,
        parameter_types: Vec<lir::LirType>,
        result_type: lir::LirType,
        result_scan: lir::RefScan,
        args: Vec<lir::Value>,
    ) -> lir::Value {
        assert_ne!(result_type, lir::LirType::Void);
        assert_eq!(parameter_types.len(), args.len(), "typed call arity");
        let signature =
            self.call_targets
                .indirect_result_signatures
                .alloc(lir::IndirectResultCallSignature {
                    params: parameter_types,
                    result: lir::ResultStorage {
                        ty: result_type.clone(),
                        scan: result_scan,
                    },
                    calling_convention: lir::CallingConvention::Cdecl,
                });
        let storage = self.new_hidden_local(result_type);
        let call = PendingTypedCall::IndirectResult {
            signature,
            storage,
            args,
        };
        let site = self.native_call_site(destination, call);
        self.push(lir::Instruction::Call { site });
        lir::Value::Local(storage)
    }

    fn lower_call(&mut self, call: &mir::Call, result_ty: &mir::Type) -> lir::Value {
        match call.target.callee {
            mir::Callee::Extern(id) => {
                assert!(matches!(call.target.kind, mir::CallKind::Direct));
                let extern_ = &self.module.extern_functions[id];
                let parameter_types = extern_.params.clone();
                let returns_unit = extern_.return_type == mir::Type::Unit;
                assert_eq!(call.args.len(), parameter_types.len(), "extern call arity");
                let args = call
                    .args
                    .iter()
                    .map(|arg| self.lower_expr(arg))
                    .collect::<Vec<_>>();
                match self.extern_function_refs[&id] {
                    LoweredExternFunctionRef::C(function) => {
                        let destination = NativeCallDestination::Safe(
                            lir::NativeSafeCallDestination::extern_function(function),
                        );
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
                        let bridge_parameter_types = vec![lir::RAW_PTR; bridge_args.len()];
                        if returns_unit {
                            self.emit_native_call(
                                destination,
                                bridge_parameter_types,
                                lir::LirType::Void,
                                bridge_args,
                            )
                        } else {
                            let result_type = self.value_type(result_ty);
                            self.emit_native_storage_call(
                                destination,
                                bridge_parameter_types,
                                result_type,
                                lir::RefScan::None,
                                bridge_args,
                            )
                        }
                    }
                    LoweredExternFunctionRef::Scoop(function) => {
                        let destination = NativeCallDestination::Borrowed(
                            lir::NativeBorrowedCallDestination::extern_function(function),
                        );
                        let parameter_types = parameter_types
                            .iter()
                            .map(|ty| self.value_type(ty))
                            .collect();
                        let result_type = if returns_unit {
                            lir::LirType::Void
                        } else {
                            self.value_type(result_ty)
                        };
                        self.emit_native_call(destination, parameter_types, result_type, args)
                    }
                }
            }
            mir::Callee::FunctionBridge(function_type) => {
                let signature = self.module.function_types[function_type].clone();
                let mut parameter_types = Vec::with_capacity(call.args.len());
                parameter_types.push(mir::Type::Any);
                parameter_types.extend(signature.parameter_types);
                for arg in call.args.iter().skip(parameter_types.len()) {
                    parameter_types.push(arg.ty.clone());
                }
                assert_eq!(
                    call.args.len(),
                    parameter_types.len(),
                    "only suspend function bridges add a hidden continuation argument"
                );
                let args: Vec<lir::Value> =
                    call.args.iter().map(|arg| self.lower_expr(arg)).collect();
                let td = self.load_at_offset(args[0], 0, lir::METADATA_PTR);
                let target_td = self.td_ref(&mir::Type::Function(function_type));
                let table = self.emit_plain_call(
                    lir::CallDestination::Runtime(lir::RuntimeFunction::NoGc(
                        lir::NoGcRuntimeFunction::ITableLookup,
                    )),
                    CallProtocol::NoGc,
                    vec![lir::METADATA_PTR, lir::METADATA_PTR],
                    lir::METADATA_PTR,
                    vec![lir::Value::Temp(td), target_td],
                );
                let destination =
                    self.dispatch_destination(table, lir::DispatchKind::FunctionBridge, 0);
                self.finish_indirect(
                    destination,
                    CallProtocol::Managed,
                    args,
                    parameter_types,
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
                    parameter_types.push(arg.ty.clone());
                }
                assert_eq!(
                    call.args.len(),
                    parameter_types.len(),
                    "only suspend closure calls add a hidden continuation argument"
                );
                let args: Vec<lir::Value> =
                    call.args.iter().map(|arg| self.lower_expr(arg)).collect();
                self.finish_closure(
                    args[0],
                    args,
                    parameter_types,
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
                let effect = match callee.gc_effect {
                    mir::GcEffect::Managed => CallProtocol::Managed,
                    mir::GcEffect::NoGc => CallProtocol::NoGc,
                };
                assert_eq!(call.args.len(), param_types.len(), "user call arity");
                // Arguments are evaluated left to right, before the call.
                let args: Vec<lir::Value> =
                    call.args.iter().map(|arg| self.lower_expr(arg)).collect();
                match call.target.kind {
                    mir::CallKind::Direct => {
                        let destination = lir::CallDestination::Local(self.local_function_map[&id]);
                        self.finish_call(
                            destination,
                            effect,
                            args,
                            param_types,
                            returns_unit,
                            result_ty,
                        )
                    }
                    // vtable dispatch (impl spec 2.9): the receiver's
                    // object header holds the TypeDescriptor, whose
                    // vtable pointer is `ScoopTypeDescriptor` field 5.
                    mir::CallKind::Virtual { slot } => {
                        let td = self.load_at_offset(args[0], 0, lir::METADATA_PTR);
                        let vtable =
                            self.load_at_offset(lir::Value::Temp(td), 5 * 8, lir::METADATA_PTR);
                        let destination = self.dispatch_destination(
                            lir::Value::Temp(vtable),
                            lir::DispatchKind::Virtual,
                            slot,
                        );
                        self.finish_indirect(
                            destination,
                            effect,
                            args,
                            param_types,
                            returns_unit,
                            result_ty,
                        )
                    }
                    // itable dispatch: `scoop_rt_itable_lookup(td,
                    // iface_td)` finds the interface's table by its
                    // TypeDescriptor key.
                    mir::CallKind::Interface { interface, slot } => {
                        let td = self.load_at_offset(args[0], 0, lir::METADATA_PTR);
                        let iface_td = self.td_ref(&mir::Type::Interface(interface));
                        let table = self.emit_plain_call(
                            lir::CallDestination::Runtime(lir::RuntimeFunction::NoGc(
                                lir::NoGcRuntimeFunction::ITableLookup,
                            )),
                            CallProtocol::NoGc,
                            vec![lir::METADATA_PTR, lir::METADATA_PTR],
                            lir::METADATA_PTR,
                            vec![lir::Value::Temp(td), iface_td],
                        );
                        let destination =
                            self.dispatch_destination(table, lir::DispatchKind::Interface, slot);
                        self.finish_indirect(
                            destination,
                            effect,
                            args,
                            param_types,
                            returns_unit,
                            result_ty,
                        )
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
                let message = match &call.args[0].kind {
                    mir::ExprKind::StringConst(id) => self.module.strings[*id].value.clone(),
                    _ => unreachable!("the trap message is a string constant"),
                };
                let trap = self.trap_block(&message);
                self.seal(lir::Terminator::Br(trap));
                self.current_sealed = true;
                // Dead value: the block is sealed, nothing consumes it.
                lir::Value::IntConst(0)
            }
            mir::Callee::Runtime(function) => {
                let expected_arg_count = match function {
                    mir::RuntimeFn::StringConcat => 2,
                    // The GC intrinsics (M9, runtime spec 3.4): the
                    // pin / handle operations speak raw machine words
                    // — the object reference in, the word out (or the
                    // reverse); the hooks take nothing.
                    mir::RuntimeFn::Pin
                    | mir::RuntimeFn::GetHandle
                    | mir::RuntimeFn::Unpin
                    | mir::RuntimeFn::ReleaseHandle
                    | mir::RuntimeFn::MaterializeException => 1,
                    mir::RuntimeFn::GcCollect | mir::RuntimeFn::GcStats => 0,
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
                assert_eq!(call.args.len(), expected_arg_count, "runtime call arity");
                let args: Vec<lir::Value> =
                    call.args.iter().map(|arg| self.lower_expr(arg)).collect();
                let (parameter_types, result_type) = match function {
                    mir::RuntimeFn::StringConcat => {
                        (vec![lir::MANAGED_PTR, lir::MANAGED_PTR], lir::MANAGED_PTR)
                    }
                    // The pin / handle intrinsics exchange a word with
                    // the runtime: `pin` / `getGcHandle` yield the raw
                    // word (i64), `unpin` / `releaseGcHandle` yield the
                    // reference (ptr), `gcStats` yields the count.
                    mir::RuntimeFn::Pin | mir::RuntimeFn::GetHandle | mir::RuntimeFn::GcStats => {
                        let params = if function == mir::RuntimeFn::GcStats {
                            Vec::new()
                        } else {
                            vec![lir::MANAGED_PTR]
                        };
                        (params, lir::LirType::I64)
                    }
                    mir::RuntimeFn::Unpin | mir::RuntimeFn::ReleaseHandle => {
                        (vec![lir::LirType::I64], lir::MANAGED_PTR)
                    }
                    // `BeginCatch` publishes the registered stable external
                    // exception object as a managed reference. The runtime
                    // materializer consumes that same typed reference even
                    // though its base is outside the moving heap.
                    mir::RuntimeFn::MaterializeException => {
                        (vec![lir::MANAGED_PTR], lir::MANAGED_PTR)
                    }
                    mir::RuntimeFn::GcCollect => (Vec::new(), lir::LirType::Void),
                    mir::RuntimeFn::Box
                    | mir::RuntimeFn::IsInstance
                    | mir::RuntimeFn::ITableLookup => {
                        unreachable!("{function:?} calls are emitted by the dedicated M6 lowerings")
                    }
                    mir::RuntimeFn::Trap => unreachable!("trap calls never reach here"),
                };
                let function = lower_runtime_function(function);
                self.emit_plain_call(
                    lir::CallDestination::Runtime(function),
                    runtime_call_protocol(function),
                    parameter_types,
                    result_type,
                    args,
                )
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
        destination: lir::CallDestination,
        protocol: CallProtocol,
        args: Vec<lir::Value>,
        parameter_types: Vec<mir::Type>,
        returns_unit: bool,
        result_ty: &mir::Type,
    ) -> lir::Value {
        let parameter_types = parameter_types
            .iter()
            .map(|ty| self.value_type(ty))
            .collect();
        let result_type = if returns_unit {
            lir::LirType::Void
        } else {
            self.value_type(result_ty)
        };
        self.emit_managed_call(destination, protocol, parameter_types, result_type, args)
    }

    /// An indirect call through a function table (vtable / itable
    /// dispatch, impl spec 2.9). Inside a try body it is invoked to
    /// the innermost landing pad, like `finish_call`.
    fn finish_indirect(
        &mut self,
        destination: lir::CallDestination,
        protocol: CallProtocol,
        args: Vec<lir::Value>,
        parameter_types: Vec<mir::Type>,
        returns_unit: bool,
        result_ty: &mir::Type,
    ) -> lir::Value {
        self.finish_call(
            destination,
            protocol,
            args,
            parameter_types,
            returns_unit,
            result_ty,
        )
    }

    /// A managed closure call through the code pointer already loaded from
    /// the closure object. Its unwind behavior is identical to direct and
    /// table-indirect managed calls.
    fn finish_closure(
        &mut self,
        closure: lir::Value,
        args: Vec<lir::Value>,
        parameter_types: Vec<mir::Type>,
        returns_unit: bool,
        result_ty: &mir::Type,
    ) -> lir::Value {
        let destination = self.dispatch_destination(closure, lir::DispatchKind::Closure, 2);
        self.finish_indirect(
            destination,
            CallProtocol::Managed,
            args,
            parameter_types,
            returns_unit,
            result_ty,
        )
    }
}
#[cfg(test)]
mod tests;
