use super::*;

#[test]
fn signature_preserves_exact_signature_wire() {
    let mut fixture = Fixture::new();
    let exact = ExactCallableSignature::new(
        Effect::Ordinary,
        Some(fixture.value),
        vec![fixture.unit; 64],
        fixture.unit,
    );
    let signature = MirBridgeCallableSignatureV1::new(exact.clone(), crate::GcEffect::NoGc);
    let mut expected = vec![0xa2, 0x01];
    expected.extend(encode(&exact).unwrap());
    expected.extend([0x02, 0xa1, 0x00, 0x02]);
    assert_eq!(encode(&signature).unwrap(), expected);
    let decoded: DecodedMirBridgeCallableSignatureV1 = decode_canonical(&expected).unwrap();
    assert_eq!(encode(&decoded).unwrap(), expected);

    assert_eq!(decoded.resolve(&mut fixture.graph).unwrap(), signature);
}
