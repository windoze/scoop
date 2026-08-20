use super::*;

pub(super) fn semantic_module(lazy: bool) -> Module {
    let unit_key = if lazy {
        InitializationUnitKey::Object(nominal("Singleton", SourceNominalKind::Object))
    } else {
        InitializationUnitKey::TopLevelProperty(property("value"))
    };
    let unit_identity = CborIdentityRecord::from_key(unit_key.clone()).unwrap();
    let unit_id = unit_identity.id();
    let storage_identity = match unit_key {
        InitializationUnitKey::TopLevelProperty(property) => {
            StaticStorageIdentity::property_backing(
                PropertyOwner::Property(property),
                MaterializationRoot::cone_owned(),
            )
            .unwrap()
        }
        InitializationUnitKey::Object(owner) => StaticStorageIdentity::singleton_published_root(
            owner,
            MaterializationRoot::cone_owned(),
        )
        .unwrap(),
        InitializationUnitKey::ExtensionProperty(_)
        | InitializationUnitKey::Companion(_)
        | InitializationUnitKey::GenericDelegatedExtensionApplication { .. }
        | InitializationUnitKey::GenericCompanionTemplate(_)
        | InitializationUnitKey::GenericCompanionApplication { .. } => unreachable!(),
    };
    let mut globals = Arena::new();
    let storage = globals.alloc(storage_global(
        storage_identity,
        if lazy {
            exact_type("Singleton", SourceNominalKind::Object)
        } else {
            exact_type("Value", SourceNominalKind::Struct)
        },
        if lazy {
            scoop_lir::MANAGED_PTR
        } else {
            LirType::I64
        },
        if lazy {
            RefScan::References(vec![0])
        } else {
            RefScan::None
        },
    ));
    let failure = globals.alloc(storage_global(
        StaticStorageIdentity::initialization_failure_root(
            unit_id,
            MaterializationRoot::cone_owned(),
        )
        .unwrap(),
        exact_type("Failure", SourceNominalKind::Class),
        scoop_lir::MANAGED_PTR,
        RefScan::References(vec![0]),
    ));

    let initializer = generated_body(unit_id, InitializationCallableRole::Initializer);
    let ensure = generated_body(unit_id, InitializationCallableRole::Ensure);
    let mut local_functions = LocalFunctionIdentities::default();
    let initializer_ref = local_functions.alloc_managed();
    let ensure_ref = local_functions.alloc_managed();
    let mut functions = vec![function(initializer), function(ensure)];
    if !lazy {
        local_functions.alloc_managed();
        functions.push(function(
            CallableBodyIdentity::for_initialization_startup_gateway(unit_id).unwrap(),
        ));
    }
    let mut initialization_units = Arena::new();
    initialization_units.alloc(InitializationUnit {
        identity: unit_identity,
        display_name: if lazy {
            "object:Singleton".to_string()
        } else {
            "top-level:value".to_string()
        },
        schedule: if lazy {
            InitializationSchedule::LazyAccess
        } else {
            InitializationSchedule::EagerStartup
        },
        kind: if lazy {
            InitializationUnitKind::LazySingleton {
                published_root: storage,
            }
        } else {
            InitializationUnitKind::EagerTopLevel { storage }
        },
        failure_root: failure,
        initializer: initializer_ref,
        ensure: ensure_ref,
        dependencies: Vec::new(),
    });
    Module {
        release_hooks: Default::default(),
        cone: scoop_lir::ConeIdentity::SINGLE_FILE,
        globals,
        initialization_units,
        structs: StructDefs::default(),
        enums: EnumDefs::default(),
        functions,
        extern_functions: ExternFunctions::default(),
        native_globals: Arena::new(),
        native_global_bridges: NativeGlobalBridges::default(),
        callback_bridges: Arena::new(),
        foreign_callback_families: Arena::new(),
        foreign_callback_bridges: Arena::new(),
        output: scoop_lir::LirOutput::Executable {
            entry: LocalFunctionRef::Managed(initializer_ref),
        },
        meta: metadata(),
    }
}

