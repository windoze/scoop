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
//! The strong production entry can still reject a persistent materialization
//! that requires ODR ownership. This is a target-profile capability error,
//! not a source-language type error.
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
//!   Boxing carries a refined BoxedValue descriptor and a complete typed
//!   payload local or logical ZST. Unboxing validates exact runtime identity.
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

mod error;
pub use error::*;

mod identity_roots;
use identity_roots::IdentityRoots;

mod capability;
use capability::validate_strong_materialization;
pub use capability::{StrongLirCapabilityError, StrongLirMaterializationRequirement};

mod cross_cone_bridge;
pub use cross_cone_bridge::{CrossConeLirBridgeLoweringError, lower_cross_cone_bridge_section};

mod exact_layouts;
pub use exact_layouts::{ExactLayoutLoweringError, lower_exact_layout_exports};

mod exact_callable_abi;
pub use exact_callable_abi::{ExactCallableAbiLoweringError, lower_exact_callable_abi_export};

mod imported_core;
pub use imported_core::StrongImportedCoreLirInput;
use imported_core::lower_imported_core_callables;
use imported_core::lower_imported_core_runtime_string;
mod imported_dependency;
pub use imported_dependency::StrongImportedDependencyLirInput;
use imported_dependency::lower_imported_dependency_callables;

mod strong_production_v2;
pub use strong_production_v2::{
    StrongProductionV2ProjectionError, project_external_initialization_uses_v2,
};

/// Lower one sealed single-Cone strong MIR product to ODR-free LIR.
pub fn lower(
    input: &mir::SingleConeStrongMirInput,
    imported_core: StrongImportedCoreLirInput<'_>,
    target_profile: lir::LirTargetProfile,
) -> Result<lir::SingleConeStrongLirOutput, StrongLirLoweringError> {
    lower_with_dependencies(
        input,
        imported_core,
        StrongImportedDependencyLirInput::Unused,
        target_profile,
    )
}

