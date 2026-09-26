use super::*;

#[test]
fn section_reader_requires_closed_products_and_protocol_cardinality() {
    for bytes in [
        vec![0xa2],
        vec![0xa4],
        // A section contains at most one complete protocol product.
        vec![0xa3, 0x02, 0xa1, 0x00, 0x01, 0x03, 0x80, 0x05, 0x82],
        // Retired fields cannot carry an empty or reconstructed wrapper.
        vec![0xa3, 0x02, 0xa1, 0x00, 0x01, 0x03, 0x80, 0x04, 0x80],
        vec![0xa3, 0x02, 0xa1, 0x00, 0x01, 0x03, 0x80, 0x04, 0x81, 0xa2],
        vec![
            0xa3, 0x01, 0xa1, 0x00, 0x01, 0x02, 0xa1, 0x00, 0x01, 0x03, 0x80,
        ],
        // Field 5 accepts the protocol product itself, not the retired wrapper.
        vec![0xa3, 0x02, 0xa1, 0x00, 0x01, 0x03, 0x80, 0x05, 0x81, 0xa2],
    ] {
        assert!(decode_canonical::<DecodedCoreBootstrapInterfaceSectionV1>(&bytes).is_err());
    }
}

#[test]
fn section_without_local_protocol_definitions_has_a_fixed_wire_vector() {
    let section = non_core_section();
    assert_eq!(hex(&encode(&section).unwrap()), "a302a1000103800580");
    assert_eq!(
        decode_section(&section)
            .validate_against(ConeIdentity::SINGLE_FILE, &CanonicalHirFoundation::empty()),
        Ok(section)
    );
}
