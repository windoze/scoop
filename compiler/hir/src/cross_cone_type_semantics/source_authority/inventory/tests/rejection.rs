use super::*;

#[test]
fn source_roots_reject_duplicate_keys() {
    let fixture = Resolver::new();
    let root = fixture.roots()[0];
    assert!(CanonicalSourceNominalIdsV1::try_new(vec![root, root]).is_err());
}

#[test]
fn source_root_reader_rejects_duplicate_keys() {
    let mut fixture = Resolver::new();
    let root = fixture.roots()[0];
    let decoded: DecodedCanonicalSourceNominalIdsV1 =
        decode_canonical(&two_records(&root, &root)).unwrap();
    assert!(matches!(
        decoded.resolve(&mut fixture),
        Err(SourceInventoryError::NonCanonicalOrder { .. })
    ));
}

#[test]
fn source_root_reader_rejects_noncanonical_order() {
    let mut fixture = Resolver::new();
    let roots = fixture.roots();
    let decoded: DecodedCanonicalSourceNominalIdsV1 =
        decode_canonical(&two_records(&roots[1], &roots[0])).unwrap();
    assert!(matches!(
        decoded.resolve(&mut fixture),
        Err(SourceInventoryError::NonCanonicalOrder { .. })
    ));
}

#[test]
fn source_root_wire_rejects_invalid_nominal_tags() {
    assert!(decode_canonical::<DecodedCanonicalSourceNominalIdsV1>(&[0x81, 0xa1, 0, 3]).is_err());
}

#[test]
fn source_root_reader_rejects_unknown_identities() {
    let mut fixture = Resolver::new();
    fixture.reject_references = true;
    let roots = CanonicalSourceNominalIdsV1::try_new(fixture.roots()).unwrap();
    assert!(matches!(
        decoded::<DecodedCanonicalSourceNominalIdsV1>(&roots).resolve(&mut fixture),
        Err(SourceInventoryError::Reference(_))
    ));
}
