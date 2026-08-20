use la_arena::Arena;
use scoop_identity::{
    CanonicalIdentifier, CborIdentityRecord, ConeIdentity, DeclarationScope, DefinitionAtomRole,
    DefinitionAtomSubkey, DefinitionOwnerChain, DigestNodeId, DigestNodeKey, DigestPatchIntentKey,
    DigestSemanticFieldRole, ExactTypeKey, ImmortalObjectKey, ImmortalObjectOwner, LinkageClass,
    ObjectDefinitionAtomKey, ObjectDefinitionPlanKey, PackagePath, PersistentExactTypeId,
    PersistentLayoutId, PersistentPropertyId, PersistentSymbolKey, PersistentSymbolRequest,
    PersistentSymbolRequestTable, PersistentTypeId, PropertyOwner, SourceDeclarationKey,
    SourceDeclarationSite, SourceNominalKind, StrongDefinitionEntity, StrongDefinitionRole,
    StructuralDefinitionPath, StructuralDefinitionSiteRole, StructuralPathSegment,
};
use scoop_lir::{
    CanonicalCAbiMetadata, CanonicalLirFoundation, ConeLirFoundation, DigestFinalizationPlanV1,
    DigestInputRefV1, DigestNodeV1, EnumDefs, ExternFunctions, ExternalTypeDescriptor, Global,
    GlobalInit, ImmortalObjectIdentity, IntrinsicTypeRepresentation, Layout, LayoutIdentity,
    LayoutKind, LirConstantImage, LirMeta, LirStaticInitialState, LirTargetProfile, LirType,
    LocalFunctionIdentities, LocalFunctionRef, MaterializationRoot, Module, NativeExternalMetadata,
    NativeGlobalBridges, PointerKind, RefScan, RegistrationIdentitySurfaceV1,
    StaticStorageIdentity, StrongStaticStorageRegistrationPlanSetV1,
    StrongStaticStorageSemanticPlanSetV1, StructDefs, TypeDescriptorRef, WellKnownTypeDescriptors,
};

