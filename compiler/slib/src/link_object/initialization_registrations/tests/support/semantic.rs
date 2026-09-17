use la_arena::Arena;
use scoop_identity::{
    CanonicalIdentifier, CborIdentityRecord, DeclarationScope, DefinitionAtomRole,
    DefinitionAtomSubkey, DefinitionOwnerChain, DigestNodeId, DigestNodeKey, DigestPatchIntentKey,
    DigestSemanticFieldRole, ExactTypeKey, GeneratedCallableKey, InitializationCallableRole,
    InitializationUnitKey, ObjectDefinitionAtomKey, ObjectDefinitionPlanId,
    ObjectDefinitionPlanKey, PackagePath, PersistentExactTypeId, PersistentGeneratedCallableId,
    PersistentPropertyId, PersistentSymbolKey, PersistentSymbolRequest,
    PersistentSymbolRequestTable, PersistentTypeId, PropertyOwner, SourceDeclarationKey,
    SourceDeclarationSite, SourceNominalKind, StrongDefinitionEntity, StrongDefinitionRole,
};
use scoop_lir::{
    AbiReturn, BasicBlock, CallTargets, CallableBodyIdentity, CallingConvention,
    CanonicalCAbiMetadata, CanonicalLirFoundation, CoreExternalTypeDescriptor, DigestInputRefV1,
    DigestNodeV1, EnumDefs, ExternFunctions, Function, GcEffect, Global, GlobalInit,
    InitializationSchedule, InitializationUnit, InitializationUnitKind, Layout, LayoutIdentity,
    LayoutKind, LirMeta, LirStaticInitialState, LirTargetProfile, LirType, LocalFunctionIdentities,
    LocalFunctionRef, MaterializationRoot, Module, NativeExternalMetadata, NativeGlobalBridges,
    OdrFreeLirFoundation, PointerKind, RefScan, SafepointIdentities, ScoopAbiSignature,
    StaticStorageIdentity, StrongCallableRegistrationPlanSetV1, StrongDigestFinalizationPlanV1,
    StrongInitializationUnitRegistrationPlanSetV1, StrongInitializationUnitSemanticPlanSetV1,
    StrongRegistrationIdentitySurfaceV1, StrongSafepointSemanticPlanSetV1, StructDefs, Terminator,
    TypeDescriptorRef, WellKnownTypeDescriptors,
};

pub(super) struct SemanticInputs {
    pub(super) foundation: OdrFreeLirFoundation,
    pub(super) definitions: Vec<ObjectDefinitionPlanId>,
    pub(super) digest_plan: StrongDigestFinalizationPlanV1,
    pub(super) plan: StrongInitializationUnitRegistrationPlanSetV1,
    pub(super) callable_plan: StrongCallableRegistrationPlanSetV1,
    pub(super) safepoints: StrongSafepointSemanticPlanSetV1,
}

