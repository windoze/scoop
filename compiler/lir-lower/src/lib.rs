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
//! Typed MIR variant construction maps onto typed `EnumWrap`, while the
//! legacy `EnumTag` / `EnumField` readers remain for coroutine lowering.
//! Representation-independent typed operations map onto `VariantTest` /
//! `VariantPayloadProject`. Codegen translates both paths mechanically per
//! the representation while the remaining readers migrate.
//! Enum layouts keep fixed ref offsets for all disjoint
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
//! M19's `mir::ExprKind::ClassAlloc` is the exact allocation primitive and
//! only calls `scoop_rt_alloc(td, size)`. Initializer functions receive the
//! resulting managed reference and emit ordinary `HeapStore`s; base and
//! `this` edges are ordinary managed direct calls on the same rooted object.
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

mod identity_roots;
use identity_roots::IdentityRoots;

/// Lower MIR to LIR.
pub fn lower(module: &mir::Module, target_profile: lir::LirTargetProfile) -> lir::Module {
    module
        .validate()
        .unwrap_or_else(|error| panic!("invalid MIR input to lir-lower: {error}"));
    let context = LoweringContext::new(target_profile);
    let identity_roots = IdentityRoots::new(module);
    // Every MIR string constant becomes a global with the same symbol.
    let mut globals = Arena::new();
    let mut string_global_map: HashMap<mir::StringConstId, lir::GlobalId> = HashMap::new();
    for (id, string) in module.strings.iter() {
        let global = globals.alloc(lir::Global {
            address_kind: lir::PointerKind::Managed,
            scan: lir::RefScan::None,
            init: lir::GlobalInit::StringConst {
                identity: lir::ImmortalObjectIdentity::from_key(
                    string.identity.clone(),
                    identity_roots.for_immortal_object(&string.identity),
                )
                .expect("validated MIR string identities have canonical CBOR records"),
                value: string.value.clone(),
            },
        });
        string_global_map.insert(id, global);
    }

    // Enum definitions with fixed representations, in the MIR arena's
    // order: `mir::EnumId` and `lir::EnumDefId` align.
    let enums = lower_enums(&context, module);
    // Struct ids also transpose 1:1. Their definitions retain the exact
    // physical layout needed by codegen and C bridge generation.
    let structs = lower_structs(&context, module, &enums);
    let native_abi = native_abi::lower(&context, module, &structs, &enums);
    // Classify every final MIR function before any body is lowered. Callee
    // definitions and all statically selected call sites reuse these exact
    // signatures rather than independently rebuilding a physical ABI.
    let function_signatures = module
        .functions
        .iter()
        .map(|(id, function)| {
            let signature = abi::classify_mir_signature(
                &context,
                function.params.iter().map(|parameter| &parameter.ty),
                &function.return_ty,
                &structs,
                &enums,
            );
            (id, signature)
        })
        .collect::<HashMap<_, _>>();
    let (extern_functions, extern_function_refs) = lower_extern_functions(
        &context,
        module,
        &structs,
        &enums,
        &native_abi.native_externals,
    );
    let (storage_globals, native_globals, native_global_bridges) = lower_globals(
        GlobalLoweringInputs {
            context: &context,
            identity_roots: &identity_roots,
            module,
            structs: &structs,
            enums: &enums,
            string_globals: &string_global_map,
            native_externals: &native_abi.native_externals,
        },
        &mut globals,
    );
    let mut local_function_identities = lir::LocalFunctionIdentities::default();
    let local_function_map = module
        .top_level
        .iter()
        .map(|id| {
            let reference = match module.functions[*id].gc_effect {
                mir::GcEffect::Managed => {
                    lir::LocalFunctionRef::Managed(local_function_identities.alloc_managed())
                }
                mir::GcEffect::NoGc => {
                    lir::LocalFunctionRef::NoGc(local_function_identities.alloc_no_gc())
                }
            };
            (*id, reference)
        })
        .collect::<HashMap<_, _>>();
    let callback_bridges = lower_callback_bridges(
        module,
        &structs,
        &enums,
        &local_function_map,
        &native_abi.callback_signatures,
    );
    let foreign_callback_families = lower_foreign_callback_families(module, &enums);
    let foreign_callback_bridges = lower_foreign_callback_bridges(
        module,
        &structs,
        &enums,
        &local_function_map,
        &native_abi.callback_signatures,
    );
    let initialization_units =
        lower_initialization_units(module, &storage_globals, &local_function_map);
    let (type_descriptors, type_descriptor_refs, well_known_type_descriptors) = type_descriptors(
        &context,
        &identity_roots,
        module,
        &enums,
        &local_function_map,
    );
    let (arrays, array_type_map) = array_types(
        &context,
        &identity_roots,
        module,
        &enums,
        &type_descriptor_refs,
    );

    // Tuple types encountered while mapping value types, in
    // first-appearance order; each one gets a meta layout.
    let mut layout_types = Vec::new();
    // Trap message globals (`scoop.cstr.N`), numbered in creation order.
    let mut cstr_count = 0usize;
    let lowered_functions = module
        .top_level
        .iter()
        .map(|&id| {
            lower_function(
                &context,
                module,
                callable_body_identity(module, id),
                &module.functions[id],
                &function_signatures[&id],
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
                &function_signatures,
                &extern_functions,
                &extern_function_refs,
            )
        })
        .collect::<Vec<_>>();
    let functions = lowered_functions
        .into_iter()
        .map(|function| safepoints::complete_function(&context, function, &structs, &enums))
        .collect();

    let (layouts, well_known_layouts) =
        layouts(&context, &identity_roots, module, &enums, &layout_types);
    lir::Module {
        cone: module.cone,
        globals,
        initialization_units,
        structs,
        enums,
        functions,
        extern_functions,
        native_globals,
        native_global_bridges,
        callback_bridges,
        foreign_callback_families,
        foreign_callback_bridges,
        entry: local_function_map[&module.entry],
        meta: lir::LirMeta {
            target_profile: context.target_profile(),
            canonical_c_abi: native_abi.canonical_c_abi,
            native_externals: native_abi.native_externals,
            well_known_layouts,
            well_known_type_descriptors,
            arrays,
            layouts,
            type_descriptors,
            core_external_type_descriptors: Arena::new(),
            core_external_callables: Arena::new(),
        },
    }
}