pub(super) fn static_storage_plan() -> StrongStaticStorageRegistrationPlanSetV1 {
    let module = semantic_module();
    let semantics = StrongStaticStorageSemanticPlanSetV1::from_module(&module).unwrap();
    let mut canonical = CanonicalLirFoundation::empty();

    let shapes = semantics
        .storages()
        .iter()
        .map(|semantic| ShapeArtifacts {
            layout: semantic.layout(),
            scan: semantic.scan(),
            layout_definition: definition(
                StrongDefinitionEntity::layout(semantic.layout()),
                StrongDefinitionRole::Layout,
            ),
            scan_definition: definition(
                StrongDefinitionEntity::scan(semantic.scan()),
                StrongDefinitionRole::ScanProgram,
            ),
        })
        .collect::<Vec<_>>();
    let storages = semantics
        .storages()
        .iter()
        .map(|semantic| StorageArtifacts::new(semantic.storage(), semantic.initial_state()))
        .collect::<Vec<_>>();
    let immortal = module
        .globals
        .iter()
        .find_map(|(_, global)| match &global.init {
            GlobalInit::StringConst { identity, .. } => Some(identity.identity_record().clone()),
            GlobalInit::CString { .. }
            | GlobalInit::Storage { .. }
            | GlobalInit::RawStorage { .. }
            | GlobalInit::ImportedStorage { .. } => None,
        })
        .unwrap();
    let immortal_object_definition = definition(
        StrongDefinitionEntity::immortal_object(immortal.id()),
        StrongDefinitionRole::ImmortalObject,
    );
    let immortal_registration_definition = definition(
        StrongDefinitionEntity::immortal_object(immortal.id()),
        StrongDefinitionRole::ImmortalRegistration,
    );

    canonical
        .set_static_storages(
            module
                .globals
                .iter()
                .filter_map(|(_, global)| match &global.init {
                    GlobalInit::Storage { identity, .. } => {
                        Some(identity.identity_record().clone())
                    }
                    GlobalInit::CString { .. }
                    | GlobalInit::StringConst { .. }
                    | GlobalInit::RawStorage { .. }
                    | GlobalInit::ImportedStorage { .. } => None,
                })
                .collect(),
        )
        .unwrap();
    canonical
        .set_immortal_objects(vec![immortal.clone()])
        .unwrap();
    canonical
        .set_layouts(
            module
                .globals
                .iter()
                .filter_map(|(_, global)| match &global.init {
                    GlobalInit::Storage { layout, .. } => {
                        Some(layout.local().unwrap().layout_record().clone())
                    }
                    GlobalInit::CString { .. }
                    | GlobalInit::StringConst { .. }
                    | GlobalInit::RawStorage { .. }
                    | GlobalInit::ImportedStorage { .. } => None,
                })
                .collect(),
        )
        .unwrap();
    canonical
        .set_scans(
            module
                .globals
                .iter()
                .filter_map(|(_, global)| match &global.init {
                    GlobalInit::Storage { layout, .. } => {
                        Some(layout.local().unwrap().scan_record().clone())
                    }
                    GlobalInit::CString { .. }
                    | GlobalInit::StringConst { .. }
                    | GlobalInit::RawStorage { .. }
                    | GlobalInit::ImportedStorage { .. } => None,
                })
                .collect(),
        )
        .unwrap();

    let mut definitions = Vec::new();
    let mut atoms = Vec::new();
    for shape in &shapes {
        definitions.extend([
            shape.layout_definition.clone(),
            shape.scan_definition.clone(),
        ]);
        atoms.extend([
            primary(&shape.layout_definition),
            primary(&shape.scan_definition),
        ]);
    }
    for storage in &storages {
        definitions.extend([
            storage.storage_definition.clone(),
            storage.registration_definition.clone(),
        ]);
        atoms.extend(storage.atoms());
    }
    definitions.extend([
        immortal_object_definition.clone(),
        immortal_registration_definition.clone(),
    ]);
    atoms.extend([
        primary(&immortal_object_definition),
        primary(&immortal_registration_definition),
    ]);
    canonical.set_definition_plans(definitions).unwrap();
    canonical.set_definition_atoms(atoms).unwrap();

    let symbols = semantics
        .storages()
        .iter()
        .flat_map(|semantic| {
            [
                semantic.symbol(),
                symbol(PersistentSymbolKey::RootRegistration(semantic.storage())),
                symbol(PersistentSymbolKey::Layout(semantic.layout())),
                symbol(PersistentSymbolKey::ScanProgram(semantic.scan())),
            ]
        })
        .chain([
            symbol(PersistentSymbolKey::ImmortalObject(immortal.id())),
            symbol(PersistentSymbolKey::ImmortalRegistration(immortal.id())),
        ])
        .collect();
    canonical.set_symbol_requests(PersistentSymbolRequestTable::new(symbols).unwrap());

    let foundation = ConeLirFoundation::try_new(ConeIdentity::SINGLE_FILE, canonical).unwrap();
    let digests = digest_plan(&foundation, &semantics, &shapes, &storages);
    let identities = RegistrationIdentitySurfaceV1::from_foundation(&foundation).unwrap();
    StrongStaticStorageRegistrationPlanSetV1::new(&foundation, &identities, &semantics, &digests)
        .unwrap()
}

struct ShapeArtifacts {
    layout: PersistentLayoutId,
    scan: scoop_lir::PersistentScanId,
    layout_definition:
        CborIdentityRecord<scoop_identity::ObjectDefinitionPlanId, ObjectDefinitionPlanKey>,
    scan_definition:
        CborIdentityRecord<scoop_identity::ObjectDefinitionPlanId, ObjectDefinitionPlanKey>,
}

struct StorageArtifacts {
    storage: scoop_lir::PersistentStaticStorageId,
    storage_definition:
        CborIdentityRecord<scoop_identity::ObjectDefinitionPlanId, ObjectDefinitionPlanKey>,
    registration_definition:
        CborIdentityRecord<scoop_identity::ObjectDefinitionPlanId, ObjectDefinitionPlanKey>,
    template: bool,
    relocations: bool,
}

