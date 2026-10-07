use la_arena::Arena;
use scoop_identity::{
    CanonicalIdentifier, CborIdentityRecord, ConeIdentity, DeclarationScope, DefinitionAtomRole,
    DefinitionAtomSubkey, DefinitionOwnerChain, ExactTypeKey, ImmortalObjectKey,
    ImmortalObjectOwner, LinkageClass, ObjectDefinitionAtomKey, ObjectDefinitionPlanKey,
    PackagePath, PersistentExactTypeId, PersistentImmortalObjectId, PersistentPropertyId,
    PersistentSymbolKey, PersistentSymbolRequest, PersistentSymbolRequestTable, PersistentTypeId,
    PropertyOwner, SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind,
    StrongDefinitionEntity, StrongDefinitionRole, StructuralDefinitionPath,
    StructuralDefinitionSiteRole, StructuralPathSegment,
};
use scoop_lir::{
    CanonicalCAbiMetadata, CanonicalLirFoundation, ConeLirFoundation, EnumDefs, ExternFunctions,
    ExternalTypeDescriptor, Global, GlobalInit, ImmortalObjectIdentity,
    IntrinsicTypeRepresentation, Layout, LayoutIdentity, LayoutKind, LirMeta, LirTargetProfile,
    LocalFunctionIdentities, LocalFunctionRef, MaterializationRoot, Module, NativeExternalMetadata,
    NativeGlobalBridges, PointerKind, RefScan, RegistrationIdentitySurfaceV1,
    StrongImmortalObjectRegistrationPlanSetV1, StrongImmortalObjectSemanticPlanSetV1, StructDefs,
    TypeDescriptorRef, WellKnownTypeDescriptors,
};

pub(super) fn immortal_plan(object_count: u32) -> StrongImmortalObjectRegistrationPlanSetV1 {
    assert!(object_count > 0);
    let string_type = exact_type("String");
    let artifacts = (0..object_count)
        .map(|seed| object_artifacts(seed + 1))
        .collect::<Vec<_>>();
    let module = semantic_module(&artifacts, string_type);
    let semantics = StrongImmortalObjectSemanticPlanSetV1::from_module(&module).unwrap();

    let mut canonical = CanonicalLirFoundation::empty();
    canonical
        .set_immortal_objects(artifacts.iter().map(|item| item.object.clone()).collect())
        .unwrap();
    canonical
        .set_definition_plans(
            artifacts
                .iter()
                .flat_map(|item| {
                    [
                        item.object_definition.clone(),
                        item.registration_definition.clone(),
                    ]
                })
                .collect(),
        )
        .unwrap();
    canonical
        .set_definition_atoms(
            artifacts
                .iter()
                .flat_map(|item| {
                    [
                        item.object_primary.clone(),
                        item.registration_primary.clone(),
                    ]
                })
                .collect(),
        )
        .unwrap();
    canonical.set_symbol_requests(
        PersistentSymbolRequestTable::new(
            artifacts
                .iter()
                .flat_map(|item| {
                    [
                        symbol(PersistentSymbolKey::ImmortalObject(item.object.id())),
                        symbol(PersistentSymbolKey::ImmortalRegistration(item.object.id())),
                    ]
                })
                .collect(),
        )
        .unwrap(),
    );
    let foundation = ConeLirFoundation::try_new(ConeIdentity::SINGLE_FILE, canonical).unwrap();
    let identities = RegistrationIdentitySurfaceV1::from_foundation(&foundation).unwrap();
    StrongImmortalObjectRegistrationPlanSetV1::new(&foundation, &identities, &semantics).unwrap()
}

struct ObjectArtifacts {
    object: CborIdentityRecord<PersistentImmortalObjectId, ImmortalObjectKey>,
    object_definition:
        CborIdentityRecord<scoop_identity::ObjectDefinitionPlanId, ObjectDefinitionPlanKey>,
    object_primary:
        CborIdentityRecord<scoop_identity::ObjectDefinitionAtomId, ObjectDefinitionAtomKey>,
    registration_definition:
        CborIdentityRecord<scoop_identity::ObjectDefinitionPlanId, ObjectDefinitionPlanKey>,
    registration_primary:
        CborIdentityRecord<scoop_identity::ObjectDefinitionAtomId, ObjectDefinitionAtomKey>,
}

fn object_artifacts(seed: u32) -> ObjectArtifacts {
    let object = CborIdentityRecord::from_key(immortal_key(seed)).unwrap();
    let object_definition = definition(
        StrongDefinitionEntity::immortal_object(object.id()),
        StrongDefinitionRole::ImmortalObject,
    );
    let registration_definition = definition(
        StrongDefinitionEntity::immortal_object(object.id()),
        StrongDefinitionRole::ImmortalRegistration,
    );
    ObjectArtifacts {
        object,
        object_primary: primary(&object_definition),
        registration_primary: primary(&registration_definition),
        object_definition,
        registration_definition,
    }
}

fn semantic_module(artifacts: &[ObjectArtifacts], string_type: PersistentExactTypeId) -> Module {
    let mut globals = Arena::new();
    for (index, item) in artifacts.iter().enumerate() {
        globals.alloc(Global {
            address_kind: PointerKind::Managed,
            scan: RefScan::None,
            init: GlobalInit::StringConst {
                identity: ImmortalObjectIdentity::from_key(
                    item.object.key().clone(),
                    MaterializationRoot::cone_owned(),
                )
                .unwrap(),
                value: format!("value-{index}"),
            },
        });
    }
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

fn immortal_key(path_index: u32) -> ImmortalObjectKey {
    let property = PersistentPropertyId::from_source_declaration(&SourceDeclarationKey::property(
        source_site(),
        CanonicalIdentifier::new("text").unwrap(),
    ))
    .unwrap();
    ImmortalObjectKey::string_constant(
        ImmortalObjectOwner::Property(PropertyOwner::Property(property)),
        StructuralDefinitionPath::from_first(
            StructuralPathSegment::new(StructuralDefinitionSiteRole::StringConstant, path_index),
            [],
        ),
    )
}

fn exact_type(name: &str) -> PersistentExactTypeId {
    let nominal = PersistentTypeId::from_source_declaration(&SourceDeclarationKey::nominal(
        source_site(),
        CanonicalIdentifier::new(name).unwrap(),
        SourceNominalKind::Class,
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
