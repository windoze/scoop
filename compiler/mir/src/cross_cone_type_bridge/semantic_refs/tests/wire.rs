use super::*;
use crate::cross_cone_type_bridge::objects::tests::support::Fixture;

#[test]
fn six_target_kinds_have_distinct_fixed_wire_and_typed_resolution() {
    let mut fixture = Fixture::new();
    let object = fixture.object(1);
    let values = [
        MirTypeBridgeTargetV1::Type(object.read().object()),
        MirTypeBridgeTargetV1::Callable(scoop_identity::CallableDefinitionOwner::Strong(
            object.ensure(),
        )),
        MirTypeBridgeTargetV1::Dispatch(object.read().object()),
        MirTypeBridgeTargetV1::Object(object.value()),
        MirTypeBridgeTargetV1::ShapeSupport(fixture.objects[1].id()),
        MirTypeBridgeTargetV1::InitializationUnit(object.unit()),
    ];
    for (index, value) in values.into_iter().enumerate() {
        let bytes = encode(&value).unwrap();
        assert_eq!(bytes[..4], [0xa2, 0, index as u8 + 1, 1]);
        let decoded: DecodedMirTypeBridgeTargetV1 = decode_canonical(&bytes).unwrap();
        assert_eq!(encode(&decoded).unwrap(), bytes);
        assert_eq!(decoded.resolve(&mut fixture.graph).unwrap(), value);
    }
}

#[test]
fn target_reader_rejects_unknown_tags_fields_and_wrong_identity_kind() {
    let mut fixture = Fixture::new();
    let object = fixture.object(1);
    let original = encode(&MirTypeBridgeTargetV1::Object(object.value())).unwrap();
    for prefix in [[0xa1, 0, 4], [0xa3, 0, 4], [0xa2, 0, 7]] {
        let mut bytes = original.clone();
        bytes[..3].copy_from_slice(&prefix);
        assert!(decode_canonical::<DecodedMirTypeBridgeTargetV1>(&bytes).is_err());
    }
    let mut wrong = original;
    wrong[2] = 1;
    let decoded: DecodedMirTypeBridgeTargetV1 = decode_canonical(&wrong).unwrap();
    assert!(matches!(
        decoded.resolve(&mut fixture.graph),
        Err(MirTypeBridgeReferenceError::Identity(_))
    ));
}