impl StorageArtifacts {
    fn new(
        storage: scoop_lir::PersistentStaticStorageId,
        initial_state: &scoop_lir::StrongStaticStorageInitialStatePlanV1,
    ) -> Self {
        Self {
            storage,
            storage_definition: definition(
                StrongDefinitionEntity::static_storage(storage),
                StrongDefinitionRole::StaticStorage,
            ),
            registration_definition: definition(
                StrongDefinitionEntity::static_storage(storage),
                StrongDefinitionRole::RootRegistration,
            ),
            template: matches!(
                initial_state,
                scoop_lir::StrongStaticStorageInitialStatePlanV1::EncodedStaticValue { .. }
            ),
            relocations: !initial_state.immortal_relocations().is_empty(),
        }
    }

    fn atoms(
        &self,
    ) -> Vec<CborIdentityRecord<scoop_identity::ObjectDefinitionAtomId, ObjectDefinitionAtomKey>>
    {
        let mut atoms = vec![
            primary(&self.storage_definition),
            primary(&self.registration_definition),
        ];
        if self.template {
            atoms.push(
                CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(
                    self.storage_definition.id(),
                    DefinitionAtomRole::AddressTakenConstant,
                    DefinitionAtomSubkey::StaticStorage(self.storage),
                ))
                .unwrap(),
            );
        }
        if self.relocations {
            atoms.push(
                CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(
                    self.storage_definition.id(),
                    DefinitionAtomRole::RuntimeRecord,
                    DefinitionAtomSubkey::StaticStorage(self.storage),
                ))
                .unwrap(),
            );
        }
        atoms
    }
}

fn digest_plan(
    foundation: &ConeLirFoundation,
    semantics: &StrongStaticStorageSemanticPlanSetV1,
    shapes: &[ShapeArtifacts],
    storages: &[StorageArtifacts],
) -> DigestFinalizationPlanV1 {
    let mut nodes = Vec::new();
    let mut image_inputs = Vec::new();
    for ((semantic, shape), storage) in semantics.storages().iter().zip(shapes).zip(storages) {
        let layout_key = DigestNodeKey::layout(shape.layout);
        let layout_id = DigestNodeId::from_key(&layout_key).unwrap();
        let layout = DigestNodeV1::new(
            layout_key,
            Vec::new(),
            vec![DigestPatchIntentKey::new(
                layout_id,
                storage.registration_definition.id(),
                DefinitionAtomRole::Primary,
                DigestSemanticFieldRole::Layout,
            )],
        )
        .unwrap();
        let scan_key = DigestNodeKey::scan(shape.scan);
        let scan_id = DigestNodeId::from_key(&scan_key).unwrap();
        let scan = DigestNodeV1::new(
            scan_key,
            Vec::new(),
            vec![DigestPatchIntentKey::new(
                scan_id,
                storage.registration_definition.id(),
                DefinitionAtomRole::Primary,
                DigestSemanticFieldRole::Scan,
            )],
        )
        .unwrap();
        assert_eq!(semantic.storage(), storage.storage);
        image_inputs.extend([
            DigestInputRefV1::from_node(&layout),
            DigestInputRefV1::from_node(&scan),
        ]);
        nodes.extend([layout, scan]);
    }

    nodes.push(
        DigestNodeV1::new(
            DigestNodeKey::runtime_image(ConeIdentity::SINGLE_FILE),
            image_inputs,
            Vec::new(),
        )
        .unwrap(),
    );
    DigestFinalizationPlanV1::new(nodes, foundation).unwrap()
}