/// Lower one sealed strong MIR product together with the exact selected
/// ordinary-dependency LIR authority that closes its dependency call roots.
pub fn lower_with_dependencies(
    input: &mir::SingleConeStrongMirInput,
    imported_core: StrongImportedCoreLirInput<'_>,
    imported_dependencies: StrongImportedDependencyLirInput<'_>,
    target_profile: lir::LirTargetProfile,
) -> Result<lir::SingleConeStrongLirOutput, StrongLirLoweringError> {
    let module = input.module();
    module
        .validate()
        .unwrap_or_else(|error| panic!("invalid MIR input to lir-lower: {error}"));
    let context = LoweringContext::new(target_profile);
    let identity_roots = IdentityRoots::new(input);
    validate_strong_materialization(input, &identity_roots)
        .map_err(StrongLirLoweringError::Capability)?;
    // Every string selected by the sealed materialization plan becomes a
    // global with the same typed identity.
    let mut globals = Arena::new();
    let mut string_global_map: HashMap<mir::StringConstId, lir::GlobalId> = HashMap::new();
    for &id in input.materialization().strings() {
        let string = &module.strings[id];
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
    let enums = lower_enums(&context, module)?;
    // Struct ids also transpose 1:1. Their definitions retain the exact
    // physical layout needed by codegen and C bridge generation.
    let structs = lower_structs(&context, module, &enums)?;
    let (external_type_descriptors, imported_runtime_string) =
        lower_imported_core_runtime_string(input, imported_core)?;
    let (mut external_callables, core_external_callable_map) =
        lower_imported_core_callables(&context, input, imported_core, &structs, &enums)?;
    let dependency_external_callable_map = lower_imported_dependency_callables(
        &context,
        input,
        imported_dependencies,
        &structs,
        &enums,
        &mut external_callables,
    )?;
    let native_abi = native_abi::lower(
        &context,
        module,
        &structs,
        &enums,
        identity_roots
            .source_nominal_shapes()
            .iter()
            .map(mir::StrongSourceNominalShapeRoot::ty),
    )?;
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
            )?;
            Ok((id, signature))
        })
        .collect::<StorageResult<HashMap<_, _>>>()?;
    let (extern_functions, extern_function_refs) = lower_extern_functions(
        &context,
        module,
        &structs,
        &enums,
        &native_abi.native_externals,
    )?;
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
    )?;
    let callable_owners = input
        .materialization()
        .callable_roots()
        .iter()
        .map(|root| (root.function(), root.implementation()))
        .collect::<HashMap<_, _>>();
    let mut local_function_identities = lir::LocalFunctionIdentities::default();
    let local_function_map = module
        .top_level
        .iter()
        .map(|&id| {
            assert!(
                callable_owners.contains_key(&id),
                "every emitted function is selected by the sealed strong plan"
            );
            let reference = match module.functions[id].gc_effect {
                mir::GcEffect::Managed => {
                    lir::LocalFunctionRef::Managed(local_function_identities.alloc_managed())
                }
                mir::GcEffect::NoGc => {
                    lir::LocalFunctionRef::NoGc(local_function_identities.alloc_no_gc())
                }
            };
            (id, reference)
        })
        .collect::<HashMap<_, _>>();
    let root_gateway_ref = match input.production().entry_bridge() {
        mir::EntryMirBridgeBranchV1::Library => None,
        mir::EntryMirBridgeBranchV1::Executable(_) => {
            Some(local_function_identities.alloc_managed())
        }
    };
    let startup_gateways = module
        .initialization_units
        .iter()
        .filter(|(_, unit)| matches!(unit.schedule, mir::InitializationSchedule::EagerStartup))
        .map(|(_, unit)| {
            let lir::LocalFunctionRef::Managed(ensure) = local_function_map[&unit.ensure] else {
                unreachable!("initialization ensure functions are always managed")
            };
            let reference = local_function_identities.alloc_managed();
            let gateway = lower_initialization_startup_gateway(unit.identity.id(), ensure);
            (reference, gateway)
        })
        .collect::<Vec<_>>();
    let root_artifacts = match input.production().entry_bridge() {
        mir::EntryMirBridgeBranchV1::Library => None,
        mir::EntryMirBridgeBranchV1::Executable(bridge) => {
            let mir::MirOutput::Executable { entry } = module.output else {
                unreachable!("the sealed MIR input keeps output and entry proof aligned")
            };
            Some(lower_root_artifacts(
                module.cone,
                bridge.source(),
                local_function_map[&entry],
                context.target_profile(),
                &mut globals,
            ))
        }
    };
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
        imported_runtime_string,
    )?;
    let (arrays, array_type_map) = array_types(
        &context,
        &identity_roots,
        module,
        &enums,
        &type_descriptor_refs,
    )?;

    let mut lowered_functions = module
        .top_level
        .iter()
        .map(|&id| {
            lower_function(
                &context,
                module.cone,
                module,
                callable_body_identity(callable_owners[&id]),
                &module.functions[id],
                &function_signatures[&id],
                &string_global_map,
                &storage_globals,
                &mut globals,
                &structs,
                &enums,
                &array_type_map,
                &type_descriptor_refs,
                &local_function_map,
                &function_signatures,
                &external_callables,
                &core_external_callable_map,
                &dependency_external_callable_map,
                &extern_functions,
                &extern_function_refs,
            )
        })
        .collect::<StorageResult<Vec<_>>>()?;
    if let (Some(reference), Some(gateway)) = (root_gateway_ref, root_artifacts) {
        assert_eq!(
            reference.declaration().into_u32() as usize,
            lowered_functions.len(),
            "the root gateway reference must address its appended LIR function"
        );
        lowered_functions.push(gateway);
    }
    for (reference, gateway) in startup_gateways {
        assert_eq!(
            reference.declaration().into_u32() as usize,
            lowered_functions.len(),
            "an initialization startup-gateway reference must address its appended LIR function"
        );
        lowered_functions.push(gateway);
    }
    let functions = lowered_functions
        .into_iter()
        .map(|function| safepoints::complete_function(&context, function, &structs, &enums))
        .collect::<StorageResult<Vec<_>>>()?;

    let core_lir_bridge =
        lower_core_lir_bridge(input, module, &functions, &enums, &local_function_map)?;

    let layouts = layouts(
        &context,
        &identity_roots,
        module,
        &enums,
        imported_runtime_string.is_none(),
    )?;
    let imported_runtime_string_exact = external_type_descriptors
        .iter()
        .next()
        .map(|(_, descriptor)| descriptor.target());
    let module = lir::Module {
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
        output: match module.output {
            mir::MirOutput::Library => lir::LirOutput::Library,
            mir::MirOutput::Executable { entry } => lir::LirOutput::Executable {
                entry: local_function_map[&entry],
            },
        },
        meta: lir::LirMeta {
            exact_types: materialized_exact_types(input, imported_runtime_string_exact),
            target_profile: context.target_profile(),
            canonical_c_abi: native_abi.canonical_c_abi,
            native_externals: native_abi.native_externals,
            well_known_type_descriptors,
            arrays,
            layouts,
            type_descriptors,
            external_type_descriptors,
            external_callables,
        },
    };
    lir::SingleConeStrongLirOutput::try_new(
        module,
        input.materialization().shape_support_sources().to_vec(),
        core_lir_bridge,
    )
    .map_err(StrongLirLoweringError::Output)
}

