//! Shared physical lowering before profile-specific finalization and sealing.

use super::*;

pub(super) struct LoweredModule {
    pub module: lir::Module,
}

mod prepared;
pub use prepared::{PreparedLir, prepare};

pub(super) fn lower_graph(
    prepared: PreparedLir<'_, '_>,
    plans: lir::AbiPlanSet,
) -> Result<LoweredModule, LirLoweringError> {
    let PreparedLir {
        input,
        selected_layout,
        context,
        identity_roots,
        external_type_descriptors,
        dependency_types,
        imported_runtime_string,
        mut globals,
        string_global_map,
        enums,
        structs,
        external_callables,
        external_callable_map,
        planning: _,
        requests,
        native_abi,
    } = prepared;
    let module = input.module();
    let signatures = abi::plans::Signatures { requests, plans };
    let native_abi = native_abi.complete(module, &enums, &signatures);
    let function_signatures = signatures
        .requests
        .functions
        .iter()
        .map(|(&id, &request)| (id, signatures.plans.scoop(request).clone()))
        .collect::<HashMap<_, _>>();
    let externs::LoweredExternFunctions {
        functions: extern_functions,
        references: extern_function_refs,
        bridges: bridge_signatures,
    } = lower_extern_functions(
        module,
        &structs,
        &enums,
        &native_abi.native_externals,
        &signatures,
    )?;
    let globals::LoweredGlobals {
        storage: storage_globals,
        native: native_globals,
        bridges: native_global_bridges,
    } = lower_globals(
        GlobalLoweringInputs {
            context: &context,
            identity_roots: &identity_roots,
            module,
            selected_layout,
            structs: &structs,
            enums: &enums,
            string_globals: &string_global_map,
            native_externals: &native_abi.native_externals,
        },
        &mut globals,
    )?;
    let callable_bodies = input
        .materialization()
        .callable_roots()
        .iter()
        .map(|root| (root.function(), callable_body_identity(input, *root)))
        .collect::<HashMap<_, _>>();
    let mut local_function_identities = lir::LocalFunctionIdentities::default();
    let local_function_map = module
        .top_level
        .iter()
        .filter(|&&id| identity_roots.emits_function(id))
        .map(|&id| {
            assert!(
                callable_bodies.contains_key(&id),
                "every emitted function is selected by the MIR materialization plan"
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
            let gateway = lower_initialization_startup_gateway(
                unit.identity.id(),
                ensure,
                &function_signatures[&unit.ensure],
                signatures.plans.scoop(signatures.requests.startup),
            );
            (reference, gateway)
        })
        .collect::<Vec<_>>();
    let root_artifacts = match input.production().entry_bridge() {
        mir::EntryMirBridgeBranchV1::Library => None,
        mir::EntryMirBridgeBranchV1::Executable(bridge) => {
            let mir::MirOutput::Executable { entry, arguments } = module.output else {
                unreachable!("the sealed MIR input keeps output and entry proof aligned")
            };
            let arguments = match arguments {
                mir::ProgramArguments::Unused => production::RootArguments::Unused,
                mir::ProgramArguments::Local(builder) => {
                    let builder_signature = &function_signatures[&builder];
                    let lir::LocalFunctionRef::Managed(builder) = local_function_map[&builder]
                    else {
                        unreachable!("the core argument builder has managed effect")
                    };
                    production::RootArguments::Build(
                        lir::ManagedCallDestination::local(builder),
                        builder_signature,
                    )
                }
                mir::ProgramArguments::External(builder) => production::RootArguments::Build(
                    lir::ManagedCallDestination::external(external_callable_map[&builder]),
                    external_callables[external_callable_map[&builder]].signature(),
                ),
            };
            Some(lower_root_artifacts(
                module.cone,
                bridge.source(),
                production::RootMain {
                    target: local_function_map[&entry],
                    signature: &function_signatures[&entry],
                    arguments,
                },
                globals::static_storage_layout(
                    &context,
                    &identity_roots,
                    module,
                    &mir::Type::Any,
                    selected_layout,
                )?,
                &mut globals,
                &signatures,
            ))
        }
    };
    let (callback_bridges, callback_map) = lower_callback_bridges(
        module,
        &structs,
        &enums,
        &local_function_map,
        &external_callable_map,
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
        &external_callable_map,
        dependency_types,
    )?;
    let (arrays, array_type_map) = array_types(
        &context,
        &identity_roots,
        module,
        &enums,
        &type_descriptor_refs,
    )?;

    let mut lower_body = |body_identity, body, signature, known_receiver| {
        lower_function(
            &context,
            module.cone,
            module,
            body_identity,
            body,
            signature,
            known_receiver,
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
            &external_callable_map,
            &callback_map,
            &extern_functions,
            &extern_function_refs,
            &bridge_signatures,
            &signatures,
        )
    };
    let mut lowered_functions = module
        .top_level
        .iter()
        .filter(|&&id| identity_roots.emits_function(id))
        .map(|id| {
            lower_body(
                callable_bodies[id].clone(),
                &module.functions[*id],
                &function_signatures[id],
                module
                    .meta
                    .interface_adjusts
                    .iter()
                    .find(|adjust| adjust.function() == *id)
                    .map(|adjust| adjust.class()),
            )
        })
        .collect::<StorageResult<Vec<_>>>()?;
    let release_signature = signatures.plans.scoop(signatures.requests.release);
    let mut release_hooks = Arena::new();
    for (_, hook) in module.release_hooks.iter() {
        let owner_type = mir::Type::Class(hook.owner);
        if !identity_roots.materializes_type(&owner_type) {
            continue;
        }
        let identity = lir::CallableBodyIdentity::for_release_hook(
            exact_type_record(module, &owner_type).id(),
            &identity_roots.for_type(&owner_type),
        )
        .expect("an exact release owner derives its machine body identity");
        let body = lower_body(identity, &hook.code, release_signature, None)?;
        let code = safepoints::complete_function(
            &context,
            body,
            &structs,
            &enums,
            signatures.runtime(lir::RuntimeFunction::Managed(
                lir::ManagedRuntimeFunction::Safepoint,
            )),
        )?;
        release_hooks.alloc(lir::ReleaseHook {
            owner: type_descriptor_refs.for_type(&owner_type),
            code,
        });
    }
    let task_descriptor = || {
        let storage = module
            .meta
            .generated_exact_types
            .iter()
            .find_map(|entry| match entry.location() {
                mir::GeneratedExactTypeLocation::Context(storage)
                    if storage.role == mir::ContextStorageRole::Task =>
                {
                    Some(storage)
                }
                _ => None,
            })
            .expect("MIR gateways retain the actual core Task Context type");
        type_descriptor_refs.for_type(&mir::Type::Context(storage))
    };
    if let (Some(reference), Some(mut gateway)) = (root_gateway_ref, root_artifacts) {
        assert_eq!(
            reference.declaration().into_u32() as usize,
            lowered_functions.len(),
            "the root gateway reference must address its appended LIR function"
        );
        production::initialize_task(
            &mut gateway,
            task_descriptor(),
            signatures.runtime(lir::RuntimeFunction::Managed(
                lir::ManagedRuntimeFunction::ContextEnsureRoot,
            )),
        );
        lowered_functions.push(gateway);
    }
    for (reference, mut gateway) in startup_gateways {
        assert_eq!(
            reference.declaration().into_u32() as usize,
            lowered_functions.len(),
            "an initialization startup-gateway reference must address its appended LIR function"
        );
        production::initialize_task(
            &mut gateway,
            task_descriptor(),
            signatures.runtime(lir::RuntimeFunction::Managed(
                lir::ManagedRuntimeFunction::ContextEnsureRoot,
            )),
        );
        lowered_functions.push(gateway);
    }
    let functions = lowered_functions
        .into_iter()
        .map(|function| {
            safepoints::complete_function(
                &context,
                function,
                &structs,
                &enums,
                signatures.runtime(lir::RuntimeFunction::Managed(
                    lir::ManagedRuntimeFunction::Safepoint,
                )),
            )
        })
        .collect::<StorageResult<Vec<_>>>()?;

    let layouts = layouts(
        &context,
        &identity_roots,
        module,
        &enums,
        imported_runtime_string.is_none(),
    )?;
    let external_exact_types = external_type_descriptors
        .iter()
        .map(|(_, descriptor)| descriptor.target())
        .collect::<std::collections::HashSet<_>>();
    let module = lir::Module {
        cone: module.cone,
        globals,
        initialization_units,
        structs,
        enums,
        functions,
        release_hooks,
        extern_functions,
        native_globals,
        native_global_bridges,
        callback_bridges,
        foreign_callback_families,
        foreign_callback_bridges,
        output: match module.output {
            mir::MirOutput::Library => lir::LirOutput::Library,
            mir::MirOutput::Executable { entry, .. } => lir::LirOutput::Executable {
                entry: local_function_map[&entry],
            },
        },
        meta: lir::LirMeta {
            exact_types: materialized_exact_types(input, &external_exact_types),
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
    safepoints::validate_release_bodies(&module)?;
    Ok(LoweredModule { module })
}

fn materialized_exact_types(
    input: &mir::ConeMirInput,
    external: &std::collections::HashSet<scoop_identity::PersistentExactTypeId>,
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
        .filter(|root| !external.contains(&root.exact()))
        .map(|root| {
            let record = exact_type_record(module, root.ty());
            assert_eq!(record.id(), root.exact());
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

fn callable_body_identity(
    input: &mir::ConeMirInput,
    root: mir::CallableMaterializationRoot,
) -> lir::CallableBodyIdentity {
    match root.subject() {
        mir::CallableSignatureSubject::Strong(owner) => strong_callable_body_identity(owner),
        mir::CallableSignatureSubject::Odr(member) => {
            let record = input
                .module()
                .meta
                .source_callable_materializations
                .get(root.function())
                .and_then(|source| source.odr_member_record())
                .or_else(|| input.foundation().odr_member(member.member()))
                .expect("a validated ODR function retains its source or MIR member record");
            lir::CallableBodyIdentity::for_odr_member(record.key())
                .expect("a validated callable member has a canonical body identity")
        }
    }
}

pub(super) fn strong_callable_body_identity(
    owner: mir::CallableOwner,
) -> lir::CallableBodyIdentity {
    let identity = match owner {
        mir::CallableOwner::Function(id) => lir::CallableBodyIdentity::for_function(id),
        mir::CallableOwner::Constructor(id) => lir::CallableBodyIdentity::for_constructor(id),
        mir::CallableOwner::Accessor(id) => lir::CallableBodyIdentity::for_property_accessor(id),
        mir::CallableOwner::Generated(id) => lir::CallableBodyIdentity::for_generated_callable(id),
        mir::CallableOwner::GenericTemplate(_) | mir::CallableOwner::Application(_) => {
            unreachable!("a callable signature subject has a defining owner")
        }
    };
    identity.expect("validated callable-body subjects have canonical runtime identities")
}
