use super::*;

#[test]
fn inheritance_wire_is_a_strict_flat_eight_field_product() {
    let mut bundle = fixture();
    let record = bundle.table.get(bundle.base.exact).unwrap();
    let bytes = encode(record).unwrap();
    assert_eq!(bytes[0], 0xa8);
    let decoded: DecodedNominalInheritanceInterfaceV1 = decode_canonical(&bytes).unwrap();
    assert_eq!(decoded.resolve(&mut bundle.fixture).unwrap(), *record);
    let mut short = bytes;
    short[0] = 0xa7;
    assert!(decode_canonical::<DecodedNominalInheritanceInterfaceV1>(&short).is_err());
}

#[test]
fn slot_schema_union_is_complete_on_read() {
    for mode in 0..2 {
        let mut bundle = fixture();
        bundle.change(bundle.base, |record| match mode {
            0 => record.slots = CanonicalInheritanceSlotContractsV1::try_new(vec![]).unwrap(),
            1 => record.slot_schemas = CanonicalInheritanceSlotSchemasV1::default(),
            _ => unreachable!(),
        });
        let decoded: DecodedCanonicalNominalInheritanceInterfacesV1 =
            decode_canonical(&encode(&bundle.table).unwrap()).unwrap();
        let expected = InheritanceInterfaceBuildError::SlotClosure;
        assert!(
            matches!(decoded.resolve(&mut bundle.fixture), Err(InheritanceInterfaceResolutionError::Build(actual)) if actual == expected)
        );
    }
}
