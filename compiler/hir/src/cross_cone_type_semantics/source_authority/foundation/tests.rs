use super::*;
use scoop_identity::{PendingIdentityValidation, ValidatedIdentityGraph};
use scoop_wire::{DecodeLimits, decode_canonical, encode};

fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
}

fn empty() -> TypeFoundationSourceAuthorityV1 {
    TypeFoundationSourceAuthorityV1::try_new(
        TypeFoundationSourceEntriesV1 {
            provider: ConeIdentity::CORE,
            exact_keys: CanonicalPersistentIdsV1::empty(),
            sources: CanonicalTypeSourceNominalsV1::default(),
            representations: CanonicalNominalRepresentationSupportV1::try_new(vec![]).unwrap(),
            generated_nominals: CanonicalPersistentIdsV1::empty(),
            accessor_keys: CanonicalPersistentIdsV1::empty(),
            definition_sources: CanonicalExportDefinitionSourcesV1::default(),
            source_roots: CanonicalSourceNominalIdsV1::default(),
            local_exact_facts: CanonicalPersistentIdsV1::empty(),
            dependency_facts: CanonicalTypeSectionDependencyFactsV1::default(),
            local_inheritance_edges: CanonicalNominalInheritanceEdgesV1::default(),
            fact_shapes: CanonicalExactTypeFactShapesV1::try_new(vec![], &mut meter()).unwrap(),
            representation_owners: CanonicalPersistentIdsV1::empty(),
        },
        &mut meter(),
    )
    .unwrap()
}

fn identities() -> ValidatedIdentityGraph {
    let mut pending = PendingIdentityValidation::new();
    pending.register_authority(ConeIdentity::CORE).unwrap();
    pending.finish().unwrap()
}

#[test]
fn empty_foundation_has_fixed_thirteen_field_wire_vector() {
    let source = empty();
    let mut expected = [vec![0xad, 1], encode(&ConeIdentity::CORE).unwrap()].concat();
    for field in 2..=13 {
        expected.extend([field, 0x80]);
    }
    assert_eq!(encode(&source).unwrap(), expected);
    let decoded: DecodedTypeFoundationSourceAuthorityV1 =
        decode_canonical(&expected, DecodeLimits::default()).unwrap();
    assert_eq!(encode(&decoded).unwrap(), expected);
    assert_eq!(
        decoded.resolve(&mut identities(), &mut meter()).unwrap(),
        source
    );
}

#[test]
fn foundation_wire_rejects_missing_extra_and_wrong_field_products() {
    let original = encode(&empty()).unwrap();
    for header in [0xa0, 0xac, 0xae] {
        let mut bytes = original.clone();
        bytes[0] = header;
        assert!(
            decode_canonical::<DecodedTypeFoundationSourceAuthorityV1>(
                &bytes,
                DecodeLimits::default()
            )
            .is_err()
        );
    }
    let mut bytes = original;
    let length = bytes.len();
    bytes[length - 2] = 14;
    assert!(
        decode_canonical::<DecodedTypeFoundationSourceAuthorityV1>(&bytes, DecodeLimits::default())
            .is_err()
    );
}

#[test]
fn foundation_resolver_checks_provider_and_shared_budget() {
    let mut source = empty().into_entries();
    source.provider = ConeIdentity::SINGLE_FILE;
    let source = TypeFoundationSourceAuthorityV1::try_new(source, &mut meter()).unwrap();
    let decoded: DecodedTypeFoundationSourceAuthorityV1 =
        decode_canonical(&encode(&source).unwrap(), DecodeLimits::default()).unwrap();
    assert!(matches!(
        decoded.resolve(&mut identities(), &mut meter()),
        Err(TypeFoundationSourceError::Constituent { field: 1, .. })
    ));
    for limits in [
        DecodeLimits {
            decoded_nodes: 0,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            validation_work_units: 0,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            semantic_recursion: 0,
            ..DecodeLimits::default()
        },
    ] {
        let decoded: DecodedTypeFoundationSourceAuthorityV1 =
            decode_canonical(&encode(&empty()).unwrap(), DecodeLimits::default()).unwrap();
        assert!(matches!(
            decoded.resolve(&mut identities(), &mut BudgetMeter::new(limits)),
            Err(TypeFoundationSourceError::Resource(_))
        ));
    }
}

#[test]
fn key_reference_inventory_budget_is_checked_before_provider_lookup() {
    let mut source = empty().into_entries();
    source.provider = ConeIdentity::SINGLE_FILE;
    let exact = PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(
        scoop_identity::CoreBuiltinNominal::Unit
            .identity_record()
            .id(),
    ))
    .unwrap();
    source.exact_keys = CanonicalPersistentIdsV1::try_new(vec![exact]).unwrap();
    let source = TypeFoundationSourceAuthorityV1::try_new(source, &mut meter()).unwrap();
    let bytes = encode(&source).unwrap();
    for limits in [
        DecodeLimits {
            semantic_table_entries: 0,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            validation_work_units: 1,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            decoded_nodes: 1,
            ..DecodeLimits::default()
        },
    ] {
        let decoded: DecodedTypeFoundationSourceAuthorityV1 =
            decode_canonical(&bytes, DecodeLimits::default()).unwrap();
        // The provider is deliberately unknown, so a lookup before preflight
        // would produce a reference error instead of exhausting the budget.
        assert!(matches!(
            decoded.resolve(&mut identities(), &mut BudgetMeter::new(limits)),
            Err(TypeFoundationSourceError::Resource(_))
        ));
    }
}
