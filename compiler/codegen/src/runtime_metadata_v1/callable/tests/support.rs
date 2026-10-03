use scoop_identity::{
    CallableBodyKey, CanonicalIdentifier, CborIdentityRecord, ConeIdentity, DeclarationScope,
    DefinitionAtomRole, DefinitionAtomSubkey, DefinitionOwnerChain, DigestNodeId, DigestNodeKey,
    DigestPatchIntentKey, DigestSemanticFieldRole, LinkageClass, ObjectDefinitionAtomKey,
    ObjectDefinitionPlanKey, PackagePath, PersistentFunctionId, PersistentSymbolKey,
    PersistentSymbolRequest, PersistentSymbolRequestTable, RuntimeIdentityRecord,
    SourceDeclarationKey, SourceDeclarationSite, StrongCallableDefinitionOwner,
    StrongDefinitionEntity, StrongDefinitionRole,
};
use scoop_lir::{
    CanonicalLirFoundation, ConeLirFoundation, DigestFinalizationPlanV1, DigestInputRefV1,
    DigestNodeV1, RegistrationIdentitySurfaceV1, StrongCallableRegistrationPlanSetV1,
};

pub(super) fn callable_plan() -> StrongCallableRegistrationPlanSetV1 {
    let declaration =
        PersistentFunctionId::from_source_declaration(&SourceDeclarationKey::function(
            SourceDeclarationSite::new(
                ConeIdentity::SINGLE_FILE,
                PackagePath::root(),
                DefinitionOwnerChain::top_level(),
                DeclarationScope::ConeWide,
            )
            .unwrap(),
            CanonicalIdentifier::new("callable").unwrap(),
            0,
            None,
            Vec::new(),
        ))
        .unwrap();
    let body = RuntimeIdentityRecord::from_key(&CallableBodyKey::strong(
        StrongCallableDefinitionOwner::Function(declaration),
    ))
    .unwrap();
    let body_definition = definition(
        StrongDefinitionEntity::callable_body(body.id()),
        StrongDefinitionRole::CallableBody,
    );
    let body_primary = primary(&body_definition);
    let registration_definition = definition(
        StrongDefinitionEntity::callable_body(body.id()),
        StrongDefinitionRole::CallableRegistration,
    );
    let registration_primary = primary(&registration_definition);

    let symbols = [
        PersistentSymbolKey::CallableBody(body.id()),
        PersistentSymbolKey::CallableRegistration(body.id()),
    ]
    .map(|key| PersistentSymbolRequest::new(key, LinkageClass::ConeStrong).unwrap());
    let mut canonical = CanonicalLirFoundation::empty();
    canonical.set_callable_bodies(vec![body]).unwrap();
    canonical
        .set_definition_plans(vec![body_definition, registration_definition.clone()])
        .unwrap();
    canonical
        .set_definition_atoms(vec![body_primary.clone(), registration_primary.clone()])
        .unwrap();
    canonical.set_symbol_requests(PersistentSymbolRequestTable::new(symbols.to_vec()).unwrap());
    let foundation = ConeLirFoundation::try_new(ConeIdentity::SINGLE_FILE, canonical).unwrap();

    let body_key = DigestNodeKey::object_definition(body_primary.id());
    let body_source = DigestNodeId::from_key(&body_key).unwrap();
    let body_node = DigestNodeV1::new(
        body_key,
        Vec::new(),
        vec![DigestPatchIntentKey::new(
            body_source,
            registration_definition.id(),
            DefinitionAtomRole::Primary,
            DigestSemanticFieldRole::CallableBodyDefinition,
        )],
    )
    .unwrap();
    let object_node = DigestNodeV1::new(
        DigestNodeKey::object_definition(registration_primary.id()),
        Vec::new(),
        Vec::new(),
    )
    .unwrap();
    let registration_key = DigestNodeKey::strong_registration(registration_definition.id());
    let registration_source = DigestNodeId::from_key(&registration_key).unwrap();
    let registration_node = DigestNodeV1::new(
        registration_key,
        vec![
            DigestInputRefV1::from_node(&object_node),
            DigestInputRefV1::from_node(&body_node),
        ],
        vec![DigestPatchIntentKey::new(
            registration_source,
            registration_definition.id(),
            DefinitionAtomRole::Primary,
            DigestSemanticFieldRole::RegistrationDefinition,
        )],
    )
    .unwrap();
    let image = DigestNodeV1::new(
        DigestNodeKey::runtime_image(ConeIdentity::SINGLE_FILE),
        vec![DigestInputRefV1::from_node(&registration_node)],
        Vec::new(),
    )
    .unwrap();
    let digests = DigestFinalizationPlanV1::new(
        vec![body_node, object_node, registration_node, image],
        &foundation,
    )
    .unwrap();
    let identities = RegistrationIdentitySurfaceV1::from_foundation(&foundation, &digests).unwrap();
    StrongCallableRegistrationPlanSetV1::new(
        &foundation,
        &identities,
        scoop_lir::StrongCallableRuntimeScanPlanSetV1::from_foundation_without_scans(&foundation)
            .unwrap(),
        &digests,
    )
    .unwrap()
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
