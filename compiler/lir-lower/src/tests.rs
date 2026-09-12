use super::*;

mod support;

use support::*;

use scoop_identity::{
    CallableMaterialization, CallableMaterializationContext, CallableTemplateOwner,
    CallbackApplicationKey, CallbackMode, CallbackParameterIndex, CallbackRegistrationKey,
    CanonicalIdentifier, CborIdentityRecord, ConeIdentity, CoreBuiltinNominal, DeclarationScope,
    DefinitionOwnerChain, Effect, ExactCallableSignature, ExactTypeKey, FieldIdentityKey,
    GeneratedCallableKey, InitializationUnitKey, LexicalCallableParent, LexicalCallableRole,
    PackagePath, PersistentCallbackApplicationId, PersistentExactTypeId, PersistentFieldId,
    PersistentFunctionId, PersistentPropertyId, PersistentTypeId, SignatureCallableShape,
    SignatureTypeKey, SourceCAbiFunctionSignature, SourceCAbiReturn, SourceDeclarationKey,
    SourceDeclarationSite, SourceExternFunctionAbi, SourceNativeExternalContract,
    SourceNativeExternalContractKey, SourceNativeExternalContractRecord,
    SourceNativeLibraryBinding, SourceNativeSymbol, SourceNominalKind,
    SourceScoopAbiFunctionSignature, StructuralDefinitionPath, StructuralDefinitionSiteRole,
    StructuralPathSegment,
};

