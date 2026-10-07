use scoop_identity::{
    CallableBodyKey, CanonicalIdentifier, CborIdentityRecord, ConeIdentity, DeclarationScope,
    DefinitionAtomRole, DefinitionAtomSubkey, DefinitionOwnerChain, DigestNodeId, DigestNodeKey,
    DigestPatchIntentKey, DigestSemanticFieldRole, ExactOrdinaryNoArgUnitSignature, ExactTypeKey,
    ExecutableSourceEntryIdentity, LinkageClass, MainCallableBodyId, ObjectDefinitionAtomKey,
    ObjectDefinitionPlanKey, PackagePath, PersistentCallableBodyId, PersistentExactTypeId,
    PersistentFunctionId, PersistentStaticStorageId, PersistentSymbolKey, PersistentSymbolRequest,
    PersistentSymbolRequestTable, RuntimeIdentityRecord, SourceDeclarationKey,
    SourceDeclarationSite, StaticStorageKey, StrongCallableDefinitionOwner, StrongDefinitionEntity,
    StrongDefinitionRole,
};
use scoop_wire::{decode_canonical, encode};

use super::{
    DecodedEntryProductionPlanV1, EntryProductionPlanBuildError, EntryProductionPlanV1,
    EntryProductionSourceV1,
};
use crate::{
    CanonicalLirFoundation, ConeLirFoundation, DigestFinalizationPlanV1, DigestNodeV1,
    RegistrationIdentitySurfaceV1,
};

#[test]
fn executable_plan_derives_the_complete_root_surface_and_round_trips() {
    let fixture = executable_fixture();
    let plan = EntryProductionPlanV1::new(
        fixture.source.clone(),
        &fixture.foundation,
        &fixture.registrations,
        &fixture.digests,
    )
    .unwrap();
    let EntryProductionPlanV1::Executable(entry) = &plan else {
        panic!("fixture is executable")
    };
    assert_eq!(entry.root_cone(), ConeIdentity::SINGLE_FILE);
    assert_eq!(entry.declaration(), fixture.declaration);
    assert_eq!(entry.main(), fixture.main);
    assert_eq!(entry.gateway(), fixture.gateway);
    assert_eq!(entry.failure_root(), fixture.failure_root);
    assert_eq!(
        entry.root_descriptor_symbol().linkage(),
        LinkageClass::ConeStrong
    );

    let bytes = encode(&plan).unwrap();
    let decoded: DecodedEntryProductionPlanV1 = decode_canonical(&bytes).unwrap();
    let validated = decoded
        .validate(
            fixture.source,
            &fixture.foundation,
            &fixture.registrations,
            &fixture.digests,
        )
        .unwrap();
    assert_eq!(encode(&validated).unwrap(), bytes);
    assert_eq!(
        hex(&bytes),
        "a2000201ab01582000769af7cd4a85d98841cd63dabb8e73c4d41bce56f5e225f7295dd27c3f39b60258209714f93e5ddfe8681524ef2048f90a96e0e87dcbc813b55879c836164da02cdd03a4010102a1000103800458201dff58a7007c61d14decc85852d44e40d113b26e96ec4d24b365bcde341966dc0458205a43bee43f27e5c33d012c1129702324d18dd3856d3158b657383cc2d61c9257055820a59ba8328a87a3c09df1111305261ccbca23796630e0ce6dea24e3ae106f48ca0658209b273ab0bbc562dd7f8e8b0487c0e98f4a7d0781b1cb5aa5b6d69c2d8a7f66b1075820d3bd523ea7c4b775508c06e622f76772db6a21fddb406c6d3fe7d1f20a2a89c108a201a2001601582000769af7cd4a85d98841cd63dabb8e73c4d41bce56f5e225f7295dd27c3f39b60201095820ad54491c8dff032d0bc88256fe3ff8bf9594a26a2fca17d7ac9a35800e6fd9f20a58206466f525cab7256ea8a403737efaf5ede20198ceda53c01c2360ffb9d799c9f90b582059bda5beccd290ca707ae5650ffe1d1bd5e1877c1625b4914ddf69c8833e69fe"
    );
}

#[test]
fn library_plan_has_a_closed_empty_wire_branch() {
    let foundation = empty_foundation();
    let digests = image_only_digests(&foundation);
    let registrations = RegistrationIdentitySurfaceV1::from_foundation(&foundation).unwrap();
    let plan = EntryProductionPlanV1::new(
        EntryProductionSourceV1::Library,
        &foundation,
        &registrations,
        &digests,
    )
    .unwrap();
    assert_eq!(encode(&plan).unwrap(), b"\xa1\x00\x01");

    let decoded: DecodedEntryProductionPlanV1 = decode_canonical(b"\xa1\x00\x01").unwrap();
    assert_eq!(
        decoded
            .validate(
                EntryProductionSourceV1::Library,
                &foundation,
                &registrations,
                &digests,
            )
            .unwrap(),
        plan
    );
}