fn lower_core_lir_bridge(
    input: &mir::SingleConeStrongMirInput,
    module: &mir::Module,
    functions: &[lir::Function],
    enums: &lir::EnumDefs,
    local_function_map: &HashMap<mir::FunctionId, lir::LocalFunctionRef>,
) -> Result<lir::CoreLirBridgeBranchV1, StrongLirLoweringError> {
    let mir::CoreMirBridgeBranchV1::Core(core) = input.production().core_bridge() else {
        return Ok(lir::CoreLirBridgeBranchV1::NotCore);
    };

    let cycle = core.initialization_cycle_thrower();
    let cycle_implementation = cycle.implementation();
    let cycle_root = input
        .materialization()
        .callable_roots()
        .iter()
        .find(|root| root.implementation() == cycle_implementation)
        .ok_or(StrongLirLoweringError::MissingCoreCallableMaterialization(
            cycle_implementation,
        ))?;
    let cycle_exact = input
        .production()
        .strong_callable_bridges()
        .bridges()
        .iter()
        .find(|bridge| bridge.implementation() == cycle_implementation)
        .ok_or(StrongLirLoweringError::MissingCoreCallableSignature(
            cycle_implementation,
        ))?
        .signature();
    if cycle_exact.effect() != scoop_identity::Effect::Ordinary {
        return Err(StrongLirLoweringError::UnsupportedCoreCallableEffect(
            cycle_implementation,
        ));
    }
    if cycle_exact.receiver().is_present() {
        return Err(StrongLirLoweringError::UnsupportedCoreCallableReceiver(
            cycle_implementation,
        ));
    }
    let cycle_function = &module.functions[cycle_root.function()];
    let cycle_lowered = &functions[local_function_map[&cycle_root.function()]
        .declaration()
        .into_u32() as usize];
    let cycle_owner = strong_callable_owner(cycle_implementation).ok_or(
        StrongLirLoweringError::UnsupportedCoreCallableOwner(cycle_implementation),
    )?;
    let cycle_abi_signature = native_abi::canonical_scoop_signature(
        module,
        enums,
        cycle_exact.clone(),
        &cycle_function
            .params
            .iter()
            .map(|parameter| parameter.ty.clone())
            .collect::<Vec<_>>(),
        &cycle_function.return_ty,
        cycle_function.gc_effect,
        &cycle_lowered.signature,
    );
    let cycle_root_plan = match cycle_function.gc_effect {
        mir::GcEffect::Managed => lir::ExternalCallableRootPlan::ManagedStatepoint,
        mir::GcEffect::NoGc => lir::ExternalCallableRootPlan::NoGc,
    };
    let cycle = lir::CoreLirInitializationCycleThrowerV1::new(
        cycle_owner,
        cycle_abi_signature,
        cycle_lowered.signature.calling_convention(),
        cycle_root_plan,
    )
    .map_err(StrongLirLoweringError::CoreLirBridge)?;
    Ok(lir::CoreLirBridgeBranchV1::Core(lir::CoreLirBridgeV1::new(
        cycle,
    )))
}

