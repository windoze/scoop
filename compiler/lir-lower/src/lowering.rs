//! Shared physical lowering before profile-specific finalization and sealing.

use super::*;

pub(super) struct LoweredModule {
    pub module: lir::Module,
}

pub(super) fn lower_graph(
    input: &mir::SingleConeStrongMirInput,
    external_descriptors: &[lir::ExternalTypeDescriptor],
    selected_callables: &lir::SelectedExternalLirSet,
    target_profile: lir::LirTargetProfile,
    selected_layout: Option<&lir::StrongProductionDependencySelectionV2<'_>>,
) -> Result<LoweredModule, StrongLirLoweringError> {
    let module = input.module();
    module
        .validate()
        .unwrap_or_else(|error| panic!("invalid MIR input to lir-lower: {error}"));
    let context = LoweringContext::new(target_profile);
    let identity_roots = IdentityRoots::new(input);
    let mut external_type_descriptors = external_descriptors.iter().copied().collect();
    let dependency_types = dependency_types::lower(
        input,
        target_profile,
        selected_layout,
        &mut external_type_descriptors,
    )?;
    let imported_runtime_string = dependency_types
        .source
        .iter()
        .find_map(|(ty, reference)| (*ty == mir::Type::String).then_some(*reference));
    validate_strong_materialization(input, &identity_roots, &dependency_types)
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
    let (external_callables, external_callable_map) = lower_external_callables(
        &context,
        input,
        selected_callables,
        selected_layout,
        &structs,
        &enums,
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
        dependency_types,
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
                &external_callable_map,
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
    Ok(LoweredModule { module })
}

fn materialized_exact_types(
    input: &mir::SingleConeStrongMirInput,
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

pub(super) fn callable_body_identity(owner: mir::CallableOwner) -> lir::CallableBodyIdentity {
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
