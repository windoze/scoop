use super::*;

#[test]
fn core_lowering_publishes_callable_abi_authority() {
    let mut builder = Builder::new();
    let function = builder.user_fn("exported", Arena::new(), Vec::new());
    let cycle_function =
        builder.user_fn("__scoopThrowInitializationCycle", Arena::new(), Vec::new());
    let mut module = builder.finish(function);
    module.cone = scoop_identity::ConeIdentity::CORE;
    module.output = mir::MirOutput::Library;
    let mir::CallableSignatureSubject::Strong(implementation) = module
        .meta
        .callable_signature_subject(function)
        .expect("test function has a strong callable subject")
    else {
        panic!("test function cannot have ODR ownership")
    };
    let scoop_identity::CallableOwner::Function(definition) = implementation else {
        panic!("test source function has a function owner")
    };
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
    let declaration = SourceDeclarationKey::function(
        SourceDeclarationSite::new(
            ConeIdentity::SINGLE_FILE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new("main").unwrap(),
        0,
        None,
        Vec::new(),
    );
    let binding = scoop_identity::PersistentExportBindingId::from_key(
        &scoop_identity::ExportBindingKey::new(
            ConeIdentity::CORE,
            PackagePath::root(),
            CanonicalIdentifier::new("exported").unwrap(),
            scoop_identity::BindingTarget::function(&declaration).unwrap(),
        ),
    )
    .unwrap();
    let foundation = mir::OdrFreeMirFoundation::from_module(&module).unwrap();
    let strong = mir::StrongCallableBridgeSurfaceV1::from_odr_free_foundation(&foundation);
    let exact = strong
        .bridges()
        .iter()
        .find(|bridge| bridge.implementation() == implementation)
        .unwrap()
        .signature()
        .clone();
    let core = mir::CoreMirBridgeV1::try_new(
        vec![mir::CoreMirCallableBridgeV1::new(binding, definition, implementation).unwrap()],
        Vec::new(),
        mir::CoreMirInitializationCycleThrowerV1::new(cycle_definition, cycle_implementation)
            .unwrap(),
    )
    .unwrap();
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
        mir::CoreShapeSupportSourceInput::Core(Vec::new()),
        mir::StrongImportedCoreInput::Unused,
    )
    .unwrap();

    let output = super::super::lower(
        &input,
        super::super::StrongImportedCoreLirInput::Unused,
        lir::LirTargetProfile::DARWIN_AARCH64,
    )
    .unwrap();
    let lir::CoreLirBridgeBranchV1::Core(core) = output.core_lir_bridge() else {
        panic!("core lowering must publish the core LIR branch")
    };
    let [callable] = core.callables() else {
        panic!("test core publishes exactly one callable")
    };
    assert_eq!(callable.binding(), binding);
    assert_eq!(callable.abi_signature().signature(), &exact);
    assert_eq!(
        callable.root_plan(),
        lir::CoreExternalCallableRootPlan::ManagedStatepoint
    );
    assert_eq!(
        callable.expected_symbol(),
        output.module().functions[0].callable_body.symbol_request()
    );
}