fn semantic_module() -> Module {
    let string_type = exact_type("String", SourceNominalKind::Class);
    let mut globals = Arena::new();
    let immortal = globals.alloc(Global {
        address_kind: PointerKind::Managed,
        scan: RefScan::None,
        init: GlobalInit::StringConst {
            identity: ImmortalObjectIdentity::from_key(
                ImmortalObjectKey::string_constant(
                    ImmortalObjectOwner::Property(PropertyOwner::Property(property("text"))),
                    StructuralDefinitionPath::from_first(
                        StructuralPathSegment::new(StructuralDefinitionSiteRole::StringConstant, 0),
                        [],
                    ),
                ),
                MaterializationRoot::cone_owned(),
            )
            .unwrap(),
            value: "immortal".to_string(),
        },
    });
    globals.alloc(storage_global(
        "encoded",
        LirType::Ptr(PointerKind::Managed),
        RefScan::References(vec![0]),
        LirStaticInitialState::EncodedStaticValue {
            payload: LirConstantImage::GlobalPointer {
                global: immortal,
                kind: PointerKind::Managed,
            },
        },
    ));
    globals.alloc(storage_global(
        "zeroed",
        LirType::I64,
        RefScan::None,
        LirStaticInitialState::ZeroedForRuntimeUnit,
    ));

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
        kind: LayoutKind::Intrinsic(IntrinsicTypeRepresentation::String),
    });
    let mut external_type_descriptors = Arena::new();
    let string_descriptor = external_type_descriptors.alloc(
        ExternalTypeDescriptor::new(scoop_identity::ConeIdentity::CORE, string_type).unwrap(),
    );
    let mut local_functions = LocalFunctionIdentities::default();
    let entry = LocalFunctionRef::Managed(local_functions.alloc_managed());
    Module {
        release_hooks: Default::default(),
        cone: ConeIdentity::SINGLE_FILE,
        globals,
        initialization_units: Arena::new(),
        structs: StructDefs::default(),
        enums: EnumDefs::default(),
        functions: Vec::new(),
        extern_functions: ExternFunctions::default(),
        native_globals: Arena::new(),
        native_global_bridges: NativeGlobalBridges::default(),
        callback_bridges: Arena::new(),
        foreign_callback_families: Arena::new(),
        foreign_callback_bridges: Arena::new(),
        output: scoop_lir::LirOutput::Executable { entry },
        meta: LirMeta {
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
        },
    }
}

fn storage_global(
    name: &str,
    ty: LirType,
    scan: RefScan,
    initial_state: LirStaticInitialState,
) -> Global {
    Global {
        address_kind: PointerKind::Raw,
        scan,
        init: GlobalInit::Storage {
            identity: StaticStorageIdentity::property_backing(
                PropertyOwner::Property(property(name)),
                MaterializationRoot::cone_owned(),
            )
            .unwrap(),
            layout: LayoutIdentity::managed_value(
                exact_type(name, SourceNominalKind::Struct),
                LirTargetProfile::DARWIN_AARCH64,
                MaterializationRoot::cone_owned(),
            )
            .unwrap()
            .into(),
            ty,
            initial_state,
        },
    }
}

fn definition(
    entity: StrongDefinitionEntity,
    role: StrongDefinitionRole,
) -> CborIdentityRecord<scoop_identity::ObjectDefinitionPlanId, ObjectDefinitionPlanKey> {
    CborIdentityRecord::from_key(
        ObjectDefinitionPlanKey::strong(ConeIdentity::SINGLE_FILE, entity, role).unwrap(),
    )
    .unwrap()
}

fn primary(
    definition: &CborIdentityRecord<
        scoop_identity::ObjectDefinitionPlanId,
        ObjectDefinitionPlanKey,
    >,
) -> CborIdentityRecord<scoop_identity::ObjectDefinitionAtomId, ObjectDefinitionAtomKey> {
    CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(
        definition.id(),
        DefinitionAtomRole::Primary,
        DefinitionAtomSubkey::Singleton,
    ))
    .unwrap()
}

fn symbol(key: PersistentSymbolKey) -> PersistentSymbolRequest {
    PersistentSymbolRequest::new(key, LinkageClass::ConeStrong).unwrap()
}

fn property(name: &str) -> PersistentPropertyId {
    PersistentPropertyId::from_source_declaration(&SourceDeclarationKey::property(
        source_site(),
        CanonicalIdentifier::new(name).unwrap(),
    ))
    .unwrap()
}

fn exact_type(name: &str, kind: SourceNominalKind) -> PersistentExactTypeId {
    let nominal = PersistentTypeId::from_source_declaration(&SourceDeclarationKey::nominal(
        source_site(),
        CanonicalIdentifier::new(name).unwrap(),
        kind,
        0,
    ))
    .unwrap();
    PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(nominal)).unwrap()
}

fn source_site() -> SourceDeclarationSite {
    SourceDeclarationSite::new(
        ConeIdentity::SINGLE_FILE,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap()
}
