use super::*;

#[test]
fn reader_rejects_duplicate_source_relations_without_canonicalizing_input() {
    let mut fixture = Fixture::default();
    let owner = fixture.add("Owner", SourceNominalKind::Interface);
    let parent = fixture.add("Parent", SourceNominalKind::Interface);
    let slot = fixture.function(owner, "run");
    let ancestor = fixture.function(parent, "run");
    let member = encode(&InterfaceSourceMemberV1::new(
        slot,
        CanonicalPersistentIdsV1::empty(),
    ))
    .unwrap();
    let parent_array = [
        vec![0x82],
        encode(&parent.exact).unwrap(),
        encode(&parent.exact).unwrap(),
    ]
    .concat();
    let member_array = [vec![0x82], member.clone(), member].concat();
    for (parents, members) in [(parent_array, vec![0x80]), (vec![0x80], member_array)] {
        let bytes = [
            vec![0xa3, 1],
            encode(&owner.exact).unwrap(),
            vec![2],
            parents,
            vec![3],
            members,
        ]
        .concat();
        let read: DecodedInterfaceSourceDispatchV1 = decode_canonical(&bytes).unwrap();
        assert!(matches!(
            read.resolve(&mut fixture),
            Err(SourceInventoryError::InvalidInterfaceDispatch { .. })
        ));
    }
    let bytes = [
        vec![0xa3, 1],
        encode(&owner.exact).unwrap(),
        vec![2, 0x80, 3, 0x81, 0xa2, 1],
        encode(&slot).unwrap(),
        vec![2, 0x82],
        encode(&ancestor).unwrap(),
        encode(&ancestor).unwrap(),
    ]
    .concat();
    let read: DecodedInterfaceSourceDispatchV1 = decode_canonical(&bytes).unwrap();
    assert!(matches!(
        read.resolve(&mut fixture),
        Err(SourceInventoryError::Reference(_))
    ));
}
