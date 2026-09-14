use la_arena::Arena;
use scoop_identity::{
    CanonicalIdentifier, CborIdentityRecord, ConeIdentity, DeclarationScope, DefinitionAtomRole,
    DefinitionAtomSubkey, DefinitionOwnerChain, DigestNodeId, DigestNodeKey, DigestPatchIntentKey,
    DigestSemanticFieldRole, DispatchTableKey, ExactTypeKey, LayoutKey, LinkageClass,
    ObjectDefinitionAtomKey, ObjectDefinitionPlanKey, PackagePath, PersistentDispatchTableId,
    PersistentExactTypeId, PersistentLayoutId, PersistentScanId, PersistentSymbolKey,
    PersistentSymbolRequest, PersistentSymbolRequestTable, PersistentTypeId, RepresentationRole,
    ScanKey, ScanRole, SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind,
    StrongDefinitionEntity, StrongDefinitionRole,
};
use scoop_lir::{
    CanonicalCAbiMetadata, CanonicalLirFoundation, DigestInputRefV1, DigestNodeV1, EnumDefs,
    ExternFunctions, Layout, LayoutIdentity, LayoutKind, LirMeta, LirTargetProfile,
    LocalFunctionIdentities, LocalFunctionRef, MaterializationRoot, Module, NativeExternalMetadata,
    NativeGlobalBridges, OdrFreeLirFoundation, RefScan, RuntimeTypeMappingRecord,
    StrongDigestFinalizationPlanV1, StrongRegistrationIdentitySurfaceV1,
    StrongTypeDescriptorSemanticPlanSetV1, StrongTypeRegistrationPlanSetV1, StructDefs,
    TypeDescriptor, TypeDescriptorIdentity, TypeDescriptorRef, TypeInstanceShapeV1, VtableRecord,
    WellKnownLayouts, WellKnownTypeDescriptors,
};

pub(super) fn type_plan(type_count: u8) -> StrongTypeRegistrationPlanSetV1 {
    assert!(type_count > 0);
    let types = (0..type_count)
        .map(|seed| type_artifacts(&format!("Type{seed}")))
        .collect::<Vec<_>>();
    let mut canonical = CanonicalLirFoundation::empty();
    canonical
        .set_layouts(types.iter().map(|item| item.layout.clone()).collect())
        .unwrap();
    canonical
        .set_scans(types.iter().map(|item| item.scan.clone()).collect())
        .unwrap();
    canonical
        .set_dispatch_tables(types.iter().map(|item| item.vtable.clone()).collect())
        .unwrap();
    canonical
        .set_runtime_types(
            types
                .iter()
                .map(|item| RuntimeTypeMappingRecord::new(item.exact_type).unwrap())
                .collect(),
        )
        .unwrap();
    canonical
        .set_definition_plans(
            types
                .iter()
                .flat_map(|item| {
                    [
                        item.descriptor_definition.clone(),
                        item.layout_definition.clone(),
                        item.registration_definition.clone(),
                    ]
                })
                .collect(),
        )
        .unwrap();
    canonical
        .set_definition_atoms(
            types
                .iter()
                .flat_map(|item| {
                    [
                        item.descriptor_primary.clone(),
                        item.descriptor_diagnostic.clone(),
                        item.layout_primary.clone(),
                        item.registration_primary.clone(),
                    ]
                })
                .collect(),
        )
        .unwrap();
    let symbols = types
        .iter()
        .flat_map(|item| {
            [
                PersistentSymbolKey::TypeDescriptor(item.exact_type),
                PersistentSymbolKey::Layout(item.layout.id()),
                PersistentSymbolKey::TypeRegistration(item.exact_type),
            ]
        })
        .map(|key| PersistentSymbolRequest::new(key, LinkageClass::ConeStrong).unwrap())
        .collect();
    canonical.set_symbol_requests(PersistentSymbolRequestTable::new(symbols).unwrap());
    let foundation = OdrFreeLirFoundation::try_new(ConeIdentity::SINGLE_FILE, canonical).unwrap();
    let digests = digest_plan(&foundation, &types);
    let identities =
        StrongRegistrationIdentitySurfaceV1::from_foundation(&foundation, &digests).unwrap();
    let semantics = type_semantics(&types);
    StrongTypeRegistrationPlanSetV1::new(
        LirTargetProfile::DARWIN_AARCH64,
        &foundation,
        &identities,
        &semantics,
        &digests,
    )
    .unwrap()
}