pub(super) fn inputs(lazy: bool) -> SemanticInputs {
    let module = semantic_module(lazy);
    let semantics = StrongInitializationUnitSemanticPlanSetV1::from_module(&module).unwrap();
    let semantic = &semantics.units()[0];
    let unit = semantic.unit();

    let cell = definition_artifacts(
        StrongDefinitionEntity::initialization_unit(unit),
        StrongDefinitionRole::InitializationCell,
    );
    let descriptor = definition_artifacts(
        StrongDefinitionEntity::initialization_unit(unit),
        StrongDefinitionRole::InitializationDescriptor,
    );
    let diagnostic_atom = CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(
        descriptor.definition.id(),
        DefinitionAtomRole::AddressTakenConstant,
        DefinitionAtomSubkey::InitializationUnit(unit),
    ))
    .unwrap();
    let registration = definition_artifacts(
        StrongDefinitionEntity::initialization_unit(unit),
        StrongDefinitionRole::InitializationRegistration,
    );
    let storages = [semantic.storage(), semantic.failure_root()].map(|storage| StorageArtifacts {
        storage,
        storage_definition: definition_artifacts(
            StrongDefinitionEntity::static_storage(storage),
            StrongDefinitionRole::StaticStorage,
        ),
        registration: definition_artifacts(
            StrongDefinitionEntity::static_storage(storage),
            StrongDefinitionRole::RootRegistration,
        ),
    });
    let callables = module
        .functions
        .iter()
        .map(|function| CallableArtifacts {
            body: function.callable_body.id(),
            body_record: function.callable_body.identity_record().clone(),
            body_definition: definition_artifacts(
                StrongDefinitionEntity::callable_body(function.callable_body.id()),
                StrongDefinitionRole::CallableBody,
            ),
            registration: definition_artifacts(
                StrongDefinitionEntity::callable_body(function.callable_body.id()),
                StrongDefinitionRole::CallableRegistration,
            ),
        })
        .collect::<Vec<_>>();

    let mut canonical = CanonicalLirFoundation::empty();
    canonical
        .set_static_storages(
            module
                .globals
                .iter()
                .map(|(_, global)| match &global.init {
                    GlobalInit::Storage { identity, .. } => identity.identity_record().clone(),
                    GlobalInit::CString { .. } | GlobalInit::StringConst { .. } => unreachable!(),
                })
                .collect(),
        )
        .unwrap();
    canonical
        .set_callable_bodies(
            callables
                .iter()
                .map(|callable| callable.body_record.clone())
                .collect(),
        )
        .unwrap();
    canonical
        .set_definition_plans(
            [
                cell.definition.clone(),
                descriptor.definition.clone(),
                registration.definition.clone(),
            ]
            .into_iter()
            .chain(storages.iter().flat_map(|storage| {
                [
                    storage.storage_definition.definition.clone(),
                    storage.registration.definition.clone(),
                ]
            }))
            .chain(callables.iter().flat_map(|callable| {
                [
                    callable.body_definition.definition.clone(),
                    callable.registration.definition.clone(),
                ]
            }))
            .collect(),
        )
        .unwrap();
    canonical
        .set_definition_atoms(
            [
                cell.primary.clone(),
                descriptor.primary.clone(),
                diagnostic_atom,
                registration.primary.clone(),
            ]
            .into_iter()
            .chain(storages.iter().flat_map(|storage| {
                [
                    storage.storage_definition.primary.clone(),
                    storage.registration.primary.clone(),
                ]
            }))
            .chain(callables.iter().flat_map(|callable| {
                [
                    callable.body_definition.primary.clone(),
                    callable.registration.primary.clone(),
                ]
            }))
            .collect(),
        )
        .unwrap();
    canonical.set_symbol_requests(
        PersistentSymbolRequestTable::new(
            [
                symbol(PersistentSymbolKey::InitializationCell(unit)),
                symbol(PersistentSymbolKey::InitializationDescriptor(unit)),
                symbol(PersistentSymbolKey::InitializationRegistration(unit)),
            ]
            .into_iter()
            .chain(storages.iter().flat_map(|storage| {
                [
                    symbol(PersistentSymbolKey::StaticStorage(storage.storage)),
                    symbol(PersistentSymbolKey::RootRegistration(storage.storage)),
                ]
            }))
            .chain(callables.iter().flat_map(|callable| {
                [
                    symbol(PersistentSymbolKey::CallableBody(callable.body)),
                    symbol(PersistentSymbolKey::CallableRegistration(callable.body)),
                ]
            }))
            .collect(),
        )
        .unwrap(),
    );

    let foundation =
        OdrFreeLirFoundation::try_new(scoop_lir::ConeIdentity::SINGLE_FILE, canonical).unwrap();
    let digest_plan = digest_plan(
        &foundation,
        semantic.schedule().gateway(),
        &cell,
        &descriptor,
        &registration,
        &storages,
        &callables,
    );
    let identities =
        StrongRegistrationIdentitySurfaceV1::from_foundation(&foundation, &digest_plan).unwrap();
    let callable_plan = StrongCallableRegistrationPlanSetV1::new(
        &foundation,
        &identities,
        scoop_lir::StrongCallableRuntimeScanPlanSetV1::from_foundation_without_scans(&foundation)
            .unwrap(),
        &digest_plan,
    )
    .unwrap();
    let plan = StrongInitializationUnitRegistrationPlanSetV1::new(
        &foundation,
        &identities,
        &semantics,
        &digest_plan,
    )
    .unwrap();
    let safepoints = StrongSafepointSemanticPlanSetV1::from_module(&module).unwrap();
    let definitions = [
        cell.definition.id(),
        descriptor.definition.id(),
        registration.definition.id(),
    ]
    .into_iter()
    .chain(storages.iter().flat_map(|storage| {
        [
            storage.storage_definition.definition.id(),
            storage.registration.definition.id(),
        ]
    }))
    .chain(callables.iter().flat_map(|callable| {
        [
            callable.body_definition.definition.id(),
            callable.registration.definition.id(),
        ]
    }))
    .collect();
    SemanticInputs {
        foundation,
        definitions,
        digest_plan,
        plan,
        callable_plan,
        safepoints,
    }
}

