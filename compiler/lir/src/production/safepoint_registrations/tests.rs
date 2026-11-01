use scoop_identity::{ConeIdentity, DigestNodeKey, LinkageClass, PersistentSymbolKey};

use super::{StrongSafepointRegistrationPlanBuildError, StrongSafepointRegistrationPlanSetV1};

mod support;
use support::{Fixture, FixtureOptions};

#[test]
fn joins_every_semantic_site_to_its_registration_and_content_nodes() {
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
fn removed_safepoints_update_digest_lookup_and_retained_input_edges() {
    let mut fixture = Fixture::new(FixtureOptions::default());
    let removed = fixture.digests.nodes()[0].clone();
    let retained = fixture.digests.nodes()[1].clone();
    assert_eq!(removed.kind(), scoop_identity::DigestKind::StackmapRecord);
    assert_eq!(retained.kind(), scoop_identity::DigestKind::StackmapRecord);
    fixture
        .digests
        .remove_safepoint_records(&std::collections::BTreeSet::from([removed.id()]));

    assert_eq!(fixture.digests.nodes().len(), 2);
    assert_eq!(fixture.digests.node(removed.key()), None);
    assert_eq!(fixture.digests.node(retained.key()), Some(&retained));
    let image = fixture
        .digests
        .node(&DigestNodeKey::runtime_image(ConeIdentity::SINGLE_FILE))
        .unwrap();
    assert_eq!(
        image.direct_inputs(),
        &[crate::DigestInputRefV1::from_node(&retained)]
    );
    fixture
        .digests
        .validate_against(&fixture.foundation)
        .unwrap();
    assert!(matches!(
        fixture.build(),
        Err(StrongSafepointRegistrationPlanBuildError::MissingDigestNode(key))
            if &key == removed.key()
    ));
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

        ..FixtureOptions::default()
    });
    assert!(matches!(
        no_stackmap.build(),
        Err(StrongSafepointRegistrationPlanBuildError::MissingDigestNode(key))
            if matches!(key.owner_and_role(), scoop_identity::DigestOwnerAndRoleKey::StackmapRecord(_))
    ));
}
