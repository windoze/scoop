use super::*;
use crate::expression_test_support::Fixture;
use scoop_wire::{DecodeLimits, ResourceKind, WireErrorKind, decode_canonical, encode};

fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
}
fn decoded(domain: &SourceAccessDomainV1) -> DecodedSourceAccessDomainV1 {
    decode_canonical(&encode(domain).unwrap(), DecodeLimits::default()).unwrap()
}
fn conjunction(fixture: &Fixture) -> SourceAccessDomainV1 {
    SourceAccessDomainV1::from_constraints(
        vec![
            SourceAccessConstraintV1::SubclassesOf(SourceNominalId::Concrete(fixture.type_id)),
            SourceAccessConstraintV1::Cone(ConeIdentity::CORE),
            SourceAccessConstraintV1::File(fixture.origin().origin().source().clone()),
            SourceAccessConstraintV1::LexicalOwner(SourceNominalId::Concrete(fixture.type_id)),
            SourceAccessConstraintV1::Cone(ConeIdentity::CORE),
        ],
        &mut meter(),
        &WirePath::root(),
    )
    .unwrap()
}

#[test]
fn source_domains_have_canonical_empty_universal_and_restricted_forms() {
    assert_eq!(
        encode(&SourceAccessDomainV1::empty()).unwrap(),
        [0xa1, 0, 1]
    );
    assert_eq!(
        encode(&SourceAccessDomainV1::universal()).unwrap(),
        [0xa2, 0, 2, 1, 0x80]
    );
    let fixture = Fixture::new();
    let mixed = conjunction(&fixture);
    assert_eq!(mixed.constraints().len(), 4);
    for domain in [
        SourceAccessDomainV1::empty(),
        SourceAccessDomainV1::universal(),
        mixed,
    ] {
        let value = decoded(&domain);
        assert_eq!(encode(&value).unwrap(), encode(&domain).unwrap());
        assert_eq!(
            value.resolve(&mut fixture.resolver(), &mut meter(), &WirePath::root()),
            Ok(domain)
        );
    }
}

#[test]
fn constraint_reader_rejects_reserved_exact_tag_and_unknown_tags() {
    for tag in [0, 4, 6, 23] {
        let error = decode_canonical::<DecodedSourceAccessConstraintV1>(
            &[0xa2, 0, tag, 1, 0],
            DecodeLimits::default(),
        )
        .unwrap_err();
        assert_eq!(error.kind(), &WireErrorKind::UnknownTag { tag: tag as u64 });
    }
    for bytes in [vec![0xa2, 0, 1, 1, 0x80], vec![0xa1, 0, 2]] {
        assert!(matches!(
            decode_canonical::<DecodedSourceAccessDomainV1>(&bytes, DecodeLimits::default())
                .unwrap_err()
                .kind(),
            WireErrorKind::InvalidLength { .. }
        ));
    }
}

#[test]
fn source_domain_resolution_rejects_duplicates_reordering_and_wrong_identities() {
    let fixture = Fixture::new();
    let DecodedSourceAccessDomainV1::Conjunction(constraints) = decoded(&conjunction(&fixture))
    else {
        panic!("conjunction")
    };
    let duplicate = DecodedSourceAccessDomainV1::Conjunction(vec![
        constraints[0].clone(),
        constraints[0].clone(),
    ]);
    assert!(matches!(
        duplicate.resolve(&mut fixture.resolver(), &mut meter(), &WirePath::root()),
        Err(SourceAccessDomainResolutionError::Duplicate { index: 1 })
    ));
    let reversed =
        DecodedSourceAccessDomainV1::Conjunction(constraints.into_iter().rev().collect());
    assert!(matches!(
        reversed.resolve(&mut fixture.resolver(), &mut meter(), &WirePath::root()),
        Err(SourceAccessDomainResolutionError::NonCanonicalOrder { index: 1 })
    ));
    assert!(matches!(
        decoded(&conjunction(&fixture)).resolve(
            &mut crate::expression_test_support::Resolver::rejecting(),
            &mut meter(),
            &WirePath::root()
        ),
        Err(SourceAccessDomainResolutionError::Identity(_))
    ));
}

#[test]
fn source_domain_resolution_consumes_one_cumulative_budget() {
    let fixture = Fixture::new();
    let domain = conjunction(&fixture);
    let path = WirePath::root().field(7);
    let mut baseline = meter();
    decoded(&domain)
        .resolve(&mut fixture.resolver(), &mut baseline, &path)
        .unwrap();
    let work = baseline.usage().validation_work_units;
    let mut shared = BudgetMeter::new(DecodeLimits {
        validation_work_units: work * 2 - 1,
        ..DecodeLimits::default()
    });
    decoded(&domain)
        .resolve(&mut fixture.resolver(), &mut shared, &path)
        .unwrap();
    let error = decoded(&domain)
        .resolve(&mut fixture.resolver(), &mut shared, &path)
        .unwrap_err();
    let SourceAccessDomainResolutionError::Resource(error) = error else {
        panic!("resource error")
    };
    assert!(matches!(
        error.kind(),
        WireErrorKind::LimitExceeded {
            resource: ResourceKind::ValidationWorkUnits,
            ..
        }
    ));
    assert_eq!(error.path(), &path);
    for limits in [
        DecodeLimits {
            semantic_table_entries: 0,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            logical_heap_bytes: 0,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            semantic_leaf_bytes: 0,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            owned_bytes: 0,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            decoded_nodes: 0,
            ..DecodeLimits::default()
        },
    ] {
        assert!(matches!(
            decoded(&domain).resolve(
                &mut fixture.resolver(),
                &mut BudgetMeter::new(limits),
                &path
            ),
            Err(SourceAccessDomainResolutionError::Resource(_))
        ));
    }
}
