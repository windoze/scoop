use super::*;

#[test]
fn source_inventory_rejects_duplicate_and_out_of_order_owner_records() {
    let mut bundle = fixture();
    let source = source_inventory(&bundle);
    let first = &source.records()[0];
    let second = &source.records()[1];
    assert!(matches!(
        CanonicalSourceInheritanceInventoriesV1::try_new(vec![first.clone(), first.clone()]),
        Err(SourceInventoryError::NonCanonicalOrder { index: 1, .. })
    ));
    for pair in [[first, first], [second, first]] {
        let bytes = [
            vec![0x82],
            encode(pair[0]).unwrap(),
            encode(pair[1]).unwrap(),
        ]
        .concat();
        let decoded: DecodedCanonicalSourceInheritanceInventoriesV1 =
            decode_canonical(&bytes).unwrap();
        assert!(matches!(
            decoded.resolve(&mut bundle.fixture),
            Err(SourceInventoryError::NonCanonicalOrder { index: 1, .. })
        ));
    }
}

#[test]
fn constructors_are_not_accepted_as_protected_members_in_source_or_wire() {
    let mut bundle = fixture();
    let source = source_inventory(&bundle);
    let base = source.get(bundle.base.exact).unwrap();
    let constructor = base.constructors().values()[0];
    let members =
        CanonicalProtectedDeclarationRefsV1::try_new(vec![ProtectedDeclarationRefV1::Constructor(
            constructor,
        )])
        .unwrap();
    assert!(matches!(
        SourceInheritanceInventoryV1::try_new(
            base.owner(),
            base.constructors().clone(),
            members.clone(),
            base.slot_schemas().clone()
        ),
        Err(SourceInventoryError::ConstructorInMembers { .. })
    ));
    let bytes = [
        vec![0xa4, 1],
        encode(&base.owner()).unwrap(),
        vec![2],
        encode(base.constructors()).unwrap(),
        vec![3],
        encode(&members).unwrap(),
        vec![4],
        encode(base.slot_schemas()).unwrap(),
    ]
    .concat();
    let decoded: DecodedSourceInheritanceInventoryV1 = decode_canonical(&bytes).unwrap();
    assert!(matches!(
        decoded.resolve(&mut bundle.fixture),
        Err(SourceInventoryError::ConstructorInMembers { .. })
    ));
}

#[test]
fn wire_and_resolver_reject_inexact_products_and_unknown_typed_references() {
    let bundle = fixture();
    let source = source_inventory(&bundle);
    let base = source.get(bundle.base.exact).unwrap();
    for header in [0xa3, 0xa5] {
        let mut bytes = encode(base).unwrap();
        bytes[0] = header;
        assert!(decode_canonical::<DecodedSourceInheritanceInventoryV1>(&bytes).is_err());
    }
    for missing in 1..=3 {
        let mut resolver = bundle.fixture.clone();
        match missing {
            1 => resolver.graph.exacts.clear(),
            2 => resolver.declarations.clear(),
            3 => resolver.slots.clear(),
            _ => unreachable!(),
        }
        let decoded: DecodedSourceInheritanceInventoryV1 = decoded(base);
        assert!(matches!(
            decoded.resolve(&mut resolver),
            Err(SourceInventoryError::Reference(_))
        ));
    }
}
