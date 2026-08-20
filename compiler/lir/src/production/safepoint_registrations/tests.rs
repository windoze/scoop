use scoop_identity::{
    ConeIdentity, DigestNodeKey, DigestSemanticFieldRole, LinkageClass, PersistentSymbolKey,
};

use super::{StrongSafepointRegistrationPlanBuildError, StrongSafepointRegistrationPlanSetV1};

mod support;
use support::{Fixture, FixtureOptions};

#[test]
fn joins_every_semantic_site_to_its_registration_and_digest_writers() {
    let fixture = Fixture::new(FixtureOptions::default());
    let plan = fixture.build().unwrap();

    assert_eq!(plan.producer(), ConeIdentity::SINGLE_FILE);
    assert_eq!(plan.registrations().len(), 2);
    assert!(
        plan.registrations()
            .windows(2)
            .all(|pair| pair[0].site() < pair[1].site())
    );
    for registration in plan.registrations() {
        let semantic = fixture
            .semantics
            .sites()
            .iter()
            .find(|site| site.site() == registration.site())
            .unwrap();
        assert_eq!(registration.safepoint(), semantic.safepoint());
        assert_eq!(registration.owner(), semantic.owner());
        assert_eq!(registration.role(), semantic.role());
        assert_eq!(registration.root_pair_count(), semantic.root_pair_count());
        assert_eq!(
            registration.symbol().key(),
            PersistentSymbolKey::SafepointRegistration(registration.site())
        );
        assert_eq!(registration.symbol().linkage(), LinkageClass::ConeStrong);

        assert_eq!(
            fixture
                .digests
                .nodes()
                .iter()
                .find(|node| node.id() == registration.normalized_stackmap_fingerprint_node())
                .unwrap()
                .key(),
            &DigestNodeKey::stackmap_record(registration.site())
        );
    }
}

#[test]
fn rejects_foreign_semantics_before_using_any_registration_identity() {
    let fixture = Fixture::new(FixtureOptions {
        semantic_producer: ConeIdentity::CORE,
        ..FixtureOptions::default()
    });

    assert_eq!(
        fixture.build(),
        Err(
            StrongSafepointRegistrationPlanBuildError::ProducerMismatch {
                foundation: ConeIdentity::SINGLE_FILE,
                semantics: ConeIdentity::CORE,
            }
        )
    );
}

#[test]
fn rejects_missing_registration_coverage() {
    let fixture = Fixture::new(FixtureOptions {
        registration_count: 1,
        ..FixtureOptions::default()
    });

    assert!(matches!(
        fixture.build(),
        Err(StrongSafepointRegistrationPlanBuildError::SafepointSet {
            expected,
            actual,
        }) if expected.len() == 2 && actual.len() == 1
    ));
}

#[test]
fn rejects_missing_runtime_mapping_and_callable_owner() {
    let no_mapping = Fixture::new(FixtureOptions {
        include_runtime_mappings: false,
        ..FixtureOptions::default()
    });
    assert!(matches!(
        no_mapping.build(),
        Err(StrongSafepointRegistrationPlanBuildError::MissingRuntimeMapping { .. })
    ));

    let no_owner = Fixture::new(FixtureOptions {
        include_callable_owner: false,
        ..FixtureOptions::default()
    });
    assert!(matches!(
        no_owner.build(),
        Err(StrongSafepointRegistrationPlanBuildError::MissingOwner { .. })
    ));
}

#[test]
fn rejects_missing_primary_atom_and_strong_symbol() {
    let no_primary = Fixture::new(FixtureOptions {
        include_primary_atoms: false,

        normalized_stackmap_patch: false,
        ..FixtureOptions::default()
    });
    assert!(matches!(
        no_primary.build(),
        Err(StrongSafepointRegistrationPlanBuildError::PrimaryAtomSet { actual, .. })
            if actual.is_empty()
    ));

    let no_symbol = Fixture::new(FixtureOptions {
        include_symbols: false,
        ..FixtureOptions::default()
    });
    assert!(matches!(
        no_symbol.build(),
        Err(StrongSafepointRegistrationPlanBuildError::MissingSymbol(_))
    ));
}

#[test]
fn rejects_missing_stackmap_digest_node() {
    let no_stackmap = Fixture::new(FixtureOptions {
        include_stackmap_nodes: false,

        normalized_stackmap_patch: false,
        ..FixtureOptions::default()
    });
    assert!(matches!(
        no_stackmap.build(),
        Err(StrongSafepointRegistrationPlanBuildError::MissingDigestNode(key))
            if matches!(key.owner_and_role(), scoop_identity::DigestOwnerAndRoleKey::StackmapRecord(_))
    ));
}

#[test]
fn requires_the_normalized_stackmap_patch_writer() {
    let no_stackmap_patch = Fixture::new(FixtureOptions {
        normalized_stackmap_patch: false,
        ..FixtureOptions::default()
    });
    assert!(matches!(
        no_stackmap_patch.build(),
        Err(StrongSafepointRegistrationPlanBuildError::PatchSet {
            expected,
            actual,
            ..
        }) if expected.semantic_field_role() == DigestSemanticFieldRole::NormalizedStackmap
            && actual.is_empty()
    ));
}
