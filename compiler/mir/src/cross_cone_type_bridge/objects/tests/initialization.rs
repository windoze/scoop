use super::*;

#[test]
fn explicit_external_initialization_uses_keep_all_typed_causes() {
    let fixture = Fixture::new();
    let object = fixture
        .use_for(
            1,
            MirExternalInitializationCauseV1::ObjectValue(fixture.values[1].id()),
        )
        .unwrap();
    let property = fixture
        .use_for(
            2,
            MirExternalInitializationCauseV1::PropertyAccessor(fixture.accessors[0].id()),
        )
        .unwrap();
    let member = fixture
        .use_for(
            1,
            MirExternalInitializationCauseV1::PropertyAccessor(fixture.accessors[1].id()),
        )
        .unwrap();
    let support = fixture
        .use_for(
            1,
            MirExternalInitializationCauseV1::InitializationSupport(fixture.units[1].id()),
        )
        .unwrap();
    let table =
        CanonicalMirExternalInitializationUsesV1::try_new(vec![object, property, member, support])
            .unwrap();
    assert_eq!(table.records().len(), 4);
    for record in table.records() {
        assert_eq!(record.local_unit(), fixture.units[0].id());
        assert_eq!(record.provider(), ConeIdentity::CORE);
    }
}

#[test]
fn initialization_cause_cannot_name_another_object_property_or_unit() {
    let fixture = Fixture::new();
    for (dependency, cause) in [
        (
            1,
            MirExternalInitializationCauseV1::ObjectValue(fixture.values[0].id()),
        ),
        (
            1,
            MirExternalInitializationCauseV1::PropertyAccessor(fixture.accessors[0].id()),
        ),
        (
            2,
            MirExternalInitializationCauseV1::PropertyAccessor(fixture.accessors[1].id()),
        ),
        (
            1,
            MirExternalInitializationCauseV1::InitializationSupport(fixture.units[2].id()),
        ),
    ] {
        assert!(matches!(
            fixture.use_for(dependency, cause),
            Err(MirObjectBridgeError::CauseUnitMismatch)
        ));
    }
}

#[test]
fn initialization_provider_is_derived_and_local_unit_belongs_to_consumer() {
    let fixture = Fixture::new();
    let cause = MirExternalInitializationCauseV1::InitializationSupport(fixture.units[1].id());
    assert!(matches!(
        SelectedExternalInitializationUseV1::try_new(
            ConeIdentity::SINGLE_FILE,
            &fixture.graph,
            fixture.units[1].id(),
            ConeIdentity::CORE,
            fixture.units[1].id(),
            cause
        ),
        Err(MirObjectBridgeError::LocalUnitOwner { .. })
    ));
    assert!(matches!(
        SelectedExternalInitializationUseV1::try_new(
            ConeIdentity::SINGLE_FILE,
            &fixture.graph,
            fixture.units[0].id(),
            ConeIdentity::SINGLE_FILE,
            fixture.units[1].id(),
            cause
        ),
        Err(MirObjectBridgeError::DependencyProvider { .. })
    ));
    assert!(matches!(
        fixture.use_for(
            3,
            MirExternalInitializationCauseV1::InitializationSupport(fixture.units[3].id())
        ),
        Err(MirObjectBridgeError::UnitIdentity)
    ));
    let record = fixture.use_for(1, cause).unwrap();
    assert!(matches!(
        CanonicalMirExternalInitializationUsesV1::try_new(vec![record, record]),
        Err(MirObjectBridgeError::DuplicateInitializationUse)
    ));
}

#[test]
fn parameter_free_extension_property_units_are_exact_and_generic_units_are_gated() {
    let fixture = Fixture::new();
    let cause = MirExternalInitializationCauseV1::PropertyAccessor(fixture.accessors[2].id());
    let record = fixture.use_for(4, cause).unwrap();
    assert_eq!(record.dependency_unit(), fixture.units[4].id());
    assert!(matches!(
        fixture.use_for(2, cause),
        Err(MirObjectBridgeError::CauseUnitMismatch)
    ));
    assert!(matches!(
        fixture.use_for(
            5,
            MirExternalInitializationCauseV1::InitializationSupport(fixture.units[5].id())
        ),
        Err(MirObjectBridgeError::GenericUnitGate { .. })
    ));
}