fn strong_callable_owner(
    owner: scoop_identity::CallableOwner,
) -> Option<scoop_identity::StrongCallableDefinitionOwner> {
    Some(match owner {
        scoop_identity::CallableOwner::Function(id) => {
            scoop_identity::StrongCallableDefinitionOwner::Function(id)
        }
        scoop_identity::CallableOwner::Constructor(id) => {
            scoop_identity::StrongCallableDefinitionOwner::Constructor(id)
        }
        scoop_identity::CallableOwner::Accessor(id) => {
            scoop_identity::StrongCallableDefinitionOwner::PropertyAccessor(id)
        }
        scoop_identity::CallableOwner::Generated(id) => {
            scoop_identity::StrongCallableDefinitionOwner::GeneratedCallable(id)
        }
        scoop_identity::CallableOwner::GenericTemplate(_)
        | scoop_identity::CallableOwner::Application(_) => return None,
    })
}

fn materialized_exact_types(
    input: &mir::SingleConeStrongMirInput,
    imported_runtime_string: Option<scoop_identity::PersistentExactTypeId>,
) -> Vec<
    scoop_identity::CborIdentityRecord<
        scoop_identity::PersistentExactTypeId,
        scoop_identity::ExactTypeKey,
    >,
> {
    let module = input.module();
    let mut records = input
        .materialization()
        .source_nominal_shapes()
        .iter()
        .filter(|root| Some(root.exact()) != imported_runtime_string)
        .map(|root| {
            let record = exact_type_record(module, root.ty());
            assert_eq!(record.id(), root.exact());
            assert_eq!(
                record.key(),
                &scoop_identity::ExactTypeKey::Nominal(root.source())
            );
            record.clone()
        })
        .chain(
            input
                .materialization()
                .generated_nominal_shapes()
                .iter()
                .map(|root| {
                    let record = generated_exact_type_record(module, root.location());
                    assert_eq!(record.id(), root.exact());
                    assert_eq!(
                        record.key(),
                        &scoop_identity::ExactTypeKey::Nominal(root.nominal())
                    );
                    record.clone()
                }),
        )
        .collect::<Vec<_>>();
    records.sort_unstable_by_key(|record| record.id());
    records
}

fn callable_body_identity(owner: mir::CallableOwner) -> lir::CallableBodyIdentity {
    let identity = match owner {
        mir::CallableOwner::Function(id) => lir::CallableBodyIdentity::for_function(id),
        mir::CallableOwner::Constructor(id) => lir::CallableBodyIdentity::for_constructor(id),
        mir::CallableOwner::Accessor(id) => lir::CallableBodyIdentity::for_property_accessor(id),
        mir::CallableOwner::Generated(id) => lir::CallableBodyIdentity::for_generated_callable(id),
        mir::CallableOwner::GenericTemplate(_) | mir::CallableOwner::Application(_) => {
            unreachable!("the sealed strong plan excludes non-defining callable owners")
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
mod production;
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

pub use production::lower_entry_production_source;
use production::{lower_initialization_startup_gateway, lower_root_artifacts};

mod function;
use function::lower_function;

#[cfg(test)]
mod tests;
