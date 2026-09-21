use super::*;

#[test]
fn core_lowering_publishes_initialization_protocol_abi() {
    let mut builder = Builder::new();
    let function = builder.user_fn("exported", Arena::new(), Vec::new());
    let cycle_function =
        builder.user_fn("__scoopThrowInitializationCycle", Arena::new(), Vec::new());
    let mut module = builder.finish(function);
    module.cone = scoop_identity::ConeIdentity::CORE;
    module.output = mir::MirOutput::Library;
    let mir::CallableSignatureSubject::Strong(cycle_implementation) = module
        .meta
        .callable_signature_subject(cycle_function)
        .expect("test cycle function has a strong callable subject")
    else {
        panic!("test cycle function cannot have ODR ownership")
    };
    let scoop_identity::CallableOwner::Function(cycle_definition) = cycle_implementation else {
        panic!("test cycle function has a function owner")
    };
    let foundation = mir::OdrFreeMirFoundation::from_module(&module).unwrap();
    let strong = mir::StrongCallableBridgeSurfaceV1::from_odr_free_foundation(&foundation);
    let exact = strong
        .bridges()
        .iter()
        .find(|bridge| bridge.implementation() == cycle_implementation)
        .unwrap()
        .signature()
        .clone();
    let core = mir::CoreMirBridgeV1::new(
        mir::CoreMirInitializationCycleThrowerV1::new(cycle_definition, cycle_implementation)
            .unwrap(),
    );
    let production = mir::CoreBootstrapBridgeSectionV1::try_new(
        ConeIdentity::CORE,
        mir::CoreMirBridgeBranchV1::Core(core),
        mir::EntryMirBridgeBranchV1::Library,
        strong,
    )
    .unwrap();
    let input = mir::SingleConeStrongMirInput::try_new(
        module,
        foundation,
        production,
        Vec::new(),
        mir::StrongImportedCoreInput::Unused,
    )
    .unwrap();

    let output = crate::lower(
        &input,
        crate::StrongImportedCoreLirInput::Unused,
        lir::LirTargetProfile::DARWIN_AARCH64,
    )
    .unwrap();
    let lir::CoreLirBridgeBranchV1::Core(core) = output.core_lir_bridge() else {
        panic!("core lowering must publish the core LIR branch")
    };
    let callable = core.initialization_cycle_thrower();
    assert_eq!(callable.abi_signature().signature(), &exact);
    assert_eq!(
        callable.root_plan(),
        lir::CoreExternalCallableRootPlan::ManagedStatepoint
    );
    assert_eq!(
        callable.expected_symbol(),
        output.module().functions[1].callable_body.symbol_request()
    );
}