struct TypeArtifacts {
    exact_type: PersistentExactTypeId,
    layout: CborIdentityRecord<PersistentLayoutId, LayoutKey>,
    scan: CborIdentityRecord<PersistentScanId, ScanKey>,
    vtable: CborIdentityRecord<PersistentDispatchTableId, DispatchTableKey>,
    descriptor_definition:
        CborIdentityRecord<scoop_identity::ObjectDefinitionPlanId, ObjectDefinitionPlanKey>,
    descriptor_primary:
        CborIdentityRecord<scoop_identity::ObjectDefinitionAtomId, ObjectDefinitionAtomKey>,
    descriptor_diagnostic:
        CborIdentityRecord<scoop_identity::ObjectDefinitionAtomId, ObjectDefinitionAtomKey>,
    layout_definition:
        CborIdentityRecord<scoop_identity::ObjectDefinitionPlanId, ObjectDefinitionPlanKey>,
    layout_primary:
        CborIdentityRecord<scoop_identity::ObjectDefinitionAtomId, ObjectDefinitionAtomKey>,
    registration_definition:
        CborIdentityRecord<scoop_identity::ObjectDefinitionPlanId, ObjectDefinitionPlanKey>,
    registration_primary:
        CborIdentityRecord<scoop_identity::ObjectDefinitionAtomId, ObjectDefinitionAtomKey>,
}

fn type_artifacts(name: &str) -> TypeArtifacts {
    let source = SourceDeclarationKey::nominal(
        SourceDeclarationSite::new(
            ConeIdentity::SINGLE_FILE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new(name).unwrap(),
        SourceNominalKind::Class,
        0,
    );
    let nominal = PersistentTypeId::from_source_declaration(&source).unwrap();
    let exact_type = PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(nominal)).unwrap();
    let layout = CborIdentityRecord::from_key(LayoutKey::darwin_aarch64(
        exact_type,
        RepresentationRole::ManagedObject,
    ))
    .unwrap();
    let scan =
        CborIdentityRecord::from_key(ScanKey::new(layout.id(), ScanRole::ManagedObject)).unwrap();
    let vtable = CborIdentityRecord::from_key(DispatchTableKey::vtable(exact_type)).unwrap();
    let descriptor_definition = definition(
        StrongDefinitionEntity::exact_type(exact_type),
        StrongDefinitionRole::TypeDescriptor,
    );
    let layout_definition = definition(
        StrongDefinitionEntity::layout(layout.id()),
        StrongDefinitionRole::Layout,
    );
    let registration_definition = definition(
        StrongDefinitionEntity::exact_type(exact_type),
        StrongDefinitionRole::TypeRegistration,
    );
    TypeArtifacts {
        exact_type,
        layout,
        scan,
        vtable,
        descriptor_primary: primary(&descriptor_definition),
        descriptor_diagnostic: CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(
            descriptor_definition.id(),
            DefinitionAtomRole::AddressTakenConstant,
            DefinitionAtomSubkey::ExactType(exact_type),
        ))
        .unwrap(),
        layout_primary: primary(&layout_definition),
        registration_primary: primary(&registration_definition),
        descriptor_definition,
        layout_definition,
        registration_definition,
    }
}

