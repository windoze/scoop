use scoop_identity::{
    CanonicalIdentifier, CborIdentityRecord, ConeIdentity, DeclarationScope, DefinitionAtomRole,
    DefinitionAtomSubkey, DefinitionOwnerChain, DigestNodeId, DigestNodeKey, DigestPatchIntentKey,
    DigestSemanticFieldRole, ExactTypeKey, LayoutKey, LinkageClass, ObjectDefinitionAtomKey,
    ObjectDefinitionPlanKey, PackagePath, PersistentExactTypeId, PersistentLayoutId,
    PersistentSymbolKey, PersistentSymbolRequest, PersistentSymbolRequestTable, PersistentTypeId,
    RepresentationRole, SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind,
    StrongDefinitionEntity, StrongDefinitionRole,
};
use scoop_lir::{
    CanonicalLirFoundation, DigestInputRefV1, DigestNodeV1, LirTargetProfile, OdrFreeLirFoundation,
    RuntimeTypeMappingRecord, StrongDigestFinalizationPlanV1, StrongRegistrationIdentitySurfaceV1,
    StrongTypeRegistrationPlanSetV1,
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
    StrongTypeRegistrationPlanSetV1::new(
        LirTargetProfile::DARWIN_AARCH64,
        &foundation,
        &identities,
        &digests,
    )
    .unwrap()
}

struct TypeArtifacts {
    exact_type: PersistentExactTypeId,
    layout: CborIdentityRecord<PersistentLayoutId, LayoutKey>,
    descriptor_definition:
        CborIdentityRecord<scoop_identity::ObjectDefinitionPlanId, ObjectDefinitionPlanKey>,
    descriptor_primary:
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
        descriptor_primary: primary(&descriptor_definition),
        layout_primary: primary(&layout_definition),
        registration_primary: primary(&registration_definition),
        descriptor_definition,
        layout_definition,
        registration_definition,
    }
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
