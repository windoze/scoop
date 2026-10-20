use super::*;

pub(super) struct ExecutableFixture {
    pub(super) foundation: ConeLirFoundation,
    pub(super) digests: DigestFinalizationPlanV1,
    pub(super) registrations: RegistrationIdentitySurfaceV1,
}

pub(super) fn executable_fixture() -> ExecutableFixture {
    fixture_for_entry(executable_source_entry())
}

pub(super) fn fixture_for_entry(entry: ExecutableEntryIdentity) -> ExecutableFixture {
    let producer = entry.root_cone();
    let declaration = entry.declaration();
    let main = entry.main();
    let gateway =
        PersistentCallableBodyId::from_key(&CallableBodyKey::root_gateway(producer, main)).unwrap();
    let failure_root = PersistentStaticStorageId::from_key(
        &StaticStorageKey::root_entry_failure_root(producer, main),
    )
    .unwrap();
    let main_body =
        RuntimeIdentityRecord::from_key(&CallableBodyKey::strong(declaration.strong_owner()))
            .unwrap();
    let gateway_body =
        RuntimeIdentityRecord::from_key(&CallableBodyKey::root_gateway(producer, main)).unwrap();
    let failure_storage =
        CborIdentityRecord::from_key(StaticStorageKey::root_entry_failure_root(producer, main))
            .unwrap();

    let definitions = [
        (
            StrongDefinitionEntity::callable_body(main.body()),
            StrongDefinitionRole::CallableBody,
        ),
        (
            StrongDefinitionEntity::callable_body(main.body()),
            StrongDefinitionRole::CallableRegistration,
        ),
        (
            StrongDefinitionEntity::callable_body(gateway),
            StrongDefinitionRole::CallableBody,
        ),
        (
            StrongDefinitionEntity::callable_body(gateway),
            StrongDefinitionRole::CallableRegistration,
        ),
        (
            StrongDefinitionEntity::static_storage(failure_root),
            StrongDefinitionRole::StaticStorage,
        ),
        (
            StrongDefinitionEntity::static_storage(failure_root),
            StrongDefinitionRole::RootRegistration,
        ),
        (
            StrongDefinitionEntity::root_entry(producer),
            StrongDefinitionRole::RootEntryDescriptor,
        ),
    ];
    let mut plans = Vec::new();
    let mut atoms = Vec::new();
    for (entity, role) in definitions {
        let plan = CborIdentityRecord::from_key(
            ObjectDefinitionPlanKey::strong(producer, entity, role).unwrap(),
        )
        .unwrap();
        atoms.push(
            CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(
                plan.id(),
                DefinitionAtomRole::Primary,
                DefinitionAtomSubkey::Singleton,
            ))
            .unwrap(),
        );
        plans.push(plan);
    }

    let symbols = [
        PersistentSymbolKey::CallableBody(main.body()),
        PersistentSymbolKey::CallableRegistration(main.body()),
        PersistentSymbolKey::CallableBody(gateway),
        PersistentSymbolKey::CallableRegistration(gateway),
        PersistentSymbolKey::StaticStorage(failure_root),
        PersistentSymbolKey::RootRegistration(failure_root),
        PersistentSymbolKey::RootEntryDescriptor(producer),
    ]
    .into_iter()
    .map(|key| PersistentSymbolRequest::new(key, LinkageClass::ConeStrong).unwrap())
    .collect();

    let mut canonical = CanonicalLirFoundation::empty();
    canonical
        .set_callable_bodies(vec![main_body, gateway_body])
        .unwrap();
    canonical
        .set_static_storages(vec![failure_storage])
        .unwrap();
    canonical.set_definition_plans(plans).unwrap();
    canonical.set_definition_atoms(atoms).unwrap();
    canonical.set_symbol_requests(PersistentSymbolRequestTable::new(symbols).unwrap());
    let foundation = ConeLirFoundation::try_new(producer, canonical).unwrap();

    let gateway_definition = definition_plan(
        producer,
        StrongDefinitionEntity::callable_body(gateway),
        StrongDefinitionRole::CallableBody,
    );
    let gateway_atom = primary_atom(&foundation, gateway_definition);
    let source_key = DigestNodeKey::source_signature(main.body());

    let source = DigestNodeV1::new(source_key, Vec::new()).unwrap();
    let gateway_key = DigestNodeKey::object_definition(gateway_atom);

    let gateway_definition_node = DigestNodeV1::new(gateway_key, Vec::new()).unwrap();
    let mut nodes = vec![source, gateway_definition_node];
    nodes.push(DigestNodeV1::new(DigestNodeKey::runtime_image(producer), Vec::new()).unwrap());
    let digests = DigestFinalizationPlanV1::new(nodes, &foundation).unwrap();
    let registrations = RegistrationIdentitySurfaceV1::from_foundation(&foundation).unwrap();
    ExecutableFixture {
        foundation,
        digests,
        registrations,
    }
}

pub(super) fn empty_foundation() -> ConeLirFoundation {
    ConeLirFoundation::try_new(ConeIdentity::SINGLE_FILE, CanonicalLirFoundation::empty()).unwrap()
}

pub(super) fn image_only_digests(foundation: &ConeLirFoundation) -> DigestFinalizationPlanV1 {
    DigestFinalizationPlanV1::new(
        vec![
            DigestNodeV1::new(
                DigestNodeKey::runtime_image(foundation.producer()),
                Vec::new(),
            )
            .unwrap(),
        ],
        foundation,
    )
    .unwrap()
}

fn definition_plan(
    producer: ConeIdentity,
    entity: StrongDefinitionEntity,
    role: StrongDefinitionRole,
) -> scoop_identity::ObjectDefinitionPlanId {
    scoop_identity::ObjectDefinitionPlanId::from_key(
        &ObjectDefinitionPlanKey::strong(producer, entity, role).unwrap(),
    )
    .unwrap()
}

fn primary_atom(
    foundation: &ConeLirFoundation,
    plan: scoop_identity::ObjectDefinitionPlanId,
) -> scoop_identity::ObjectDefinitionAtomId {
    foundation
        .definition_atoms()
        .iter()
        .find(|record| record.key().plan() == plan)
        .unwrap()
        .id()
}

pub(super) fn executable_source_entry() -> ExecutableEntryIdentity {
    let declaration = CborIdentityRecord::from_key(SourceDeclarationKey::function(
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
    ))
    .unwrap();
    ExecutableEntryIdentity::try_new(
        &declaration,
        ExactCallableSignature::new(
            scoop_identity::Effect::Ordinary,
            None,
            Vec::new(),
            unit_exact_type(),
        ),
    )
    .unwrap()
}

fn unit_exact_type() -> PersistentExactTypeId {
    PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(
        scoop_identity::CoreBuiltinNominal::Unit
            .identity_record()
            .id(),
    ))
    .unwrap()
}
