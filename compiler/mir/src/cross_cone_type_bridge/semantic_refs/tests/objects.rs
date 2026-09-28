use super::*;
use crate::cross_cone_type_bridge::objects::tests::support::Fixture;

#[test]
fn object_edges_keep_source_backing_ensure_and_unit_separate() {
    let fixture = Fixture::new();
    let record = fixture.object(1);
    let references = MirTypeBridgeSemanticReferencesV1::of_object(&record, &fixture.graph).unwrap();
    assert_eq!(
        references.targets(),
        expected(vec![
            MirTypeBridgeTargetV1::Type(record.read().object()),
            MirTypeBridgeTargetV1::Type(record.backing()),
            MirTypeBridgeTargetV1::Callable(scoop_identity::CallableDefinitionOwner::Strong(
                record.ensure()
            )),
            MirTypeBridgeTargetV1::InitializationUnit(record.unit()),
        ])
    );
    let references = MirTypeBridgeSemanticReferencesV1::of_type(
        fixture.types.get(record.backing()).unwrap(),
        &fixture.graph,
    )
    .unwrap();
    assert_eq!(
        references.targets(),
        &[MirTypeBridgeTargetV1::Type(record.read().object())]
    );
    let references =
        MirTypeBridgeSemanticReferencesV1::of_initialization_unit(record.unit(), &fixture.graph)
            .unwrap();
    assert!(references.targets().is_empty());
    let references = MirTypeBridgeSemanticReferencesV1::of_initialization_contract(
        record.unit(),
        fixture
            .callables
            .get(record.ensure())
            .unwrap()
            .semantic_signature(),
        &fixture.graph,
    )
    .unwrap();
    assert_eq!(
        references.targets(),
        &[MirTypeBridgeTargetV1::Type(fixture.unit_exact)]
    );
}

#[test]
fn initialization_cause_does_not_reclassify_old_accessor_as_new_callable() {
    let fixture = Fixture::new();
    for (dependency, cause) in [
        (
            1,
            MirExternalInitializationCauseV1::ObjectValue(fixture.values[1].id()),
        ),
        (
            2,
            MirExternalInitializationCauseV1::PropertyAccessor(fixture.accessors[0].id()),
        ),
        (
            1,
            MirExternalInitializationCauseV1::InitializationSupport(fixture.units[1].id()),
        ),
    ] {
        let record = fixture.use_for(dependency, cause).unwrap();
        let references =
            MirTypeBridgeSemanticReferencesV1::of_initialization_use(&record, &fixture.graph)
                .unwrap();
        let mut targets = vec![
            MirTypeBridgeTargetV1::InitializationUnit(fixture.units[0].id()),
            MirTypeBridgeTargetV1::InitializationUnit(fixture.units[dependency].id()),
        ];
        if let MirExternalInitializationCauseV1::ObjectValue(value) = cause {
            targets.push(MirTypeBridgeTargetV1::Object(value));
        }
        assert_eq!(references.targets(), expected(targets));
    }
}

#[test]
fn object_ensure_collects_logical_unit_type_and_unit_role() {
    let fixture = Fixture::new();
    let object = fixture.object(1);
    let references = MirTypeBridgeSemanticReferencesV1::of_callable(
        MirCallableRecordRefV1::Lowered(fixture.callables.get(object.ensure()).unwrap()),
        &fixture.graph,
    )
    .unwrap();
    assert_eq!(
        references.targets(),
        expected(vec![
            MirTypeBridgeTargetV1::Type(fixture.unit_exact),
            MirTypeBridgeTargetV1::InitializationUnit(object.unit()),
        ])
    );
}

#[test]
fn generic_initialization_unit_still_requires_the_next_stage() {
    let fixture = Fixture::new();
    let generic = fixture
        .units
        .iter()
        .find(|unit| {
            matches!(
                unit.key(),
                InitializationUnitKey::GenericDelegatedExtensionApplication { .. }
            )
        })
        .unwrap();
    assert!(
        matches!(MirTypeBridgeSemanticReferencesV1::of_initialization_unit(
        generic.id(), &fixture.graph,
    ), Err(MirTypeBridgeReferenceError::GenericUnitGate(unit)) if unit == generic.id())
    );
}