#[test]
fn ordinary_lowering_materializes_and_calls_the_initialization_protocol() {
    let mut core_builder = Builder::new();
    let mut core_locals = Arena::new();
    let core_parameter = core_locals.alloc(local("value", mir::Type::String));
    let core_function = core_builder.user_fn_full(
        "__scoopThrowInitializationCycle",
        vec![mir::Param {
            name: "value".to_string(),
            ty: mir::Type::String,
            local: core_parameter,
        }],
        mir::Type::Unit,
        core_locals,
        Vec::new(),
    );
    let mut core_module = core_builder.finish(core_function);
    core_module.cone = ConeIdentity::CORE;
    core_module.output = mir::MirOutput::Library;
    let mir::CallableSignatureSubject::Strong(implementation) = core_module
        .meta
        .callable_signature_subject(core_function)
        .unwrap()
    else {
        panic!("test core callable must have strong ownership")
    };
    let scoop_identity::CallableOwner::Function(definition) = implementation else {
        panic!("test core callable must be a source function")
    };
    let core_foundation = mir::OdrFreeMirFoundation::from_module(&core_module).unwrap();
    let strong = mir::StrongCallableBridgeSurfaceV1::from_odr_free_foundation(&core_foundation);
    let exact = strong
        .bridges()
        .iter()
        .find(|bridge| bridge.implementation() == implementation)
        .unwrap()
        .signature()
        .clone();
    let core_production = mir::CoreBootstrapBridgeSectionV1::try_new(
        ConeIdentity::CORE,
        mir::CoreMirBridgeBranchV1::Core(mir::CoreMirBridgeV1::new(
            mir::CoreMirInitializationCycleThrowerV1::new(definition, implementation).unwrap(),
        )),
        mir::EntryMirBridgeBranchV1::Library,
        strong,
    )
    .unwrap();
    let core_input = mir::SingleConeStrongMirInput::try_new(
        core_module,
        core_foundation.clone(),
        core_production,
        Vec::new(),
        mir::StrongImportedCoreInput::Unused,
    )
    .unwrap();
    let core_lir = crate::lower(
        &core_input,
        crate::StrongImportedCoreLirInput::Unused,
        lir::LirTargetProfile::DARWIN_AARCH64,
    )
    .unwrap();

    let mut pending = PendingIdentityValidation::new();
    pending.register_authority(ConeIdentity::CORE).unwrap();
    let identities = pending.finish().unwrap();
    let mut session = SemanticIdentitySession::new();
    let (_, imported_mir_identities, _) = session
        .import(
            ConeIdentity::CORE,
            SemanticOriginFingerprint::new([1; 32], [2; 32], [3; 32]),
            &identities,
        )
        .unwrap()
        .into_parts();
    let imported_mir =
        mir::ImportedMirFoundation::from_odr_free(core_foundation, imported_mir_identities);
    let mir_callable = imported_mir
        .project_initialization_cycle_thrower(core_input.production(), definition, exact.clone())
        .unwrap();
    let mut selected_mir = mir::SelectedImportedMirSet::new(&imported_mir, core_input.production());
    let mir_selection = selected_mir.insert(mir_callable).unwrap();

    let mut ordinary_builder = Builder::new();
    let mut caller_locals = Arena::new();
    let caller_argument = caller_locals.alloc(local("message", mir::Type::String));
    let caller = ordinary_builder.user_fn_full(
        "caller",
        vec![param("message", mir::Type::String, caller_argument)],
        mir::Type::Unit,
        caller_locals,
        Vec::new(),
    );
    let mut ordinary_module = ordinary_builder.finish(caller);
    ordinary_module.output = mir::MirOutput::Library;
    let imported_use = ordinary_module.meta.imported_core_callables.alloc(
        selected_mir
            .callable_use(mir_selection)
            .expect("selected MIR callable mints one use"),
    );
    let entry = ordinary_module.functions[caller].body.entry;
    ordinary_module.functions[caller].body.blocks[entry]
        .statements
        .push(call_stmt(mir::Call {
            target: mir::CallTarget {
                kind: mir::CallKind::Direct,
                callee: mir::Callee::CoreExternal(imported_use),
            },
            args: vec![local_expr(caller_argument, mir::Type::String)],
            pending: mir::CoroutinePendingContext::Root,
        }));
    let ordinary_foundation = mir::OdrFreeMirFoundation::from_module(&ordinary_module).unwrap();
    let ordinary_production = mir::CoreBootstrapBridgeSectionV1::try_new(
        ConeIdentity::SINGLE_FILE,
        mir::CoreMirBridgeBranchV1::NotCore,
        mir::EntryMirBridgeBranchV1::Library,
        mir::StrongCallableBridgeSurfaceV1::from_odr_free_foundation(&ordinary_foundation),
    )
    .unwrap();
    let ordinary_input = mir::SingleConeStrongMirInput::try_new(
        ordinary_module,
        ordinary_foundation,
        ordinary_production,
        Vec::new(),
        mir::StrongImportedCoreInput::Selected(&selected_mir),
    )
    .unwrap();

    let canonical_lir = core_lir.foundation().as_canonical().clone();
    let decoded_lir = scoop_wire::decode_canonical::<lir::DecodedLirFoundation>(
        &scoop_wire::encode(&canonical_lir).unwrap(),
        scoop_wire::DecodeLimits::default(),
    )
    .unwrap();
    let mut pending = PendingIdentityValidation::new();
    pending.register_authority(ConeIdentity::CORE).unwrap();
    pending.register_authority(definition).unwrap();
    let decoded_exact_types = core_input
        .module()
        .meta
        .source_exact_types
        .iter()
        .map(|identity| {
            scoop_wire::decode_canonical::<
                scoop_identity::DecodedCborIdentityRecord<
                    scoop_identity::PersistentExactTypeId,
                    scoop_identity::DecodedExactTypeKey,
                >,
            >(
                &scoop_wire::encode(identity.identity_record()).unwrap(),
                scoop_wire::DecodeLimits::default(),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    for identity in core_input.module().meta.source_exact_types.iter() {
        match identity.identity_record().key() {
            scoop_identity::ExactTypeKey::Nominal(owner) => {
                pending.register_authority(*owner).unwrap();
            }
            scoop_identity::ExactTypeKey::NominalApplication { origin, .. } => {
                pending.register_authority(*origin).unwrap();
            }
            scoop_identity::ExactTypeKey::Tuple(_)
            | scoop_identity::ExactTypeKey::Function { .. }
            | scoop_identity::ExactTypeKey::RawPointer(_)
            | scoop_identity::ExactTypeKey::NativeFunctionPointer { .. } => {}
        }
    }
    for identity in &decoded_exact_types {
        pending
            .register(scoop_identity::IdentityLayer::Hir, identity)
            .unwrap();
    }
    decoded_lir.register_identities(&mut pending).unwrap();
    for identity in &decoded_exact_types {
        pending.resolve(identity).unwrap();
    }
    decoded_lir.resolve_identities(&mut pending).unwrap();
    let identities = pending.finish().unwrap();
    let mut session = SemanticIdentitySession::new();
    let (_, _, imported_lir_identities) = session
        .import(
            ConeIdentity::CORE,
            SemanticOriginFingerprint::new([4; 32], [5; 32], [6; 32]),
            &identities,
        )
        .unwrap()
        .into_parts();
    let imported_lir = lir::ImportedLirFoundation::from_odr_free(
        core_lir.foundation().clone(),
        imported_lir_identities,
    );
    let definitions =
        lir::StrongObjectSymbolSurfaceV1::from_odr_free_foundation(core_lir.foundation()).unwrap();
    let core_bridge = core_lir.core_lir_bridge().core().unwrap();
    let lir_callable = imported_lir
        .project_initialization_cycle_thrower(
            core_bridge,
            &definitions,
            scoop_identity::StrongCallableDefinitionOwner::Function(definition),
            exact,
        )
        .unwrap();
    let string_exact = core_input
        .module()
        .meta
        .source_exact_types
        .iter()
        .find(|identity| identity.ty() == &mir::Type::String)
        .unwrap()
        .identity_record()
        .id();
    let runtime_string = imported_lir
        .project_type_descriptor(&definitions, string_exact)
        .unwrap();
    let mut selected_lir = lir::SelectedImportedLirSet::try_new(
        &imported_lir,
        &definitions,
        core_bridge,
        runtime_string,
    )
    .unwrap();
    selected_lir.insert(lir_callable).unwrap();

    assert!(matches!(
        crate::lower(
            &ordinary_input,
            crate::StrongImportedCoreLirInput::Unused,
            lir::LirTargetProfile::DARWIN_AARCH64,
        ),
        Err(crate::StrongLirLoweringError::MissingImportedCoreLirAuthority)
    ));
    let output = crate::lower(
        &ordinary_input,
        crate::StrongImportedCoreLirInput::Selected(&selected_lir),
        lir::LirTargetProfile::DARWIN_AARCH64,
    )
    .unwrap();
    let module = output.module();
    assert_eq!(module.meta.core_external_callables.len(), 1);
    assert_eq!(module.meta.external_type_descriptors.len(), 1);
    let string_descriptor = &module
        .meta
        .external_type_descriptors
        .iter()
        .next()
        .unwrap()
        .1;
    assert_eq!(string_descriptor.target(), string_exact);
    assert!(matches!(
        module.meta.well_known_type_descriptors.string,
        lir::TypeDescriptorRef::External(_)
    ));
    assert!(
        module
            .meta
            .type_descriptors
            .iter()
            .all(|(_, descriptor)| descriptor.identity.exact_type() != string_exact)
    );
    assert!(
        module.meta.layouts.iter().all(|(_, layout)| {
            layout.identity.layout_record().key().exact_type() != string_exact
        })
    );
    let external = &module.meta.core_external_callables.iter().next().unwrap().1;
    assert_eq!(external.signature().logical_argument_count(), 1);
    assert_eq!(
        external.target(),
        scoop_identity::StrongCallableDefinitionOwner::Function(definition)
    );
    assert_eq!(
        external.root_plan(),
        lir::CoreExternalCallableRootPlan::ManagedStatepoint
    );
    let call = module.functions[0].blocks[module.functions[0].entry]
        .instructions
        .iter()
        .find_map(|instruction| match instruction {
            lir::Instruction::Call { site } => Some(site),
            _ => None,
        })
        .expect("ordinary caller emits the imported core call");
    assert!(matches!(
        call.destination(&module.functions[0].call_targets),
        lir::CallDestination::CoreExternal(_)
    ));
    let production = output
        .build_production_section(
            scoop_identity::ConeCoordinate::reserved_single_file(),
            lir::EntryProductionSourceV1::Library,
        )
        .unwrap();
    assert_eq!(production.external_bridges().bridges().len(), 2);
    assert_eq!(
        production
            .external_bridges()
            .bridges()
            .iter()
            .filter(|bridge| matches!(bridge, lir::StrongExternalLirBridgeV1::Callable(_)))
            .count(),
        1
    );
    assert!(
        production
            .external_bridges()
            .bridges()
            .iter()
            .any(|bridge| {
                matches!(
                    bridge,
                    lir::StrongExternalLirBridgeV1::TypeDescriptor(descriptor)
                        if descriptor.target() == string_exact
                )
            })
    );
}