fn callable_body_identity(
    module: &mir::Module,
    function: mir::FunctionId,
) -> lir::CallableBodyIdentity {
    let subject = module
        .meta
        .callable_signature_subject(function)
        .expect("validated MIR gives every emitted function one callable subject");
    let identity = match subject {
        mir::CallableSignatureSubject::Strong(owner) => match owner {
            mir::CallableOwner::Function(id) => lir::CallableBodyIdentity::for_function(id),
            mir::CallableOwner::Constructor(id) => lir::CallableBodyIdentity::for_constructor(id),
            mir::CallableOwner::Accessor(id) => {
                lir::CallableBodyIdentity::for_property_accessor(id)
            }
            mir::CallableOwner::Generated(id) => {
                lir::CallableBodyIdentity::for_generated_callable(id)
            }
            mir::CallableOwner::GenericTemplate(_) | mir::CallableOwner::Application(_) => {
                panic!("validated MIR cannot assign a non-defining strong callable subject")
            }
        },
        mir::CallableSignatureSubject::Odr(member) => {
            lir::CallableBodyIdentity::for_odr_member(member)
        }
    };
    identity.expect("validated callable-body subjects have canonical runtime identities")
}

mod abi;
mod callbacks;
mod externs;
mod globals;
mod initialization;
mod locals;
mod metadata;
mod native_abi;
mod runtime;
mod safepoints;
mod target;

use callbacks::*;
use externs::*;
use globals::*;
use initialization::*;
use metadata::*;
use runtime::*;
use target::*;

mod function;
use function::lower_function;

#[cfg(test)]
mod tests;
