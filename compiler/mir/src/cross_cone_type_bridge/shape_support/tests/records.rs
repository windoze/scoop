use super::*;

#[test]
fn zst_scalar_and_reference_families_keep_distinct_box_obligations() {
    for family in [
        Family::value(false),
        Family::value(true),
        Family::reference(true),
    ] {
        let record = family.build().unwrap();
        assert_eq!(record.source(), family.fixture.empty.id());
        assert_eq!(record.exact(), family.fixture.payload.id());
        assert_eq!(record.boxed(), family.boxed);
        assert_eq!(
            record.coroutine_step(),
            family.fixture.step_export().exact()
        );
        assert_eq!(
            record.coroutine_slot(),
            family.fixture.slot_export().exact()
        );
        assert_eq!(record.provider(), ConeIdentity::SINGLE_FILE);
        family
            .table()
            .validate_required_sources(&[record.source()])
            .unwrap();
    }
}

#[test]
fn value_cannot_omit_box_and_reference_cannot_request_one() {
    let mut value = Family::value(false);
    value.boxed = MirBoxedShapeSupportV1::ReferenceNominalRequiresNoBox;
    assert!(matches!(
        value.build(),
        Err(MirShapeSupportError::BoxAvailability { .. })
    ));
    let mut reference = Family::reference(true);
    reference.boxed = MirBoxedShapeSupportV1::Available(reference.fixture.step_export().exact());
    assert!(matches!(
        reference.build(),
        Err(MirShapeSupportError::BoxAvailability { .. })
    ));
}

#[test]
fn helper_gc_is_joined_to_the_source_instead_of_trusting_local_enum_facts() {
    assert!(matches!(
        Family::reference(false).build(),
        Err(MirShapeSupportError::HelperGc { .. })
    ));
}

#[test]
fn missing_helper_and_wrong_role_are_rejected() {
    let mut family = Family::value(false);
    family.boxed = MirBoxedShapeSupportV1::Available(family.fixture.step_export().exact());
    assert!(matches!(
        family.build(),
        Err(MirShapeSupportError::HelperRole { .. })
    ));
    family.boxed = MirBoxedShapeSupportV1::Available(family.fixture.boxed_export().exact());
    family.types = CanonicalParamFreeMirTypeExportsV1::try_new(vec![
        family.fixture.empty_export(),
        family.fixture.boxed_export(),
        family.fixture.step_export(),
    ])
    .unwrap();
    assert!(matches!(
        family.build(),
        Err(MirShapeSupportError::MissingType { .. })
    ));
}

#[test]
fn same_shape_box_from_another_source_cannot_satisfy_the_family() {
    let mut family = Family::value(false);
    let other = Fixture::with_source("AnotherEmpty", SourceNominalKind::Struct);
    let other_boxed = other.boxed_export();
    let mut types = family.types.records().to_vec();
    types.push(other_boxed.clone());
    family.types = CanonicalParamFreeMirTypeExportsV1::try_new(types).unwrap();
    family.boxed = MirBoxedShapeSupportV1::Available(other_boxed.exact());
    assert!(matches!(
        family.build(),
        Err(MirShapeSupportError::HelperRole { exact }) if exact == other_boxed.exact()
    ));
}

#[test]
fn source_identity_cannot_be_replaced_by_an_unrelated_nominal_or_a_helper() {
    let family = Family::value(false);
    for exact in [
        family.fixture.payload.id(),
        family.fixture.boxed_export().exact(),
    ] {
        assert!(matches!(
            ParamFreeMirShapeSupportV1::try_new(
                family.authority(),
                family.fixture.other.id(),
                exact,
                family.boxed,
                family.fixture.step_export().exact(),
                family.fixture.slot_export().exact(),
            ),
            Err(MirShapeSupportError::InvalidSource { .. })
        ));
    }
}

#[test]
fn shape_table_checks_all_providers_and_duplicate_sources() {
    let family = Family::value(false);
    let record = family.build().unwrap();
    let other_provider = scoop_identity::ConeCoordinate::new("test", "other", "1.0.0")
        .unwrap()
        .identity()
        .unwrap();
    assert!(matches!(
        CanonicalMirShapeSupportsV1::try_new(
            other_provider,
            family.authority(),
            vec![record.clone()]
        ),
        Err(MirShapeSupportError::ProviderMismatch { .. })
    ));
    assert!(matches!(
        CanonicalMirShapeSupportsV1::try_new(
            ConeIdentity::CORE,
            family.authority(),
            vec![record.clone()]
        ),
        Err(MirShapeSupportError::ProviderMismatch { .. })
    ));
    let empty_core =
        CanonicalMirShapeSupportsV1::try_new(ConeIdentity::CORE, family.authority(), vec![])
            .unwrap();
    empty_core.validate_required_sources(&[]).unwrap();
    assert!(matches!(
        empty_core.validate_required_sources(&[record.source()]),
        Err(MirShapeSupportError::MissingSource { source }) if source == record.source()
    ));
    assert!(matches!(
        CanonicalMirShapeSupportsV1::try_new(
            ConeIdentity::SINGLE_FILE,
            family.authority(),
            vec![record.clone(), record]
        ),
        Err(MirShapeSupportError::DuplicateSource { .. })
    ));
}

#[test]
fn independently_required_roots_detect_missing_extra_and_noncanonical_sets() {
    let family = Family::value(false);
    let table = family.table();
    assert!(matches!(
        table.validate_required_sources(&[]),
        Err(MirShapeSupportError::UnexpectedSource { .. })
    ));
    let empty =
        CanonicalMirShapeSupportsV1::try_new(ConeIdentity::SINGLE_FILE, family.authority(), vec![])
            .unwrap();
    assert!(matches!(
        empty.validate_required_sources(&[family.fixture.empty.id()]),
        Err(MirShapeSupportError::MissingSource { .. })
    ));
    assert!(matches!(
        table.validate_required_sources(&[family.fixture.empty.id(), family.fixture.empty.id()]),
        Err(MirShapeSupportError::NonCanonicalRequiredSources { .. })
    ));
}