struct DefinitionArtifacts {
    definition: CborIdentityRecord<scoop_lir::ObjectDefinitionPlanId, ObjectDefinitionPlanKey>,
    primary: CborIdentityRecord<scoop_lir::ObjectDefinitionAtomId, ObjectDefinitionAtomKey>,
}

struct StorageArtifacts {
    storage: scoop_lir::PersistentStaticStorageId,
    storage_definition: DefinitionArtifacts,
    registration: DefinitionArtifacts,
}

struct CallableArtifacts {
    body: scoop_lir::PersistentCallableBodyId,
    body_record: scoop_identity::RuntimeIdentityRecord<scoop_lir::PersistentCallableBodyId>,
    body_definition: DefinitionArtifacts,
    registration: DefinitionArtifacts,
}

fn definition_artifacts(
    entity: StrongDefinitionEntity,
    role: StrongDefinitionRole,
) -> DefinitionArtifacts {
    let definition = CborIdentityRecord::from_key(
        ObjectDefinitionPlanKey::strong(scoop_lir::ConeIdentity::SINGLE_FILE, entity, role)
            .unwrap(),
    )
    .unwrap();
    let primary = CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(
        definition.id(),
        DefinitionAtomRole::Primary,
        DefinitionAtomSubkey::Singleton,
    ))
    .unwrap();
    DefinitionArtifacts {
        definition,
        primary,
    }
}