#[test]
fn ordinary_lowering_materializes_and_calls_the_selected_core_callable() {
    let mut core_builder = Builder::new();
    let mut core_locals = Arena::new();
    let core_parameter = core_locals.alloc(local("value", INT));
    let core_function = core_builder.user_fn_full(
        "exported",
        vec![mir::Param {
            name: "value".to_string(),
            ty: INT,
            local: core_parameter,
        }],
        mir::Type::Unit,
        core_locals,
        Vec::new(),
    );
    let cycle_function =
        core_builder.user_fn("__scoopThrowInitializationCycle", Arena::new(), Vec::new());
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
    let mir::CallableSignatureSubject::Strong(cycle_implementation) = core_module
        .meta
        .callable_signature_subject(cycle_function)
        .expect("test cycle function has a strong callable subject")
    else {
        panic!("test cycle function cannot have ODR ownership")
    };
    let scoop_identity::CallableOwner::Function(cycle_definition) = cycle_implementation else {
        panic!("test cycle function has a function owner")
    };
    let declaration = SourceDeclarationKey::function(
        SourceDeclarationSite::new(
            ConeIdentity::CORE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new("exported").unwrap(),
        0,
        None,
        Vec::new(),
    );
    let binding = scoop_identity::PersistentExportBindingId::from_key(
        &scoop_identity::ExportBindingKey::new(
            ConeIdentity::CORE,
            PackagePath::root(),
            CanonicalIdentifier::new("exported").unwrap(),
            scoop_identity::BindingTarget::function(&declaration).unwrap(),
        ),
    )
    .unwrap();
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
        mir::CoreMirBridgeBranchV1::Core(
            mir::CoreMirBridgeV1::try_new(
                vec![
                    mir::CoreMirCallableBridgeV1::new(binding, definition, implementation).unwrap(),
                ],
                Vec::new(),
                mir::CoreMirInitializationCycleThrowerV1::new(
                    cycle_definition,
                    cycle_implementation,
                )
                .unwrap(),
            )
            .unwrap(),
        ),
        mir::EntryMirBridgeBranchV1::Library,
        strong,
    )
    .unwrap();
    let core_input = mir::SingleConeStrongMirInput::try_new(
        core_module,
        core_foundation.clone(),
        core_production,
        mir::CoreShapeSupportSourceInput::Core(Vec::new()),
        mir::StrongImportedCoreInput::Unused,
    )
    .unwrap();
    let core_lir = super::super::lower(
        &core_input,
        super::super::StrongImportedCoreLirInput::Unused,
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
        .project_core_callable(core_input.production(), binding, definition, exact.clone())
        .unwrap();
    let mut selected_mir = mir::SelectedImportedMirSet::new(&imported_mir, core_input.production());
    let mir_selection = selected_mir.insert(mir_callable).unwrap();

    let mut ordinary_builder = Builder::new();
    let caller = ordinary_builder.user_fn("caller", Arena::new(), Vec::new());
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
            args: vec![int_expr(7)],
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
        mir::CoreShapeSupportSourceInput::NotCore,
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
    pending.register_authority(cycle_definition).unwrap();
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
        .project_core_callable(
            core_bridge,
            &definitions,
            binding,
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
        .project_core_type_descriptor(&definitions, string_exact)
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
        super::super::lower(
            &ordinary_input,
            super::super::StrongImportedCoreLirInput::Unused,
            lir::LirTargetProfile::DARWIN_AARCH64,
        ),
        Err(super::super::StrongLirLoweringError::MissingImportedCoreLirAuthority)
    ));
    let output = super::super::lower(
        &ordinary_input,
        super::super::StrongImportedCoreLirInput::Selected(&selected_lir),
        lir::LirTargetProfile::DARWIN_AARCH64,
    )
    .unwrap();
    let module = output.module();
    assert_eq!(module.meta.core_external_callables.len(), 1);
    assert_eq!(module.meta.core_external_type_descriptors.len(), 1);
    let string_descriptor = &module
        .meta
        .core_external_type_descriptors
        .iter()
        .next()
        .unwrap()
        .1;
    assert_eq!(string_descriptor.target(), string_exact);
    assert!(matches!(
        module.meta.well_known_type_descriptors.string,
        lir::TypeDescriptorRef::CoreExternal(_)
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

#[test]
fn reachable_structural_function_descriptor_reports_strong_capability_error() {
    let mut b = Builder::new();
    let function_type = b.function_types.alloc(mir::FunctionType {
        is_suspend: false,
        parameter_types: Vec::new(),
        return_type: mir::Type::Unit,
    });
    let mut locals = Arena::new();
    let callable = locals.alloc(local("callable", mir::Type::Any));
    let main = b.main(
        locals,
        vec![call_stmt(mir::Call {
            target: mir::CallTarget {
                kind: mir::CallKind::FunctionBridge { function_type },
                callee: mir::Callee::FunctionBridge(function_type),
            },
            args: vec![local_expr(callable, mir::Type::Any)],
            pending: mir::CoroutinePendingContext::Root,
        })],
    );
    let mut source = b.finish(main);
    register_test_source_exact_type(&mut source, mir::Type::Function(function_type));

    let error = match try_lower(source) {
        Err(error) => error,
        Ok(_) => panic!("reachable function bridge must fail strong LIR capability validation"),
    };
    assert!(
        error
            .to_string()
            .starts_with(StrongLirCapabilityError::CODE)
    );
    match error {
        StrongLirLoweringError::Capability(error) => {
            assert_eq!(error.function(), main);
            assert_eq!(
                error.requirement(),
                &StrongLirMaterializationRequirement::TypeDescriptor(mir::Type::Function(
                    function_type
                ))
            );
        }
        StrongLirLoweringError::Output(lir::SingleConeStrongLirOutputError::Foundation(_)) => {
            panic!("capability validation must run before LIR foundation projection")
        }
        StrongLirLoweringError::Output(lir::SingleConeStrongLirOutputError::CoreShapeSupport(
            _,
        )) => panic!("a non-core capability fixture cannot enter core shape sealing"),
        other => panic!("unexpected strong lowering error: {other}"),
    }
}

#[test]
fn function_signatures_params_and_calls() {
    let mut b = Builder::new();
    // fun add(x: Int, y: Int): Int { return x + y }
    let mut locals = Arena::new();
    let x = locals.alloc(local("x", INT));
    let y = locals.alloc(local("y", INT));
    let add = b.user_fn_body(
        "add",
        vec![param("x", INT, x), param("y", INT, y)],
        INT,
        returning_body(
            locals,
            integer_binary_expr(
                mir::IntegerKind::SIGNED_32,
                mir::IntegerBinaryOperator::Add,
                local_expr(x, INT),
                local_expr(y, INT),
            ),
        ),
    );
    // main: val r = add(40, 2)
    let mut main_locals = Arena::new();
    let r = main_locals.alloc(local("r", INT));
    let main = b.main(
        main_locals,
        vec![call_value(
            r,
            mir::Call {
                target: mir::CallTarget {
                    kind: mir::CallKind::Direct,
                    callee: mir::Callee::User(add),
                },
                args: vec![int_expr(40), int_expr(2)],
                pending: mir::CoroutinePendingContext::Root,
            },
        )],
    );
    let module = lower(b.finish(main));

    // Parameters are SSA values (`Value::Param`), not stack slots;
    // the add body has no locals at all.
    let add_fn = &module.functions[0];
    assert_eq!(
        add_fn
            .signature
            .arguments()
            .iter()
            .map(lir::AbiArgument::logical_storage_type)
            .collect::<Vec<_>>(),
        [&lir::LirType::I32, &lir::LirType::I32]
    );
    assert_eq!(
        add_fn.signature.result().logical_storage_type(),
        Some(&lir::LirType::I32)
    );
    assert_eq!(add_fn.locals.len(), 0);

    insta::assert_snapshot!(lir::dump(&module), @r###"
Module
  global @scoop$1$ss$229a4d048049cf9bf3e032011c7d4e6761bc12c77fae79ba745ea06c32b07585 : ptr<managed> scan=refs[0]
  fun @scoop$1$cb$f7aa0e16d7e2d04ad4b1959f084e8eb868250c67e42ec11715a06a727f8ef34e(i32, i32) -> i32
  block entry
    poll managed-void-target0 sp<managed-poll:0> live=[]
    t0 = integer_Add<Int> param0, param1 : i32
    ret t0
  fun @scoop$1$cb$231a9ff4d6fc765297e8eb2c6cee080892fcc69d9b541b4356dd49d5e5726fde() -> void
    local %0 r: i32
  block entry
    poll managed-void-target0 sp<managed-poll:0> live=[]
    call managed-direct-target0 sp<managed-call:0> live=[] t0 = sig=direct0 (i32, i32) -> i32 local-fn0(integer<Int>(0x00000028), integer<Int>(0x00000002))
    store t0 -> local0
    ret
  fun @scoop$1$cb$35c3dc5c3c3d7d1d3b6d2a47d7e6d6c88d61bca0e08966efecf4802178cdefa3() -> i32
  block entry
    poll managed-void-target1 sp<managed-poll:0> live=[]
    invoke managed-void-target0 sp<managed-invoke:0> roots=[] sig=void0 () local-fn1() normal @success unwind @failure
    br @success
  block success
    ret integer<UInt>(0x00000000)
  block failure
    (t0, t1) = landingpad : (exception_record, ptr<raw>)
    t2 = begin_catch t1 : ptr<managed>
    global_store global0, t2
    end_catch
    ret integer<UInt>(0x00000001)
  td td0 Unit @scoop$1$td$1dff58a7007c61d14decc85852d44e40d113b26e96ec4d24b365bcde341966dc type-id=15768153469707105389 shape=BoxedValue minimum-size=16 align=8 parent=none vtable=[] itables=[]
  td td1 ULong @scoop$1$td$6540713f4816f1b567f9b6748e3a56db61b978601d8b31e9ddb964c4defb6f04 type-id=1551972451261988531 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td2 Int16 @scoop$1$td$6847006b21faa1b2f6581e828d7316cdcb56ea55d63fad2d5ab4d54fbc66a67d type-id=6090757864100470475 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td3 Int @scoop$1$td$6b87a07c3203f405ad126d1a0a8d440a3e0dea6bc0395d44602821b3a87e5816 type-id=6878802435704108962 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td4 Int8 @scoop$1$td$8750f2c8970ee21c9e4c352b0ced3fe3646c8e13c7a58abdec7eb93f11a041b3 type-id=3127261975970956121 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td5 UInt16 @scoop$1$td$8c2572d704dc526f384ed644ae8c20af6bfa9ee6051e9d089b44e82e2479b7e3 type-id=15604079800532685352 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td6 Boolean @scoop$1$td$c5593913e1722c44bbd16b5ba20bb09da93de51ddba97509748063fd2731db5e type-id=2212946439315248882 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td7 UInt @scoop$1$td$cd33e50d4bee20d1122a80e678258fafccbdcf60a258a56f92d698b61932d841 type-id=18175881444594673019 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td8 UInt8 @scoop$1$td$e9b2707b5c4d75570191bbd4adbfff0c67aeef329cffb1987b73a4d7e813681e type-id=16653769684987306371 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td9 Long @scoop$1$td$ecd8b585ebc7fc3d76d9765f2fe1d8dec433276d11f6de158399c5b02e14f55c type-id=3262026339401001817 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  layout Int8 size=1 align=1 refs=[]
  layout Int16 size=2 align=2 refs=[]
  layout Int size=4 align=4 refs=[]
  layout Long size=8 align=8 refs=[]
  layout UInt8 size=1 align=1 refs=[]
  layout UInt16 size=2 align=2 refs=[]
  layout UInt size=4 align=4 refs=[]
  layout ULong size=8 align=8 refs=[]
  layout Boolean size=1 align=1 refs=[]
  layout Unit size=0 align=1 refs=[]
  output executable @scoop$1$cb$231a9ff4d6fc765297e8eb2c6cee080892fcc69d9b541b4356dd49d5e5726fde
"###);
}

#[test]
fn c_extern_arguments_keep_their_exact_backing_storage_in_lir() {
    let mut b = Builder::new();
    let int8 = mir::Type::Integer(mir::IntegerKind::SIGNED_8);
    let consume = b.c_extern(
        "consumeInt8",
        "native_consume_int8",
        vec![int8.clone()],
        mir::Type::Unit,
    );
    let main = b.main(
        Arena::new(),
        vec![call_stmt(extern_call(
            consume,
            vec![integer_expr(mir::IntegerKind::SIGNED_8, 7)],
        ))],
    );
    let module = lower(b.finish(main));

    let function = &module.functions[0];
    let instructions = instructions_without_polls(&function.blocks[function.entry]);
    let local = instructions
        .iter()
        .find_map(|instruction| match instruction {
            lir::Instruction::Store { local, .. } => Some(*local),
            _ => None,
        })
        .expect("C argument is stored in one exact typed local");
    let arguments = instructions
        .iter()
        .find_map(|instruction| match instruction {
            lir::Instruction::Call {
                site: lir::CallSite::NativeSafe(site),
            } => Some(site.call.args()),
            _ => None,
        })
        .expect("C extern uses the native-safe protocol");
    assert_eq!(
        arguments,
        [lir::AbiCallArgument::Direct(lir::Value::CArgumentStorage(
            lir::CArgumentStorage::address_of(local)
        ))]
    );
    assert_eq!(function.locals[local].ty(), &lir::LirType::I8);
    assert!(lir::dump(&module).contains("extern0(c-arg-address(local0))"));
    assert!(
        !instructions
            .iter()
            .any(|instruction| matches!(instruction, lir::Instruction::LocalAddress { .. }))
    );
}

#[test]
fn return_inside_a_branch_seals_its_block() {
    // fun f(x: Int): Int { if (true) { return x }; return 0 }
    let mut b = Builder::new();
    let mut locals = Arena::new();
    let x = locals.alloc(local("x", INT));
    let mut blocks = Arena::new();
    let entry = cfg_block(&mut blocks, "entry");
    let then_block = cfg_block(&mut blocks, "if.then.1");
    let merge = cfg_block(&mut blocks, "if.merge.2");
    set_cfg_block(
        &mut blocks,
        entry,
        Vec::new(),
        mir::Terminator::Branch {
            cond: mir::Expr::bool(true),
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
            value: Some(local_expr(x, INT)),
        },
        None,
    );
    set_cfg_block(
        &mut blocks,
        merge,
        Vec::new(),
        mir::Terminator::Return {
            value: Some(int_expr(0)),
        },
        None,
    );
    let f = b.user_fn_body(
        "f",
        vec![param("x", INT, x)],
        INT,
        mir::Body {
            locals,
            blocks,
            entry,
            loop_header_polls: Vec::new(),
        },
    );
    let _ = f;
    let main = b.main(Arena::new(), vec![]);
    let module = lower(b.finish(main));

    // The constant branch is folded and its unreachable merge path is
    // removed; the `return` seals the remaining then block.
    insta::assert_snapshot!(lir::dump(&module), @r###"
Module
  global @scoop$1$ss$229a4d048049cf9bf3e032011c7d4e6761bc12c77fae79ba745ea06c32b07585 : ptr<managed> scan=refs[0]
  fun @scoop$1$cb$f7aa0e16d7e2d04ad4b1959f084e8eb868250c67e42ec11715a06a727f8ef34e(i32) -> i32
  block entry
    poll managed-void-target0 sp<managed-poll:0> live=[]
    br @if.then.1
  block if.then.1
    ret param0
  fun @scoop$1$cb$231a9ff4d6fc765297e8eb2c6cee080892fcc69d9b541b4356dd49d5e5726fde() -> void
  block entry
    poll managed-void-target0 sp<managed-poll:0> live=[]
    ret
  fun @scoop$1$cb$35c3dc5c3c3d7d1d3b6d2a47d7e6d6c88d61bca0e08966efecf4802178cdefa3() -> i32
  block entry
    poll managed-void-target1 sp<managed-poll:0> live=[]
    invoke managed-void-target0 sp<managed-invoke:0> roots=[] sig=void0 () local-fn1() normal @success unwind @failure
    br @success
  block success
    ret integer<UInt>(0x00000000)
  block failure
    (t0, t1) = landingpad : (exception_record, ptr<raw>)
    t2 = begin_catch t1 : ptr<managed>
    global_store global0, t2
    end_catch
    ret integer<UInt>(0x00000001)
  td td0 Unit @scoop$1$td$1dff58a7007c61d14decc85852d44e40d113b26e96ec4d24b365bcde341966dc type-id=15768153469707105389 shape=BoxedValue minimum-size=16 align=8 parent=none vtable=[] itables=[]
  td td1 ULong @scoop$1$td$6540713f4816f1b567f9b6748e3a56db61b978601d8b31e9ddb964c4defb6f04 type-id=1551972451261988531 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td2 Int16 @scoop$1$td$6847006b21faa1b2f6581e828d7316cdcb56ea55d63fad2d5ab4d54fbc66a67d type-id=6090757864100470475 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td3 Int @scoop$1$td$6b87a07c3203f405ad126d1a0a8d440a3e0dea6bc0395d44602821b3a87e5816 type-id=6878802435704108962 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td4 Int8 @scoop$1$td$8750f2c8970ee21c9e4c352b0ced3fe3646c8e13c7a58abdec7eb93f11a041b3 type-id=3127261975970956121 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td5 UInt16 @scoop$1$td$8c2572d704dc526f384ed644ae8c20af6bfa9ee6051e9d089b44e82e2479b7e3 type-id=15604079800532685352 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td6 Boolean @scoop$1$td$c5593913e1722c44bbd16b5ba20bb09da93de51ddba97509748063fd2731db5e type-id=2212946439315248882 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td7 UInt @scoop$1$td$cd33e50d4bee20d1122a80e678258fafccbdcf60a258a56f92d698b61932d841 type-id=18175881444594673019 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td8 UInt8 @scoop$1$td$e9b2707b5c4d75570191bbd4adbfff0c67aeef329cffb1987b73a4d7e813681e type-id=16653769684987306371 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  td td9 Long @scoop$1$td$ecd8b585ebc7fc3d76d9765f2fe1d8dec433276d11f6de158399c5b02e14f55c type-id=3262026339401001817 shape=BoxedValue minimum-size=24 align=8 parent=none vtable=[] itables=[]
  layout Int8 size=1 align=1 refs=[]
  layout Int16 size=2 align=2 refs=[]
  layout Int size=4 align=4 refs=[]
  layout Long size=8 align=8 refs=[]
  layout UInt8 size=1 align=1 refs=[]
  layout UInt16 size=2 align=2 refs=[]
  layout UInt size=4 align=4 refs=[]
  layout ULong size=8 align=8 refs=[]
  layout Boolean size=1 align=1 refs=[]
  layout Unit size=0 align=1 refs=[]
  output executable @scoop$1$cb$231a9ff4d6fc765297e8eb2c6cee080892fcc69d9b541b4356dd49d5e5726fde
"###);
}
