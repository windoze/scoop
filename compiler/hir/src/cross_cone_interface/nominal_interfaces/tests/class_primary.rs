use super::*;

#[test]
fn class_primary_mapping_preserves_plain_parameters_and_typed_properties() {
    let fixture = Fixture::new();
    let mapping = ClassPrimaryConstructorV1::try_new(
        fixture.constructor.id(),
        vec![None, Some(fixture.property.id()), None],
    )
    .unwrap();
    let bytes = encode(&mapping).unwrap();
    let decoded: DecodedClassPrimaryConstructorV1 = decode_canonical(&bytes).unwrap();
    let mut identities = fixture.authority(true);
    assert_eq!(decoded.resolve(&mut identities).unwrap(), mapping);
    assert_eq!(
        mapping.properties(),
        &[None, Some(fixture.property.id()), None]
    );
}

#[test]
fn class_primary_mapping_rejects_a_property_used_by_two_parameters() {
    let fixture = Fixture::new();
    assert_eq!(
        ClassPrimaryConstructorV1::try_new(
            fixture.constructor.id(),
            vec![
                Some(fixture.property.id()),
                None,
                Some(fixture.property.id())
            ],
        ),
        Err(ClassPrimaryConstructorBuildError::DuplicateProperty(
            fixture.property.id()
        ))
    );
}