fn test_field_identity(owner_name: &str, field_name: &str) -> PersistentFieldId {
    fn identifier(value: &str) -> CanonicalIdentifier {
        let encoded = format!(
            "test{}",
            value
                .bytes()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        );
        CanonicalIdentifier::new(&encoded).unwrap()
    }

    let owner = SourceDeclarationKey::nominal(
        SourceDeclarationSite::new(
            ConeIdentity::SINGLE_FILE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        identifier(owner_name),
        SourceNominalKind::Struct,
        0,
    );
    PersistentFieldId::from_key(
        &FieldIdentityKey::source_declared(&owner, identifier(field_name)).unwrap(),
    )
    .unwrap()
}

fn test_source_native_contract(
    source_name: &str,
    native_symbol: &str,
    abi: mir::ExternAbi,
) -> SourceNativeExternalContractRecord {
    let identifier = format!(
        "test{}",
        format!("{source_name}:{native_symbol}")
            .bytes()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    );
    let declaration = SourceDeclarationKey::function(
        SourceDeclarationSite::new(
            ConeIdentity::SINGLE_FILE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new(&identifier).unwrap(),
        0,
        None,
        Vec::new(),
    );
    let abi = match abi {
        mir::ExternAbi::C => SourceExternFunctionAbi::C(SourceCAbiFunctionSignature::new(
            Vec::new(),
            SourceCAbiReturn::Void,
        )),
        mir::ExternAbi::Scoop => SourceExternFunctionAbi::Scoop {
            signature: SourceScoopAbiFunctionSignature::new(
                Vec::new(),
                SignatureTypeKey::Nominal(CoreBuiltinNominal::Unit.identity_record().id()),
            ),
            gc_effect: scoop_identity::GcEffect::Managed,
        },
    };
    SourceNativeExternalContractRecord::new(
        SourceNativeExternalContractKey::function(&declaration).unwrap(),
        SourceNativeExternalContract::Function {
            symbol: SourceNativeSymbol::new(native_symbol).unwrap(),
            library: SourceNativeLibraryBinding::DefaultNativeNamespace,
            abi,
            calling_convention: scoop_identity::SourceCallingConvention::Cdecl,
        },
    )
    .unwrap()
}

fn test_source_native_data_contract(
    source_name: &str,
    native_symbol: &str,
    storage: SignatureTypeKey,
) -> SourceNativeExternalContractRecord {
    let identifier = format!(
        "test{}",
        source_name
            .bytes()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    );
    let declaration = SourceDeclarationKey::property(
        SourceDeclarationSite::new(
            ConeIdentity::SINGLE_FILE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new(&identifier).unwrap(),
    );
    SourceNativeExternalContractRecord::new(
        SourceNativeExternalContractKey::property(&declaration).unwrap(),
        SourceNativeExternalContract::ReadOnlyData {
            symbol: SourceNativeSymbol::new(native_symbol).unwrap(),
            library: SourceNativeLibraryBinding::DefaultNativeNamespace,
            storage,
        },
    )
    .unwrap()
}

fn lower(module: &mir::Module) -> lir::Module {
    super::lower(module, lir::LirTargetProfile::DARWIN_AARCH64)
}

fn register_test_source_exact_type(module: &mut mir::Module, ty: mir::Type) {
    let mut entries = module
        .meta
        .source_exact_types
        .iter()
        .cloned()
        .collect::<Vec<_>>();
    test_exact_type(&module.function_types, &ty, &mut entries);
    module.meta.source_exact_types = mir::SourceExactTypeIdentities::checked(entries).unwrap();
}

fn test_callable_body(symbol: &str) -> lir::CallableBodyIdentity {
    let identifier = format!(
        "test{}",
        symbol
            .as_bytes()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    );
    let site = scoop_identity::SourceDeclarationSite::new(
        scoop_identity::ConeIdentity::SINGLE_FILE,
        scoop_identity::PackagePath::root(),
        scoop_identity::DefinitionOwnerChain::top_level(),
        scoop_identity::DeclarationScope::ConeWide,
    )
    .unwrap();
    let declaration = scoop_identity::SourceDeclarationKey::function(
        site,
        scoop_identity::CanonicalIdentifier::new(&identifier).unwrap(),
        0,
        None,
        Vec::new(),
    );
    let function =
        scoop_identity::PersistentFunctionId::from_source_declaration(&declaration).unwrap();
    lir::CallableBodyIdentity::for_function(function).unwrap()
}

fn expected_callable_body(subject: mir::CallableSignatureSubject) -> lir::CallableBodyIdentity {
    match subject {
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
                panic!("test subject must name a concrete strong definition")
            }
        },
        mir::CallableSignatureSubject::Odr(member) => {
            lir::CallableBodyIdentity::for_odr_member(member)
        }
    }
    .unwrap()
}

fn callback_application()
-> CborIdentityRecord<PersistentCallbackApplicationId, CallbackApplicationKey> {
    let site = SourceDeclarationSite::new(
        ConeIdentity::CORE,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap();
    let function = PersistentFunctionId::from_source_declaration(&SourceDeclarationKey::function(
        site,
        CanonicalIdentifier::new("callbackOwner").unwrap(),
        0,
        None,
        Vec::new(),
    ))
    .unwrap();
    let unit = CoreBuiltinNominal::Unit.identity_record().id();
    let registration = CallbackRegistrationKey::new(
        LexicalCallableParent::function(function),
        StructuralDefinitionPath::from_first(
            StructuralPathSegment::new(StructuralDefinitionSiteRole::CallbackConversion, 0),
            [],
        ),
        SourceCAbiFunctionSignature::new(Vec::new(), SourceCAbiReturn::Void),
        CallbackParameterIndex::new(0),
        SignatureCallableShape::new(
            Effect::Ordinary,
            None,
            Vec::new(),
            SignatureTypeKey::Nominal(unit),
        ),
        CallbackMode::Reusable,
    );
    let application = CallbackApplicationKey::new(
        &registration,
        CallableMaterializationContext::NoSubstitution,
    )
    .unwrap();
    CborIdentityRecord::from_key(application).unwrap()
}

fn exact_callback_signature() -> ExactCallableSignature {
    let unit = CoreBuiltinNominal::Unit.identity_record().id();
    let unit = PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(unit)).unwrap();
    ExactCallableSignature::new(Effect::Ordinary, None, Vec::new(), unit)
}

fn register_boxed_source_nominal(
    module: &mut mir::Module,
    payload: mir::Type,
    class: mir::ClassId,
    name: &str,
    kind: SourceNominalKind,
) {
    let site = SourceDeclarationSite::new(
        ConeIdentity::SINGLE_FILE,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap();
    let declaration =
        SourceDeclarationKey::nominal(site, CanonicalIdentifier::new(name).unwrap(), kind, 0);
    let nominal = PersistentTypeId::from_source_declaration(&declaration).unwrap();
    let exact = CborIdentityRecord::from_key(ExactTypeKey::Nominal(nominal)).unwrap();

    let mut exact_types = module
        .meta
        .source_exact_types
        .iter()
        .filter(|entry| entry.ty() != &mir::Type::Class(class) && entry.ty() != &payload)
        .cloned()
        .collect::<Vec<_>>();
    exact_types
        .push(mir::SourceExactTypeIdentity::checked(payload.clone(), exact.clone(), None).unwrap());
    module.meta.source_exact_types = mir::SourceExactTypeIdentities::checked(exact_types).unwrap();
    module
        .meta
        .boxed_types
        .push(mir::BoxedType::for_source_nominal(payload, class, &exact).unwrap());
    install_generated_exact_types(module);
}

fn install_generated_exact_types(module: &mut mir::Module) {
    let mut entries = Vec::new();
    let mut register = |location, nominal, odr_member| {
        entries.push(mir::GeneratedExactTypeIdentity::new(location, nominal, odr_member).unwrap());
    };
    for environment in &module.meta.closure_environments {
        register(
            mir::GeneratedExactTypeLocation::Closure(environment.class()),
            environment.identity().generated_type_record(),
            environment.identity().odr_member_record(),
        );
    }
    for (_, adapter) in module.meta.closure_adapters.iter() {
        register(
            mir::GeneratedExactTypeLocation::Closure(adapter.class()),
            adapter.identity().environment_record(),
            Some(adapter.identity().environment_member_record()),
        );
    }
    for (_, adapter) in module.meta.dynamic_closure_adapters.iter() {
        register(
            mir::GeneratedExactTypeLocation::Closure(adapter.class()),
            adapter.identity().environment_record(),
            Some(adapter.identity().environment_member_record()),
        );
    }
    for (_, step) in module.meta.coroutine_steps.iter() {
        register(
            mir::GeneratedExactTypeLocation::Enum(step.enum_id()),
            step.identity().generated_type_record(),
            step.identity().root().member_record(),
        );
    }
    for (_, slot) in module.meta.coroutine_slots.iter() {
        register(
            mir::GeneratedExactTypeLocation::Enum(slot.enum_id()),
            slot.identity().generated_type_record(),
            slot.identity().root().member_record(),
        );
    }
    for (_, frame) in module.meta.coroutine_frames.iter() {
        register(
            mir::GeneratedExactTypeLocation::Class(frame.class()),
            frame.identity().generated_type_record(),
            frame.identity().odr_member_record(),
        );
    }
    for (_, point) in module.meta.coroutine_resume_points.iter() {
        register(
            mir::GeneratedExactTypeLocation::Class(point.adapter()),
            point.identity().generated_type_record(),
            point.identity().odr_member_record(),
        );
    }
    for boxed in &module.meta.boxed_types {
        register(
            mir::GeneratedExactTypeLocation::Class(boxed.class()),
            boxed.identity().generated_type_record(),
            boxed.identity().root().member_record(),
        );
    }
    module.meta.generated_exact_types =
        mir::GeneratedExactTypeIdentities::checked(entries).unwrap();
}

fn install_callable_signatures(module: &mut mir::Module) {
    let mut entries = module
        .meta
        .source_callable_materializations
        .iter()
        .map(|source| source.signature_record().clone())
        .collect::<Vec<_>>();
    entries.extend(module.foreign_callback_bridges.iter().map(|(_, bridge)| {
        mir::CallableSignatureRecord::new(
            bridge.application_record.managed_adapter(),
            bridge.application_record.managed_signature().clone(),
        )
    }));
    module.meta.callable_signatures = mir::MirCallableSignatures::checked(entries).unwrap();
}

fn initialization_unit_identity() -> mir::InitializationUnitIdentityRecord {
    let site = SourceDeclarationSite::new(
        ConeIdentity::SINGLE_FILE,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap();
    let property =
        SourceDeclarationKey::property(site, CanonicalIdentifier::new("initializedValue").unwrap());
    let property = PersistentPropertyId::from_source_declaration(&property).unwrap();
    CborIdentityRecord::from_key(InitializationUnitKey::TopLevelProperty(property)).unwrap()
}

#[test]
fn selected_target_profile_is_embedded_in_lir_meta() {
    let mut builder = Builder::new();
    let main = builder.main(Arena::new(), Vec::new());
    let module = lower(&builder.finish(main));

    assert_eq!(
        module.meta.target_profile,
        lir::LirTargetProfile::DARWIN_AARCH64
    );
}

#[test]
fn initialization_display_name_survives_lir_lowering() {
    let mut builder = Builder::new();
    let main = builder.main(Arena::new(), Vec::new());
    let mut source = builder.finish(main);
    let unit_identity = initialization_unit_identity();
    let persistent_unit = unit_identity.id();
    let storage_owner = property_owner("value");
    let storage = source.globals.alloc(mir::Global {
        name: "value".to_string(),
        storage_owner: mir::StaticStorageOwner::PropertyBacking(storage_owner),
        ty: mir::Type::Unit,
        mutable: false,
        storage: mir::GlobalStorage::Managed {
            initial_state: mir::MirStaticInitialState::ZeroedForRuntimeUnit,
        },
    });
    let failure_global = source.globals.alloc(mir::Global {
        name: "$init.failure".to_string(),
        storage_owner: mir::StaticStorageOwner::InitializationFailureRoot(persistent_unit),
        ty: mir::Type::String,
        mutable: true,
        storage: mir::GlobalStorage::Managed {
            initial_state: mir::MirStaticInitialState::ZeroedForRuntimeUnit,
        },
    });
    let unit_id = mir::InitializationUnitId::from_raw(0_u32.into());
    let failure_root = source
        .initialization_failure_roots
        .alloc(mir::InitializationFailureRoot {
            global: failure_global,
        });
    let unit = source.initialization_units.alloc(mir::InitializationUnit {
        identity: unit_identity,
        display_name: "top-level:value".to_string(),
        schedule: mir::InitializationSchedule::EagerStartup,
        kind: mir::InitializationUnitKind::EagerTopLevel { storage },
        initializer: main,
        ensure: main,
        failure_root,
        dependencies: Vec::new(),
        cycle_exception: mir::MessageClassConstructor {
            class: mir::ClassId::from_raw(0_u32.into()),
            initializer: main,
            message_type: mir::Type::String,
        },
    });
    assert_eq!(unit, unit_id);

    let module = lower(&source);
    let lowered_unit =
        &module.initialization_units[lir::InitializationUnitId::from_raw(unit_id.into_raw())];
    assert_eq!(lowered_unit.display_name, "top-level:value");
    assert_eq!(
        lowered_unit.identity,
        source.initialization_units[unit].identity
    );
    let lir::GlobalInit::Storage {
        identity: storage_identity,
        ..
    } = &module.globals[lowered_unit.kind.storage()].init
    else {
        panic!("initialization storage must remain a storage global")
    };
    assert_eq!(
        storage_identity,
        &lir::StaticStorageIdentity::static_place_for_property(
            storage_owner,
            lir::MaterializationRoot::cone_owned(),
        )
        .unwrap()
    );
    let lir::GlobalInit::Storage {
        identity: failure_identity,
        ..
    } = &module.globals[lowered_unit.failure_root].init
    else {
        panic!("initialization failure root must remain a storage global")
    };
    assert_eq!(
        failure_identity,
        &lir::StaticStorageIdentity::initialization_failure_root(
            persistent_unit,
            lir::MaterializationRoot::cone_owned(),
        )
        .unwrap()
    );
}

#[test]
fn nominal_descriptor_symbols_use_exact_type_identity() {
    let mut builder = Builder::new();
    builder.class("Same", None, &[], Vec::new(), Vec::new());
    builder.class("Same", None, &[], Vec::new(), Vec::new());
    builder.interface("View", &[]);
    builder.interface("View", &[]);
    let main = builder.main(Arena::new(), Vec::new());
    let mut source = builder.finish(main);
    let function_type = source.function_types.alloc(mir::FunctionType {
        is_suspend: false,
        parameter_types: Vec::new(),
        return_type: mir::Type::Unit,
    });
    register_test_source_exact_type(&mut source, mir::Type::Function(function_type));
    let site = SourceDeclarationSite::new(
        ConeIdentity::SINGLE_FILE,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap();
    let owner = PersistentFunctionId::from_source_declaration(&SourceDeclarationKey::function(
        site,
        CanonicalIdentifier::new("descriptorClosureOwner").unwrap(),
        0,
        None,
        Vec::new(),
    ))
    .unwrap();
    let mut source_callables = source
        .meta
        .source_callable_materializations
        .iter()
        .cloned()
        .collect::<Vec<_>>();
    for index in 0..2 {
        let callable = CborIdentityRecord::from_key(GeneratedCallableKey::Lexical {
            parent: LexicalCallableParent::function(owner),
            role: LexicalCallableRole::LambdaBody,
            path: StructuralDefinitionPath::from_first(
                StructuralPathSegment::new(
                    StructuralDefinitionSiteRole::Lambda,
                    u32::try_from(index).unwrap(),
                ),
                [],
            ),
        })
        .unwrap()
        .id();
        let materialization = CallableMaterialization::new(
            CallableTemplateOwner::Generated(callable),
            CallableMaterializationContext::NoSubstitution,
        );
        let invoke_function = source.functions.alloc(mir::Function {
            gc_effect: mir::GcEffect::Managed,
            name: format!("$closure{index}.invoke"),
            params: Vec::new(),
            return_ty: mir::Type::Unit,
            body: mir::Body::unreachable(Arena::new()),
        });
        source.top_level.push(invoke_function);
        source_callables.push(
            mir::SourceCallableMaterialization::new(
                invoke_function,
                materialization,
                exact_callback_signature(),
                None,
            )
            .unwrap(),
        );
        let invoke = source
            .closure_invoke_functions
            .alloc(mir::ClosureInvokeFunction {
                function: invoke_function,
            });
        let class = source.closure_classes.alloc(mir::ClosureClass {
            name: "SameClosure".to_string(),
            function_type,
            invoke,
            captures: Vec::new(),
            bridges: Vec::new(),
        });
        let identity =
            mir::ClosureEnvironmentIdentity::for_lambda(materialization, Vec::new(), None).unwrap();
        source.meta.closure_environments.push(
            mir::ClosureEnvironment::checked(class, &source.closure_classes[class], identity)
                .unwrap(),
        );
    }
    source.meta.source_callable_materializations =
        mir::SourceCallableMaterializations::checked(source_callables).unwrap();
    install_generated_exact_types(&mut source);
    install_callable_signatures(&mut source);
    let module = lower(&source);

    let descriptors = module
        .meta
        .type_descriptors
        .iter()
        .filter(|(_, descriptor)| {
            matches!(descriptor.name.as_str(), "Same" | "View" | "SameClosure")
        })
        .map(|(_, descriptor)| descriptor)
        .collect::<Vec<_>>();
    assert_eq!(descriptors.len(), 6);
    let symbols = descriptors
        .iter()
        .map(|descriptor| descriptor.identity.symbol())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(symbols.len(), 6);
    for descriptor in descriptors {
        assert_eq!(
            descriptor.identity.symbol_request().unwrap().key(),
            scoop_identity::PersistentSymbolKey::TypeDescriptor(descriptor.identity.exact_type())
        );
    }
}

#[test]
fn lowering_context_derives_scalar_pointer_and_runtime_prefix_layouts() {
    let profile = lir::LirTargetProfile::DARWIN_AARCH64;
    let context = LoweringContext::new(profile);
    let physical = |layout: lir::ScalarLayout| PhysicalLayout {
        size: layout.size_bytes(),
        align: layout.alignment_bytes(),
    };

    assert_eq!(
        context.scalar_layout(lir::BackendScalarKind::I1),
        physical(profile.scalar_layout(lir::BackendScalarKind::I1))
    );
    for kind in lir::IntegerKind::ALL {
        assert_eq!(
            context.integer_layout(kind),
            physical(profile.scalar_layout(kind.width().backend_scalar_kind()))
        );
    }
    assert_eq!(
        context.machine_scalar_layout(),
        physical(profile.scalar_layout(lir::BackendScalarKind::I64))
    );
    for kind in [
        lir::PointerKind::Managed,
        lir::PointerKind::Raw,
        lir::PointerKind::Code,
        lir::PointerKind::Metadata,
    ] {
        assert_eq!(
            context.pointer_layout(kind),
            physical(profile.pointer_layout(kind))
        );
    }

    let i64_layout = context.scalar_layout(lir::BackendScalarKind::I64);
    let metadata_pointer = context.pointer_layout(lir::PointerKind::Metadata);
    let (header_offsets, expected_header) =
        context.aggregate_layout([metadata_pointer, i64_layout]);
    assert_eq!(context.object_header_layout(), expected_header);
    assert_eq!(context.object_type_descriptor_offset(), header_offsets[0]);

    let (_, expected_string) = context.aggregate_layout([expected_header, i64_layout]);
    assert_eq!(context.string_layout(), expected_string);

    let code_pointer = context.pointer_layout(lir::PointerKind::Code);
    let (closure_offsets, expected_closure) =
        context.aggregate_layout([expected_header, code_pointer]);
    assert_eq!(
        context.closure_prefix(),
        (closure_offsets[1], expected_closure)
    );

    let raw_pointer = context.pointer_layout(lir::PointerKind::Raw);
    assert_eq!(
        context.closure_invoke_dispatch_slot(),
        u32::try_from(closure_offsets[1] / raw_pointer.size).expect("test slot fits u32")
    );
    let i32_layout = context.scalar_layout(lir::BackendScalarKind::I32);
    let (_, expected_exception) = context.aggregate_layout([raw_pointer, i32_layout]);
    assert_eq!(context.exception_record_layout(), expected_exception);

    let (descriptor_offsets, _) = context.aggregate_layout([
        i64_layout,
        i64_layout,
        i64_layout,
        metadata_pointer,
        metadata_pointer,
        metadata_pointer,
    ]);
    assert_eq!(
        context.type_descriptor_vtable_offset(),
        descriptor_offsets[5]
    );
}

#[test]
fn profile_layout_drives_aggregate_root_scan_offsets() {
    let context = LoweringContext::new(lir::LirTargetProfile::DARWIN_AARCH64);
    let structs = lir::StructDefs::default();
    let enums = lir::EnumDefs::default();
    let ty = lir::LirType::Aggregate(vec![lir::LirType::I1, lir::MANAGED_PTR]);
    let bool_layout = context.scalar_layout(lir::BackendScalarKind::I1);
    let pointer_layout = context.pointer_layout(lir::PointerKind::Managed);
    let (offsets, expected) = context.aggregate_layout([bool_layout, pointer_layout]);

    assert_eq!(
        safepoints::lir_size_align(&context, &ty, &structs, &enums),
        (expected.size, expected.align)
    );
    assert_eq!(
        safepoints::root_scan(&context, &ty, &structs, &enums, 0),
        lir::RefScan::References(vec![offsets[1]])
    );
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

#[test]
fn c_abi_preserves_all_eight_exact_integer_kinds() {
    let mut builder = Builder::new();
    let params = mir::IntegerKind::ALL
        .map(mir::Type::Integer)
        .into_iter()
        .collect::<Vec<_>>();
    builder.extern_functions.alloc(mir::ExternFunction {
        source_contract: test_source_native_contract("integers", "integers", mir::ExternAbi::C),
        source_name: "integers".to_string(),
        native_symbol: "integers".to_string(),
        library: String::new(),
        abi: mir::ExternAbi::C,
        calling_convention: mir::CallingConvention::Cdecl,
        gc_effect: mir::GcEffect::NoGc,
        params: params.clone(),
        return_type: mir::Type::Integer(mir::IntegerKind::UNSIGNED_64),
    });
    builder.c_extern(
        "sameIntegers",
        "same_integers",
        params,
        mir::Type::Integer(mir::IntegerKind::UNSIGNED_64),
    );
    let main = builder.main(Arena::new(), Vec::new());
    let mut mir_module = builder.finish(main);
    let native_signature = mir_module.function_types.alloc(mir::FunctionType {
        is_suspend: false,
        parameter_types: mir::IntegerKind::ALL
            .map(mir::Type::Integer)
            .into_iter()
            .collect(),
        return_type: mir::Type::Integer(mir::IntegerKind::UNSIGNED_64),
    });
    let forbidden_descriptor_name = format!(
        "function<{}>",
        mir::type_name(&mir_module, &mir::Type::Function(native_signature))
    );
    let module = lower(&mir_module);
    let (_, function) = module
        .extern_functions
        .iter()
        .next()
        .expect("at least one C extern");
    let lir::ExternFunctionKind::C { signature, .. } = &function.kind else {
        panic!("C declaration remains a C bridge")
    };
    assert_eq!(
        &signature.params,
        &lir::IntegerKind::ALL
            .map(lir::CType::Integer)
            .into_iter()
            .collect::<Vec<_>>()
    );
    assert_eq!(
        signature.return_type,
        lir::CReturnType::Value(Box::new(
            lir::CType::Integer(lir::IntegerKind::UNSIGNED_64,)
        ))
    );
    assert_eq!(
        signature.storage_params(),
        [
            lir::LirType::I8,
            lir::LirType::I16,
            lir::LirType::I32,
            lir::LirType::I64,
            lir::LirType::I8,
            lir::LirType::I16,
            lir::LirType::I32,
            lir::LirType::I64,
        ]
    );
    assert!(
        descriptor_values(&module).all(|descriptor| descriptor.name != forbidden_descriptor_name),
        "a native FunPtr signature must not fabricate a managed TypeDescriptor"
    );

    let expected_parameters = mir::IntegerKind::ALL
        .into_iter()
        .map(|kind| {
            let ty = mir::Type::Integer(kind);
            let exact_type = mir_module
                .meta
                .source_exact_types
                .get(&ty)
                .expect("every source integer has an exact identity")
                .identity_record()
                .id();
            let storage = scoop_identity::CanonicalCStorageType::Integer {
                exact_type,
                signedness: match kind.signedness() {
                    mir::IntegerSignedness::Signed => scoop_identity::Signedness::Signed,
                    mir::IntegerSignedness::Unsigned => scoop_identity::Signedness::Unsigned,
                },
                bit_width: match kind.width() {
                    mir::IntegerWidth::W8 => scoop_identity::IntegerBitWidth::Bits8,
                    mir::IntegerWidth::W16 => scoop_identity::IntegerBitWidth::Bits16,
                    mir::IntegerWidth::W32 => scoop_identity::IntegerBitWidth::Bits32,
                    mir::IntegerWidth::W64 => scoop_identity::IntegerBitWidth::Bits64,
                },
            };
            scoop_identity::CanonicalCAbiParameter::new(exact_type, storage).unwrap()
        })
        .collect();
    let return_type = mir::Type::Integer(mir::IntegerKind::UNSIGNED_64);
    let return_exact = mir_module
        .meta
        .source_exact_types
        .get(&return_type)
        .expect("the source return type has an exact identity")
        .identity_record()
        .id();
    let expected_signature = scoop_identity::CanonicalCAbiSignatureFingerprintRecord::new(
        scoop_identity::CanonicalCAbiFunctionSignature::cdecl(
            expected_parameters,
            scoop_identity::CanonicalCAbiReturn::value(
                return_exact,
                scoop_identity::CanonicalCStorageType::Integer {
                    exact_type: return_exact,
                    signedness: scoop_identity::Signedness::Unsigned,
                    bit_width: scoop_identity::IntegerBitWidth::Bits64,
                },
            )
            .unwrap(),
        ),
    )
    .unwrap();
    assert_eq!(
        module.meta.canonical_c_abi.signatures(),
        &[expected_signature]
    );
    assert!(module.meta.canonical_c_abi.layouts().is_empty());
}

#[test]
fn canonical_c_abi_metadata_handles_struct_function_pointer_recursion() {
    let mut builder = Builder::new();
    let node_id = mir::StructId::from_raw(
        u32::try_from(builder.structs.len())
            .expect("test struct arena length fits u32")
            .into(),
    );
    let handler_signature = builder.function_types.alloc(mir::FunctionType {
        is_suspend: false,
        parameter_types: vec![mir::Type::Struct(node_id)],
        return_type: mir::Type::Unit,
    });
    let handler_type = mir::Type::FunPtr(handler_signature);
    let node = builder.c_strukt(
        "Node",
        mir::MirCLayoutValue::Natural,
        mir::MirCLayoutValue::Natural,
        false,
        &[("handler", handler_type.clone())],
    );
    assert_eq!(node, node_id);
    builder.c_extern(
        "visitNode",
        "visit_node",
        vec![mir::Type::Struct(node)],
        mir::Type::Unit,
    );
    let main = builder.main(Arena::new(), Vec::new());
    let mut source = builder.finish(main);
    register_test_source_exact_type(&mut source, handler_type.clone());
    let node_exact = source
        .meta
        .source_exact_types
        .get(&mir::Type::Struct(node))
        .expect("Node has an exact identity")
        .identity_record()
        .id();
    let handler_exact = source
        .meta
        .source_exact_types
        .get(&handler_type)
        .expect("Node handler has an exact identity")
        .identity_record()
        .id();

    let module = lower(&source);
    let expected_layout = scoop_identity::CanonicalCAbiLayoutFingerprintRecord::new(
        scoop_identity::CanonicalCAbiLayout::new(
            node_exact,
            8,
            std::num::NonZeroU64::new(8).unwrap(),
            scoop_identity::CLayoutOverride::Natural,
            scoop_identity::CLayoutOverride::Natural,
            vec![scoop_identity::CanonicalCAbiLayoutField::new(
                test_field_identity("Node", "handler"),
                0,
                scoop_identity::CanonicalCStorageType::CodePointer {
                    exact_type: handler_exact,
                    storage: scoop_identity::CPointerStorage::Direct,
                },
            )],
        ),
    )
    .unwrap();
    let expected_signature = scoop_identity::CanonicalCAbiSignatureFingerprintRecord::new(
        scoop_identity::CanonicalCAbiFunctionSignature::cdecl(
            vec![
                scoop_identity::CanonicalCAbiParameter::new(
                    node_exact,
                    scoop_identity::CanonicalCStorageType::Struct {
                        exact_type: node_exact,
                        layout: expected_layout.fingerprint(),
                    },
                )
                .unwrap(),
            ],
            scoop_identity::CanonicalCAbiReturn::Void,
        ),
    )
    .unwrap();

    assert_eq!(module.meta.canonical_c_abi.layouts(), &[expected_layout]);
    assert_eq!(
        module.meta.canonical_c_abi.signatures(),
        &[expected_signature]
    );
    let foundation = lir::CanonicalLirFoundation::from_module(&module).unwrap();
    assert_eq!(foundation.counts().c_abi_layouts, 1);
    assert_eq!(foundation.counts().c_abi_signatures, 1);
}

#[test]
fn canonical_c_abi_metadata_includes_external_global_layouts() {
    let mut builder = Builder::new();
    let header = builder.c_strukt(
        "Header",
        mir::MirCLayoutValue::A8,
        mir::MirCLayoutValue::A1,
        false,
        &[("flag", mir::Type::Boolean), ("value", INT)],
    );
    let main = builder.main(Arena::new(), Vec::new());
    let mut source = builder.finish(main);
    let exact = source
        .meta
        .source_exact_types
        .get(&mir::Type::Struct(header))
        .expect("Header has an exact identity")
        .identity_record();
    let ExactTypeKey::Nominal(nominal) = exact.key() else {
        panic!("a non-generic test struct has a nominal exact identity")
    };
    source.globals.alloc(mir::Global {
        name: "header".to_string(),
        storage_owner: mir::StaticStorageOwner::PropertyBacking(property_owner("header")),
        ty: mir::Type::Struct(header),
        mutable: false,
        storage: mir::GlobalStorage::Extern {
            source_contract: Box::new(test_source_native_data_contract(
                "header",
                "native_header",
                SignatureTypeKey::Nominal(*nominal),
            )),
            library: String::new(),
            native_symbol: "native_header".to_string(),
            thread_local: false,
        },
    });

    let module = lower(&source);
    assert!(module.meta.canonical_c_abi.signatures().is_empty());
    assert_eq!(module.meta.canonical_c_abi.layouts().len(), 1);
    assert_eq!(
        lir::CanonicalLirFoundation::from_module(&module)
            .unwrap()
            .counts()
            .c_abi_layouts,
        1
    );
}

#[test]
fn c_abi_nullable_refs_bind_the_exact_lowered_pointee_and_signature() {
    let mut builder = Builder::new();
    let raw_payload = mir::Type::Ptr(Box::new(INT));
    let raw_option = builder.option_enum("Option<Ptr<Int>>", raw_payload.clone());
    let native_signature = builder.function_types.alloc(mir::FunctionType {
        is_suspend: false,
        parameter_types: vec![mir::Type::Integer(mir::IntegerKind::UNSIGNED_16)],
        return_type: mir::Type::Integer(mir::IntegerKind::SIGNED_8),
    });
    let code_payload = mir::Type::FunPtr(native_signature);
    let code_option = builder.option_enum("Option<FunPtr>", code_payload.clone());
    builder.extern_functions.alloc(mir::ExternFunction {
        source_contract: test_source_native_contract(
            "nullablePointers",
            "nullable_pointers",
            mir::ExternAbi::C,
        ),
        source_name: "nullablePointers".to_string(),
        native_symbol: "nullable_pointers".to_string(),
        library: String::new(),
        abi: mir::ExternAbi::C,
        calling_convention: mir::CallingConvention::Cdecl,
        gc_effect: mir::GcEffect::NoGc,
        params: vec![
            mir::Type::Enum(raw_option, vec![raw_payload]),
            mir::Type::Enum(code_option, vec![code_payload]),
        ],
        return_type: mir::Type::Unit,
    });
    let main = builder.main(Arena::new(), Vec::new());

    let module = lower(&builder.finish(main));
    let (_, function) = module.extern_functions.iter().next().expect("one C extern");
    let lir::ExternFunctionKind::C { signature, .. } = &function.kind else {
        panic!("C declaration remains a C bridge")
    };
    let lir::CType::DataPointer {
        pointee,
        storage: lir::CDataPointerStorage::Nullable(raw_reference),
    } = &signature.params[0]
    else {
        panic!("first parameter is an exact nullable data pointer")
    };
    assert_eq!(pointee, raw_reference.pointee());
    assert_eq!(
        module
            .enums
            .nullable_data_pointer_binding(raw_reference.definition()),
        Some(raw_reference.pointee())
    );
    let lir::CType::CodePointer {
        signature: code_signature,
        storage: lir::CCodePointerStorage::Nullable(code_reference),
    } = &signature.params[1]
    else {
        panic!("second parameter is an exact nullable code pointer")
    };
    assert_eq!(code_signature.as_ref(), code_reference.signature());
    assert_eq!(
        module
            .enums
            .nullable_code_pointer_binding(code_reference.definition()),
        Some(code_reference.signature())
    );
    let dump = lir::dump(&module);
    assert!(dump.contains("data-ptr<Int,nullable=enum0<Int>>"), "{dump}");
    assert!(
        dump.contains("code-ptr<(UInt16)->Int8,nullable=enum1<(UInt16)->Int8>>"),
        "{dump}"
    );
}

#[test]
#[should_panic(expected = "HIR C-FFI classification rejects")]
fn c_abi_does_not_guess_nullable_pointer_from_a_non_option_enum_shape() {
    let mut builder = Builder::new();
    let payload = mir::Type::Ptr(Box::new(INT));
    let lookalike = builder.enums.alloc(mir::EnumDef {
        name: "LooksLikeOption".to_string(),
        type_arguments: Vec::new(),
        gc_free: true,
        variants: vec![
            mir::VariantDef {
                name: "Some".to_string(),
                gc_free: true,
                fields: vec![mir::Field {
                    name: "_1".to_string(),
                    ty: payload.clone(),
                }],
            },
            mir::VariantDef {
                name: "None".to_string(),
                gc_free: true,
                fields: Vec::new(),
            },
        ],
    });
    builder.extern_functions.alloc(mir::ExternFunction {
        source_contract: test_source_native_contract("lookalike", "lookalike", mir::ExternAbi::C),
        source_name: "lookalike".to_string(),
        native_symbol: "lookalike".to_string(),
        library: String::new(),
        abi: mir::ExternAbi::C,
        calling_convention: mir::CallingConvention::Cdecl,
        gc_effect: mir::GcEffect::NoGc,
        params: vec![mir::Type::Enum(lookalike, vec![payload])],
        return_type: mir::Type::Unit,
    });
    let main = builder.main(Arena::new(), Vec::new());

    let _ = lower(&builder.finish(main));
}

#[test]
fn foreign_callback_bridge_preserves_its_nominal_family() {
    let mut builder = Builder::new();
    let callback = builder.structs.alloc(mir::StructDef {
        type_arguments: Vec::new(),
        name: "ForeignCallback<(Int) -> Unit>".to_string(),
        gc_free: true,
        representation: mir::StructRepresentation::Declared {
            c_layout: None,
            interior_mutable: false,
            fields: vec![
                mir::DeclaredStructField {
                    identity: test_field_identity("Lookalike", "function"),
                    name: "function".to_string(),
                    ty: mir::Type::FunPtr(mir::FunctionTypeId::from_raw(0.into())),
                },
                mir::DeclaredStructField {
                    identity: test_field_identity("Lookalike", "context"),
                    name: "context".to_string(),
                    ty: mir::Type::Ptr(Box::new(mir::Type::Unit)),
                },
            ],
        },
    });
    let unit_variant = |name: &str| mir::VariantDef {
        name: name.to_string(),
        gc_free: true,
        fields: Vec::new(),
    };
    let mode = builder.enums.alloc(mir::EnumDef {
        name: "ForeignCallbackMode".to_string(),
        type_arguments: Vec::new(),
        gc_free: true,
        variants: ["Reusable", "OneShot"].map(unit_variant).into(),
    });
    let state = builder.enums.alloc(mir::EnumDef {
        name: "ForeignCallbackState".to_string(),
        type_arguments: Vec::new(),
        gc_free: true,
        variants: ["Registered", "Active", "Completed", "Failed"]
            .map(unit_variant)
            .into(),
    });
    let throwable = builder.class("Throwable", None, &[], Vec::new(), Vec::new());
    let failure = builder.option_enum("Option<Throwable>", mir::Type::Class(throwable));
    let modes = mir::ForeignCallbackModes::checked(
        &builder.enums,
        mir::MirVariantRef::new(&builder.enums, mode, 0).expect("Reusable"),
        mir::MirVariantRef::new(&builder.enums, mode, 1).expect("OneShot"),
    )
    .expect("callback modes");
    let states = mir::ForeignCallbackStates::checked(
        &builder.enums,
        mir::MirVariantRef::new(&builder.enums, state, 0).expect("Registered"),
        mir::MirVariantRef::new(&builder.enums, state, 1).expect("Active"),
        mir::MirVariantRef::new(&builder.enums, state, 2).expect("Completed"),
        mir::MirVariantRef::new(&builder.enums, state, 3).expect("Failed"),
    )
    .expect("callback states");
    let failure_result = mir::ForeignCallbackFailureResult::checked(
        &builder.enums,
        *builder.option_core.last().expect("failure Option metadata"),
        throwable,
    )
    .expect("callback failure result");
    let main = builder.main(Arena::new(), Vec::new());
    let mut module = builder.finish(main);
    let native_signature = module.function_types.alloc(mir::FunctionType {
        is_suspend: false,
        parameter_types: vec![mir::Type::Ptr(Box::new(mir::Type::Unit))],
        return_type: mir::Type::Unit,
    });
    let managed_signature = module.function_types.alloc(mir::FunctionType {
        is_suspend: false,
        parameter_types: Vec::new(),
        return_type: mir::Type::Unit,
    });
    let family = module
        .foreign_callback_families
        .alloc(mir::ForeignCallbackFamily {
            callback,
            modes,
            states,
            failure_result,
        });
    let application_identity = callback_application();
    let application = application_identity.id();
    let exact_signature = exact_callback_signature();
    let adapter_function = module.functions.alloc(mir::Function {
        gc_effect: mir::GcEffect::Managed,
        name: "foreign callback adapter".to_string(),
        params: Vec::new(),
        return_ty: mir::Type::Unit,
        body: mir::Body::unreachable(Arena::new()),
    });
    module.top_level.push(adapter_function);
    let adapter = module.foreign_callback_adapters.alloc(
        mir::ForeignCallbackAdapter::checked(
            adapter_function,
            managed_signature,
            &module.function_types[managed_signature],
            application,
            &exact_signature,
            None,
        )
        .unwrap(),
    );
    module
        .foreign_callback_bridges
        .alloc(mir::ForeignCallbackBridge {
            application_identity,
            application_record: mir::CallbackApplicationRecord::new(
                application,
                module.foreign_callback_adapters[adapter].signature_subject(),
                exact_signature,
                mir::ForeignCallbackStorageAbi::ClosureResultRootsThrowableToU32,
                CallbackMode::Reusable,
            ),
            adapter,
            family,
            native_signature,
            context_index: 0,
            mode: modes.reusable(),
        });
    module.meta.generated_callables =
        mir::MirGeneratedCallableIdentities::checked(vec![mir::MirGeneratedCallableIdentity::new(
            adapter_function,
            module.foreign_callback_adapters[adapter].identity_record(),
            module.foreign_callback_adapters[adapter].signature_subject(),
        )])
        .unwrap();
    register_test_source_exact_type(&mut module, mir::Type::Ptr(Box::new(mir::Type::Unit)));
    install_callable_signatures(&mut module);

    let lowered = lower(&module);
    assert_eq!(
        lowered
            .foreign_callback_bridges
            .iter()
            .next()
            .expect("the callback bridge is retained")
            .1
            .application,
        application
    );
    let lowered_family = lowered.foreign_callback_families.iter().next().unwrap().1;
    assert_eq!(lowered_family.callback.into_raw(), callback.into_raw());
    assert_eq!(
        lowered_family.states.definition().into_raw(),
        state.into_raw()
    );
    assert_eq!(
        lowered_family.failure_result.definition().into_raw(),
        failure.into_raw()
    );
    assert_eq!(
        lowered_family.modes.reusable().definition().into_raw(),
        mode.into_raw()
    );
    assert_eq!(
        lowered
            .foreign_callback_bridges
            .iter()
            .next()
            .unwrap()
            .1
            .family,
        scoop_lir::ForeignCallbackFamilyId::from_raw(family.into_raw())
    );
}

mod arrays;
mod basics;
mod enums;
mod exceptions;
mod functions;
mod objects;
mod traps;
