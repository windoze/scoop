use super::*;

#[test]
fn selected_wire_decoder_enforces_cbor_depth_and_identity_leaf_size() {
    let f = Fixture::new();
    let bytes = encode(&f.signature()).unwrap();

    let decoded: DecodedSelectedExternalTypeUseV1 = decode_canonical(&bytes).unwrap();
    assert_eq!(decoded.resolve(&mut f.resolver()).unwrap(), f.signature());
}
