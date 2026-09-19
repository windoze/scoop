use super::*;

#[test]
fn metered_representation_reader_preserves_all_shapes_and_legacy_wire() {
    for (mut fixture, record) in fixtures::cases() {
        let decoded: DecodedNominalRepresentationSupportV1 = parsed(&record);
        assert_eq!(encode(&decoded).unwrap(), encode(&record).unwrap());
        let legacy = decoded.clone().resolve(&mut fixture).unwrap();
        let mut resolver = Counting::new(&mut fixture);
        let mut resources = meter();
        let actual = decoded
            .resolve_metered(&mut resolver, &mut resources, &path())
            .unwrap();
        assert_eq!(actual, legacy);
        assert_eq!(actual, record);
        assert_eq!(resolver.calls[0], "source key");
        assert!(resolver.calls.contains(&"context"));
        assert!(resources.usage().owned_bytes > 0);
        assert!(resources.usage().validation_work_units > 0);
    }
}

#[test]
fn metered_field_resolution_keeps_the_checked_field_role_boundary() {
    let mut fixture = Fixture::new(SourceNominalKind::Class);
    let field = fixture.class_field("stored");
    let decoded: DecodedStructRepresentationFieldV1 = parsed(&field);
    assert!(matches!(
        decoded.resolve_metered(&mut Counting::new(&mut fixture), &mut meter(), &path()),
        Err(MeteredRepresentationFieldResolutionError::Value(
            RepresentationFieldResolutionError::Field(
                RepresentationFieldBuildError::ExpectedStructField
            )
        ))
    ));
}