#[test]
fn library_rejects_every_executable_root_surface() {
    let fixture = executable_fixture();
    assert!(matches!(
        EntryProductionPlanV1::new(
            EntryProductionSourceV1::Library,
            &fixture.foundation,
            &fixture.registrations,
            &fixture.digests,
        ),
        Err(EntryProductionPlanBuildError::LibraryRootArtifacts { .. })
    ));
}

#[test]
fn executable_requires_the_derived_root_entities() {
    let foundation = empty_foundation();
    let digests = image_only_digests(&foundation);
    let registrations = RegistrationIdentitySurfaceV1::from_foundation(&foundation).unwrap();
    assert!(matches!(
        EntryProductionPlanV1::new(
            EntryProductionSourceV1::executable(executable_source_entry()),
            &foundation,
            &registrations,
            &digests,
        ),
        Err(EntryProductionPlanBuildError::RootGatewaySet { .. })
    ));
}

#[test]
fn reader_rebuilds_the_whole_branch_instead_of_promoting_ids() {
    let fixture = executable_fixture();
    let plan = EntryProductionPlanV1::new(
        fixture.source.clone(),
        &fixture.foundation,
        &fixture.registrations,
        &fixture.digests,
    )
    .unwrap();
    let mut decoded: DecodedEntryProductionPlanV1 =
        decode_canonical(&encode(&plan).unwrap()).unwrap();
    let DecodedEntryProductionPlanV1::Executable(entry) = &mut decoded else {
        panic!("fixture is executable")
    };
    std::mem::swap(&mut entry.main, &mut entry.gateway);

    assert!(
        decoded
            .validate(
                fixture.source,
                &fixture.foundation,
                &fixture.registrations,
                &fixture.digests,
            )
            .is_err()
    );
}

struct ExecutableFixture {
    source: EntryProductionSourceV1,
    declaration: PersistentFunctionId,
    main: MainCallableBodyId,
    gateway: PersistentCallableBodyId,
    failure_root: PersistentStaticStorageId,
    foundation: ConeLirFoundation,
    digests: DigestFinalizationPlanV1,
    registrations: RegistrationIdentitySurfaceV1,
}

fn executable_fixture() -> ExecutableFixture {
    let producer = ConeIdentity::SINGLE_FILE;
    let entry = executable_source_entry();
    let declaration = entry.declaration();
    let main = entry.main();
    let gateway =
        PersistentCallableBodyId::from_key(&CallableBodyKey::root_gateway(producer, main)).unwrap();
    let failure_root = PersistentStaticStorageId::from_key(
        &StaticStorageKey::root_entry_failure_root(producer, main),
    )
    .unwrap();
    let main_body = RuntimeIdentityRecord::from_key(&CallableBodyKey::strong(
        StrongCallableDefinitionOwner::Function(declaration),
    ))
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
    let source_id = DigestNodeId::from_key(&source_key).unwrap();
    let source = DigestNodeV1::new(
        source_key,
        Vec::new(),
        vec![DigestPatchIntentKey::new(
            source_id,
            definition_plan(
                producer,
                StrongDefinitionEntity::root_entry(producer),
                StrongDefinitionRole::RootEntryDescriptor,
            ),
            DefinitionAtomRole::Primary,
            DigestSemanticFieldRole::SourceSignature,
        )],
    )
    .unwrap();
    let gateway_key = DigestNodeKey::object_definition(gateway_atom);
    let gateway_id = DigestNodeId::from_key(&gateway_key).unwrap();
    let gateway_definition_node = DigestNodeV1::new(
        gateway_key,
        Vec::new(),
        vec![DigestPatchIntentKey::new(
            gateway_id,
            definition_plan(
                producer,
                StrongDefinitionEntity::root_entry(producer),
                StrongDefinitionRole::RootEntryDescriptor,
            ),
            DefinitionAtomRole::Primary,
            DigestSemanticFieldRole::GatewayDefinition,
        )],
    )
    .unwrap();
    let mut nodes = vec![source, gateway_definition_node];
    nodes.push(
        DigestNodeV1::new(
            DigestNodeKey::runtime_image(producer),
            Vec::new(),
            Vec::new(),
        )
        .unwrap(),
    );
    let digests = DigestFinalizationPlanV1::new(nodes, &foundation).unwrap();
    let registrations = RegistrationIdentitySurfaceV1::from_foundation(&foundation).unwrap();
    let source = EntryProductionSourceV1::executable(entry);
    ExecutableFixture {
        source,
        declaration,
        main,
        gateway,
        failure_root,
        foundation,
        digests,
        registrations,
    }
}

fn empty_foundation() -> ConeLirFoundation {
    ConeLirFoundation::try_new(ConeIdentity::SINGLE_FILE, CanonicalLirFoundation::empty()).unwrap()
}

fn image_only_digests(foundation: &ConeLirFoundation) -> DigestFinalizationPlanV1 {
    DigestFinalizationPlanV1::new(
        vec![
            DigestNodeV1::new(
                DigestNodeKey::runtime_image(foundation.producer()),
                Vec::new(),
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

fn executable_source_entry() -> ExecutableSourceEntryIdentity {
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
    ExecutableSourceEntryIdentity::try_new(
        &declaration,
        ExactOrdinaryNoArgUnitSignature::new(unit_exact_type()),
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

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
