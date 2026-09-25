use super::*;

#[test]
fn inheritance_wire_is_a_strict_flat_nine_field_product() {
    let mut bundle = fixture();
    let record = bundle.table.get(bundle.base.exact).unwrap();
    let bytes = encode(record).unwrap();
    assert_eq!(bytes[0], 0xa9);
    let decoded: DecodedNominalInheritanceInterfaceV1 = decode_canonical(&bytes).unwrap();
    assert_eq!(decoded.resolve(&mut bundle.fixture).unwrap(), *record);
    let mut short = bytes;
    short[0] = 0xa8;
    assert!(decode_canonical::<DecodedNominalInheritanceInterfaceV1>(&short).is_err());
}

#[test]
fn slot_schema_union_and_nominal_empty_slot_domain_are_required_on_read() {
    for mode in 0..3 {
        let mut bundle = fixture();
        bundle.change(bundle.base, |record| match mode {
            0 => record.slots = CanonicalInheritanceSlotContractsV1::try_new(vec![]).unwrap(),
            1 => record.slot_schemas = CanonicalInheritanceSlotSchemasV1::default(),
            2 => {
                record.domains = NominalAccessDomainsV1::new(
                    record.domains.lookup().clone(),
                    record.domains.inheritance().clone(),
                    PersistentSlotContractDomainV1::new(PersistentAccessDomainV1::universal()),
                )
            }
            _ => unreachable!(),
        });
        let decoded: DecodedCanonicalNominalInheritanceInterfacesV1 =
            decode_canonical(&encode(&bundle.table).unwrap()).unwrap();
        let expected = if mode == 2 {
            InheritanceInterfaceBuildError::NominalSlotDomain
        } else {
            InheritanceInterfaceBuildError::SlotClosure
        };
        assert!(
            matches!(decoded.resolve(&mut bundle.fixture), Err(InheritanceInterfaceResolutionError::Build(actual)) if actual == expected)
        );
    }
}