fn generated_body(
    unit: scoop_lir::PersistentInitializationUnitId,
    role: InitializationCallableRole,
) -> CallableBodyIdentity {
    let generated =
        PersistentGeneratedCallableId::from_key(&GeneratedCallableKey::Initialization {
            unit,
            role,
        })
        .unwrap();
    CallableBodyIdentity::for_generated_callable(generated).unwrap()
}

fn function(callable_body: CallableBodyIdentity) -> Function {
    let mut blocks = Arena::new();
    let entry = blocks.alloc(BasicBlock {
        name: "entry".to_string(),
        instructions: Vec::new(),
        terminator: Terminator::Return { value: None },
    });
    Function {
        callable_body,
        gc_effect: GcEffect::Managed,
        signature: ScoopAbiSignature::new(
            Vec::new(),
            AbiReturn::UnitVoid,
            CallingConvention::Cdecl,
        ),
        call_targets: CallTargets::default(),
        safepoints: SafepointIdentities::default(),
        locals: Arena::new(),
        temps: Arena::new(),
        blocks,
        entry,
    }
}

fn storage_global(
    identity: StaticStorageIdentity,
    exact_type: PersistentExactTypeId,
    ty: LirType,
    scan: RefScan,
) -> Global {
    Global {
        address_kind: PointerKind::Raw,
        scan,
        init: GlobalInit::Storage {
            identity,
            layout: LayoutIdentity::managed_value(
                exact_type,
                LirTargetProfile::DARWIN_AARCH64,
                MaterializationRoot::cone_owned(),
            )
            .unwrap()
            .into(),
            ty,
            initial_state: LirStaticInitialState::ZeroedForRuntimeUnit,
        },
    }
}

fn metadata() -> LirMeta {
    let string_type = exact_type("String", SourceNominalKind::Class);
    let mut layouts = Arena::new();
    let _string_layout = layouts.alloc(Layout {
        identity: LayoutIdentity::managed_object(
            string_type,
            LirTargetProfile::DARWIN_AARCH64,
            MaterializationRoot::cone_owned(),
        )
        .unwrap(),
        name: "String".to_string(),
        size: 24,
        align: 8,
        fields: Vec::new(),
        c_layout: None,
        interior_mutable: false,
        kind: LayoutKind::Intrinsic(scoop_lir::IntrinsicTypeRepresentation::String),
    });
    let mut external_type_descriptors = Arena::new();
    let string_descriptor = external_type_descriptors.alloc(
        ExternalTypeDescriptor::new(scoop_identity::ConeIdentity::CORE, string_type).unwrap(),
    );
    LirMeta {
        exact_types: Vec::new(),
        target_profile: LirTargetProfile::DARWIN_AARCH64,
        canonical_c_abi: CanonicalCAbiMetadata::default(),
        native_externals: NativeExternalMetadata::default(),
        well_known_type_descriptors: WellKnownTypeDescriptors {
            string: TypeDescriptorRef::External(string_descriptor),
        },
        arrays: Arena::new(),
        layouts,
        type_descriptors: Arena::new(),
        external_type_descriptors,
        external_callables: Arena::new(),
    }
}

fn property(name: &str) -> PersistentPropertyId {
    PersistentPropertyId::from_source_declaration(&SourceDeclarationKey::property(
        source_site(),
        CanonicalIdentifier::new(name).unwrap(),
    ))
    .unwrap()
}

fn exact_type(name: &str, kind: SourceNominalKind) -> PersistentExactTypeId {
    PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(nominal(name, kind))).unwrap()
}

fn nominal(name: &str, kind: SourceNominalKind) -> PersistentTypeId {
    PersistentTypeId::from_source_declaration(&SourceDeclarationKey::nominal(
        source_site(),
        CanonicalIdentifier::new(name).unwrap(),
        kind,
        0,
    ))
    .unwrap()
}

fn source_site() -> SourceDeclarationSite {
    SourceDeclarationSite::new(
        scoop_lir::ConeIdentity::SINGLE_FILE,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap()
}
