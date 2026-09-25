use super::*;

#[test]
fn complete_callable_products_round_trip_without_collapsing_the_two_signatures() {
    let mut fixture = Fixture::new();
    let constructor = fixture
        .bind(
            MirCallableOriginV1::Constructor(fixture.constructors[0].id()),
            sig(None, vec![], fixture.class),
            sig(Some(fixture.class), vec![], fixture.unit),
            MirCallableLoweringRoleV1::ClassInitializer {
                owner: fixture.class,
            },
        )
        .unwrap();
    let adjust = fixture
        .bind(
            generated(&fixture.boxing),
            sig(Some(fixture.value), vec![], fixture.unit),
            sig(Some(fixture.interface), vec![], fixture.unit),
            MirCallableLoweringRoleV1::BoxingAdjust {
                target: StrongCallableDefinitionOwner::Function(fixture.method.id()),
            },
        )
        .unwrap();
    let expected = CanonicalMirCallableBindingsV1::try_new(vec![
        constructor,
        adjust,
        fixture.method_binding(),
    ])
    .unwrap();
    let bytes = encode(&expected).unwrap();
    let decoded: DecodedCanonicalMirCallableBindingsV1 = decode_canonical(&bytes).unwrap();
    assert_eq!(encode(&decoded).unwrap(), bytes);
    assert_eq!(
        decoded
            .validate(&mut fixture.graph, &fixture.foundation, &fixture.types)
            .unwrap(),
        expected
    );
}

#[test]
fn callable_tables_reject_duplicate_or_reordered_implementations() {
    let mut fixture = Fixture::new();
    let method = fixture.method_binding();
    assert!(matches!(
        CanonicalMirCallableBindingsV1::try_new(vec![method.clone(), method.clone()]),
        Err(MirCallableBridgeError::DuplicateImplementation { .. })
    ));
    let mut bytes = vec![0x82];
    bytes.extend(encode(&method).unwrap());
    bytes.extend(encode(&method).unwrap());
    let decoded: DecodedCanonicalMirCallableBindingsV1 = decode_canonical(&bytes).unwrap();
    assert!(matches!(
        decoded.validate(&mut fixture.graph, &fixture.foundation, &fixture.types),
        Err(MirCallableBridgeError::NonCanonicalBindingOrder { index: 1 })
    ));
    let constructor = fixture
        .bind(
            MirCallableOriginV1::Constructor(fixture.constructors[1].id()),
            sig(None, vec![], fixture.value),
            sig(None, vec![], fixture.value),
            MirCallableLoweringRoleV1::ValueConstructor {
                owner: fixture.value,
            },
        )
        .unwrap();
    let mut bytes = vec![0x82];
    bytes.extend(encode(&constructor).unwrap());
    bytes.extend(encode(&method).unwrap());
    let decoded: DecodedCanonicalMirCallableBindingsV1 = decode_canonical(&bytes).unwrap();
    assert!(matches!(
        decoded.validate(&mut fixture.graph, &fixture.foundation, &fixture.types),
        Err(MirCallableBridgeError::NonCanonicalBindingOrder { index: 1 })
    ));
}

#[test]
fn signature_wire_preserves_gc_effect_and_rejects_unknown_tags() {
    let fixture = Fixture::new();
    let exact = sig(Some(fixture.value), vec![fixture.value], fixture.unit);
    for gc_effect in [crate::GcEffect::Managed, crate::GcEffect::NoGc] {
        let signature = MirBridgeCallableSignatureV1::new(exact.clone(), gc_effect);
        let bytes = encode(&signature).unwrap();
        assert_eq!(bytes[0], 0xa2);
        let decoded: DecodedMirBridgeCallableSignatureV1 = decode_canonical(&bytes).unwrap();
        assert_eq!(encode(&decoded).unwrap(), bytes);
        let mut bad = bytes;
        *bad.last_mut().unwrap() = 99;
        assert!(decode_canonical::<DecodedMirBridgeCallableSignatureV1>(&bad).is_err());
    }
}
