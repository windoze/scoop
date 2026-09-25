use super::*;
use scoop_identity::{PendingIdentityValidation, ValidatedIdentityGraph};
use scoop_wire::{decode_canonical, encode};

fn empty() -> TypeFoundationSourceAuthorityV1 {
    TypeFoundationSourceAuthorityV1::try_new(TypeFoundationSourceEntriesV1 {
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
        fact_shapes: CanonicalExactTypeFactShapesV1::try_new(vec![]).unwrap(),
        representation_owners: CanonicalPersistentIdsV1::empty(),
    })
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
    let decoded: DecodedTypeFoundationSourceAuthorityV1 = decode_canonical(&expected).unwrap();
    assert_eq!(encode(&decoded).unwrap(), expected);
    assert_eq!(decoded.resolve(&mut identities()).unwrap(), source);
}

#[test]
fn foundation_wire_rejects_missing_extra_and_wrong_field_products() {
    let original = encode(&empty()).unwrap();
    for header in [0xa0, 0xac, 0xae] {
        let mut bytes = original.clone();
        bytes[0] = header;
        assert!(decode_canonical::<DecodedTypeFoundationSourceAuthorityV1>(&bytes).is_err());
    }
    let mut bytes = original;
    let length = bytes.len();
    bytes[length - 2] = 14;
    assert!(decode_canonical::<DecodedTypeFoundationSourceAuthorityV1>(&bytes).is_err());
}