fn type_semantics(types: &[TypeArtifacts]) -> StrongTypeDescriptorSemanticPlanSetV1 {
    let mut type_descriptors = Arena::new();
    let mut first_descriptor = None;
    for item in types {
        let identity = TypeDescriptorIdentity::new(
            RuntimeTypeMappingRecord::new(item.exact_type).unwrap(),
            MaterializationRoot::cone_owned(),
        )
        .unwrap();
        let instance_layout = LayoutIdentity::managed_object(
            item.exact_type,
            LirTargetProfile::DARWIN_AARCH64,
            MaterializationRoot::cone_owned(),
        )
        .unwrap();
        let vtable = VtableRecord::new(&identity, Vec::new()).unwrap();
        let descriptor = type_descriptors.alloc(TypeDescriptor {
            diagnostic_name: format!("type-{}", item.exact_type),
            identity,
            instance_layout,
            instance_shape: TypeInstanceShapeV1::fixed_object(
                LirTargetProfile::DARWIN_AARCH64,
                16,
                8,
                RefScan::None,
            )
            .unwrap(),
            parent: None,
            vtable,
            itables: Vec::new(),
        });
        first_descriptor.get_or_insert(descriptor);
    }
    let first_descriptor = first_descriptor.expect("the fixture requires at least one type");
    let mut layouts = Arena::new();
    let first_layout = layouts.alloc(Layout {
        identity: LayoutIdentity::managed_object(
            types[0].exact_type,
            LirTargetProfile::DARWIN_AARCH64,
            MaterializationRoot::cone_owned(),
        )
        .unwrap(),
        name: "fixture".to_string(),
        size: 16,
        align: 8,
        fields: Vec::new(),
        c_layout: None,
        interior_mutable: false,
        kind: LayoutKind::Plain {
            scan: RefScan::None,
        },
    });
    let mut functions = LocalFunctionIdentities::default();
    let module = Module {
        cone: ConeIdentity::SINGLE_FILE,
        globals: Arena::new(),
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
        entry: LocalFunctionRef::Managed(functions.alloc_managed()),
        meta: LirMeta {
            target_profile: LirTargetProfile::DARWIN_AARCH64,
            canonical_c_abi: CanonicalCAbiMetadata::default(),
            native_externals: NativeExternalMetadata::default(),
            well_known_layouts: WellKnownLayouts {
                string: first_layout,
            },
            well_known_type_descriptors: WellKnownTypeDescriptors {
                string: TypeDescriptorRef::Local(first_descriptor),
            },
            arrays: Arena::new(),
            layouts,
            type_descriptors,
            core_external_type_descriptors: Arena::new(),
            core_external_callables: Arena::new(),
        },
    };
    StrongTypeDescriptorSemanticPlanSetV1::from_module(&module).unwrap()
}

fn digest_plan(
    foundation: &OdrFreeLirFoundation,
    types: &[TypeArtifacts],
) -> StrongDigestFinalizationPlanV1 {
    let mut nodes = Vec::new();
    let mut image_inputs = Vec::new();
    for item in types {
        let descriptor_key = DigestNodeKey::object_definition(item.descriptor_primary.id());
        let descriptor_source = DigestNodeId::from_key(&descriptor_key).unwrap();
        let descriptor = DigestNodeV1::new(
            descriptor_key,
            Vec::new(),
            vec![DigestPatchIntentKey::new(
                descriptor_source,
                item.registration_definition.id(),
                DefinitionAtomRole::Primary,
                DigestSemanticFieldRole::DescriptorDefinition,
            )],
        )
        .unwrap();
        let layout_key = DigestNodeKey::layout(item.layout.id());
        let layout_source = DigestNodeId::from_key(&layout_key).unwrap();
        let layout = DigestNodeV1::new(
            layout_key,
            Vec::new(),
            vec![DigestPatchIntentKey::new(
                layout_source,
                item.registration_definition.id(),
                DefinitionAtomRole::Primary,
                DigestSemanticFieldRole::Layout,
            )],
        )
        .unwrap();
        let object = DigestNodeV1::new(
            DigestNodeKey::object_definition(item.registration_primary.id()),
            Vec::new(),
            Vec::new(),
        )
        .unwrap();
        let registration_key =
            DigestNodeKey::strong_registration(item.registration_definition.id());
        let registration_source = DigestNodeId::from_key(&registration_key).unwrap();
        let registration = DigestNodeV1::new(
            registration_key,
            vec![
                DigestInputRefV1::from_node(&object),
                DigestInputRefV1::from_node(&descriptor),
                DigestInputRefV1::from_node(&layout),
            ],
            vec![DigestPatchIntentKey::new(
                registration_source,
                item.registration_definition.id(),
                DefinitionAtomRole::Primary,
                DigestSemanticFieldRole::RegistrationDefinition,
            )],
        )
        .unwrap();
        image_inputs.push(DigestInputRefV1::from_node(&registration));
        nodes.extend([descriptor, layout, object, registration]);
    }
    nodes.push(
        DigestNodeV1::new(
            DigestNodeKey::runtime_image(ConeIdentity::SINGLE_FILE),
            image_inputs,
            Vec::new(),
        )
        .unwrap(),
    );
    StrongDigestFinalizationPlanV1::new(nodes, foundation).unwrap()
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