fn digest_plan(
    foundation: &OdrFreeLirFoundation,
    gateway: Option<scoop_lir::PersistentCallableBodyId>,
    cell: &DefinitionArtifacts,
    descriptor: &DefinitionArtifacts,
    registration: &DefinitionArtifacts,
    storages: &[StorageArtifacts; 2],
    callables: &[CallableArtifacts],
) -> StrongDigestFinalizationPlanV1 {
    let cell_object = object_leaf(cell);
    let descriptor_object = object_leaf(descriptor);
    let registration_object = object_leaf(registration);
    let callable_objects = callables
        .iter()
        .map(|callable| {
            let key = DigestNodeKey::object_definition(callable.body_definition.primary.id());
            let id = DigestNodeId::from_key(&key).unwrap();
            let mut patches = vec![DigestPatchIntentKey::new(
                id,
                callable.registration.definition.id(),
                DefinitionAtomRole::Primary,
                DigestSemanticFieldRole::CallableBodyDefinition,
            )];
            if Some(callable.body) == gateway {
                patches.push(DigestPatchIntentKey::new(
                    id,
                    registration.definition.id(),
                    DefinitionAtomRole::Primary,
                    DigestSemanticFieldRole::GatewayDefinition,
                ));
            }
            DigestNodeV1::new(key, Vec::new(), patches).unwrap()
        })
        .collect::<Vec<_>>();
    let callable_registration_objects = callables
        .iter()
        .map(|callable| object_leaf(&callable.registration))
        .collect::<Vec<_>>();
    let mut nodes = vec![
        cell_object.clone(),
        descriptor_object.clone(),
        registration_object.clone(),
    ];
    nodes.extend(callable_objects.iter().cloned());
    nodes.extend(callable_registration_objects.iter().cloned());
    let mut image_inputs = Vec::new();
    for storage in storages {
        let strong = DigestNodeV1::new(
            DigestNodeKey::strong_registration(storage.registration.definition.id()),
            Vec::new(),
            Vec::new(),
        )
        .unwrap();
        image_inputs.push(DigestInputRefV1::from_node(&strong));
        nodes.push(strong);
    }
    for ((callable, body_object), registration_object) in callables
        .iter()
        .zip(&callable_objects)
        .zip(&callable_registration_objects)
    {
        let key = DigestNodeKey::strong_registration(callable.registration.definition.id());
        let id = DigestNodeId::from_key(&key).unwrap();
        let strong = DigestNodeV1::new(
            key,
            vec![
                DigestInputRefV1::from_node(registration_object),
                DigestInputRefV1::from_node(body_object),
            ],
            vec![DigestPatchIntentKey::new(
                id,
                callable.registration.definition.id(),
                DefinitionAtomRole::Primary,
                DigestSemanticFieldRole::RegistrationDefinition,
            )],
        )
        .unwrap();
        image_inputs.push(DigestInputRefV1::from_node(&strong));
        nodes.push(strong);
    }

    let registration_key = DigestNodeKey::strong_registration(registration.definition.id());
    let registration_id = DigestNodeId::from_key(&registration_key).unwrap();
    let mut registration_inputs = vec![
        DigestInputRefV1::from_node(&registration_object),
        DigestInputRefV1::from_node(&cell_object),
        DigestInputRefV1::from_node(&descriptor_object),
    ];
    if let Some(gateway) = gateway {
        let gateway_object = callables
            .iter()
            .zip(&callable_objects)
            .find_map(|(callable, object)| (callable.body == gateway).then_some(object))
            .unwrap();
        registration_inputs.push(DigestInputRefV1::from_node(gateway_object));
    }
    let unit_registration = DigestNodeV1::new(
        registration_key,
        registration_inputs,
        vec![DigestPatchIntentKey::new(
            registration_id,
            registration.definition.id(),
            DefinitionAtomRole::Primary,
            DigestSemanticFieldRole::RegistrationDefinition,
        )],
    )
    .unwrap();
    image_inputs.push(DigestInputRefV1::from_node(&unit_registration));
    nodes.push(unit_registration);
    nodes.push(
        DigestNodeV1::new(
            DigestNodeKey::runtime_image(scoop_lir::ConeIdentity::SINGLE_FILE),
            image_inputs,
            Vec::new(),
        )
        .unwrap(),
    );
    StrongDigestFinalizationPlanV1::new(nodes, foundation).unwrap()
}

fn object_leaf(artifacts: &DefinitionArtifacts) -> DigestNodeV1 {
    DigestNodeV1::new(
        DigestNodeKey::object_definition(artifacts.primary.id()),
        Vec::new(),
        Vec::new(),
    )
    .unwrap()
}

fn semantic_module(lazy: bool) -> Module {
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
        | InitializationUnitKey::GenericDelegatedExtensionApplication { .. } => unreachable!(),
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
            .unwrap(),
            ty,
            initial_state: LirStaticInitialState::ZeroedForRuntimeUnit,
            thread_local: false,
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
    let mut core_external_type_descriptors = Arena::new();
    let string_descriptor =
        core_external_type_descriptors.alloc(CoreExternalTypeDescriptor::new(string_type).unwrap());
    LirMeta {
        exact_types: Vec::new(),
        target_profile: LirTargetProfile::DARWIN_AARCH64,
        canonical_c_abi: CanonicalCAbiMetadata::default(),
        native_externals: NativeExternalMetadata::default(),
        well_known_type_descriptors: WellKnownTypeDescriptors {
            string: TypeDescriptorRef::CoreExternal(string_descriptor),
        },
        arrays: Arena::new(),
        layouts,
        type_descriptors: Arena::new(),
        core_external_type_descriptors,
        core_external_callables: Arena::new(),
        dependency_external_callables: Arena::new(),
    }
}

fn symbol(key: PersistentSymbolKey) -> PersistentSymbolRequest {
    PersistentSymbolRequest::new(key, scoop_lir::LinkageClass::ConeStrong).unwrap()
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
