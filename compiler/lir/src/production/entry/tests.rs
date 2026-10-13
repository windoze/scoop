use scoop_identity::{
    CallableBodyKey, CanonicalIdentifier, CborIdentityRecord, ConeIdentity, DeclarationScope,
    DefinitionAtomRole, DefinitionAtomSubkey, DefinitionOwnerChain, DigestNodeId, DigestNodeKey,
    DigestPatchIntentKey, DigestSemanticFieldRole, ExactCallableSignature, ExactTypeKey,
    ExecutableEntryIdentity, LinkageClass, MainCallableBodyId, ObjectDefinitionAtomKey,
    ObjectDefinitionPlanKey, PackagePath, PersistentCallableBodyId, PersistentExactTypeId,
    PersistentStaticStorageId, PersistentSymbolKey, PersistentSymbolRequest,
    PersistentSymbolRequestTable, RuntimeIdentityRecord, SourceDeclarationKey,
    SourceDeclarationSite, StaticStorageKey, StrongDefinitionEntity, StrongDefinitionRole,
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
        "a2000201ab01582000769af7cd4a85d98841cd63dabb8e73c4d41bce56f5e225f7295dd27c3f39b602a200010158209714f93e5ddfe8681524ef2048f90a96e0e87dcbc813b55879c836164da02cdd03a4010102a1000103800458201dff58a7007c61d14decc85852d44e40d113b26e96ec4d24b365bcde341966dc0458205a43bee43f27e5c33d012c1129702324d18dd3856d3158b657383cc2d61c9257055820a59ba8328a87a3c09df1111305261ccbca23796630e0ce6dea24e3ae106f48ca0658209b273ab0bbc562dd7f8e8b0487c0e98f4a7d0781b1cb5aa5b6d69c2d8a7f66b1075820d3bd523ea7c4b775508c06e622f76772db6a21fddb406c6d3fe7d1f20a2a89c108a201a2001601582000769af7cd4a85d98841cd63dabb8e73c4d41bce56f5e225f7295dd27c3f39b60201095820ad54491c8dff032d0bc88256fe3ff8bf9594a26a2fca17d7ac9a35800e6fd9f20a58206466f525cab7256ea8a403737efaf5ede20198ceda53c01c2360ffb9d799c9f90b582059bda5beccd290ca707ae5650ffe1d1bd5e1877c1625b4914ddf69c8833e69fe"
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

mod support;
use support::*;

#[test]
fn generated_entry_round_trips_and_cannot_reuse_the_production_gateway() {
    let record = CborIdentityRecord::from_key(scoop_identity::GeneratedCallableKey::TestEntry {
        cone: ConeIdentity::SINGLE_FILE,
    })
    .unwrap();
    let source = executable_source_entry();
    let entry =
        ExecutableEntryIdentity::for_test(&record, source.source_signature().clone()).unwrap();
    let fixture = fixture_for_entry(entry.clone());
    let plan = EntryProductionPlanV1::new(
        fixture.source.clone(),
        &fixture.foundation,
        &fixture.registrations,
        &fixture.digests,
    )
    .unwrap();
    let decoded: DecodedEntryProductionPlanV1 = decode_canonical(&encode(&plan).unwrap()).unwrap();
    let mut pending = scoop_identity::PendingIdentityValidation::new();
    pending
        .register_authority(ConeIdentity::SINGLE_FILE)
        .unwrap();
    pending
        .register_external_canonical_authority(record.clone())
        .unwrap();
    pending
        .register_authority(entry.source_signature().result())
        .unwrap();
    let mut identities = pending.finish().unwrap();
    assert_eq!(
        decoded.link_source(&mut identities).unwrap(),
        fixture.source
    );
    assert_eq!(
        decoded
            .validate(
                fixture.source,
                &fixture.foundation,
                &fixture.registrations,
                &fixture.digests
            )
            .unwrap(),
        plan
    );
    let EntryProductionPlanV1::Executable(plan) = plan else {
        panic!("test entry");
    };
    assert_eq!(
        plan.declaration(),
        scoop_identity::ExecutableEntryDeclaration::Generated(record.id())
    );
    let production = executable_fixture();
    assert_ne!(plan.main(), production.main);
    assert_ne!(plan.gateway(), production.gateway);
    assert_ne!(plan.failure_root(), production.failure_root);
    assert!(matches!(
        EntryProductionPlanV1::new(
            EntryProductionSourceV1::executable(entry),
            &production.foundation,
            &production.registrations,
            &production.digests
        ),
        Err(EntryProductionPlanBuildError::RootGatewaySet { .. })
    ));
}
