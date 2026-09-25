use super::tables::{SourceTables, empty_shapes};
use super::*;
use crate::cross_cone_type_bridge::objects::tests::support::Fixture as ObjectFixture;

fn exports(fixture: &ObjectFixture) -> MirTypeBridgeExportConstituentsV1 {
    let object = fixture.object(0);
    let types = CanonicalParamFreeMirTypeExportsV1::try_new(
        fixture
            .types
            .records()
            .iter()
            .filter(|record| [object.backing(), object.read().object()].contains(&record.exact()))
            .cloned()
            .collect(),
    )
    .unwrap();
    let callables = CanonicalMirCallableBindingsV1::try_new(vec![
        fixture.callables.get(object.ensure()).unwrap().clone(),
    ])
    .unwrap();
    let dispatch = CanonicalMirDispatchSchemasV1::try_new(
        MirDispatchSchemaAuthority {
            identities: &fixture.graph,
            types: &fixture.types,
            callables: &fixture.callables,
        },
        vec![],
    )
    .unwrap();
    let shapes = empty_shapes(&fixture.graph, &types);
    MirTypeBridgeExportConstituentsV1::new(
        types,
        callables,
        dispatch,
        CanonicalMirObjectValuesV1::try_new(vec![object]).unwrap(),
        shapes,
        CanonicalMirExternalInitializationUsesV1::try_new(vec![
            fixture
                .use_for(
                    1,
                    MirExternalInitializationCauseV1::ObjectValue(fixture.values[1].id()),
                )
                .unwrap(),
        ])
        .unwrap(),
    )
}

#[test]
fn source_join_keeps_object_backing_ensure_and_foreign_initialization_use() {
    let fixture = ObjectFixture::new();
    let source = SourceTables::new(exports(&ObjectFixture::new()));
    let candidate = exports(&fixture);
    source.check(&candidate, &fixture.graph).unwrap();
    assert_eq!(candidate.types().records().len(), 2);
    assert_eq!(candidate.objects().records().len(), 1);
    assert_eq!(candidate.initialization_uses().records().len(), 1);
}

#[test]
fn source_object_lookup_must_return_the_complete_requested_record() {
    let fixture = ObjectFixture::new();
    let mut source = SourceTables::new(exports(&ObjectFixture::new()));
    source.object_override = Some(fixture.object(1));
    assert!(matches!(source.check(&exports(&fixture), &fixture.graph),
        Err(MirTypeBridgeSourceJoinError::Record(MirTypeBridgeSourceRecordV1::Object(value)))
        if value == fixture.values[0].id()
    ));
}

#[test]
fn source_join_rejects_missing_or_changed_committed_initialization_edges() {
    let fixture = ObjectFixture::new();
    let source = SourceTables::new(exports(&ObjectFixture::new()));
    for records in [
        vec![],
        vec![
            fixture
                .use_for(
                    2,
                    MirExternalInitializationCauseV1::PropertyAccessor(fixture.accessors[0].id()),
                )
                .unwrap(),
        ],
    ] {
        let mut candidate = exports(&fixture);
        candidate.initialization_uses =
            CanonicalMirExternalInitializationUsesV1::try_new(records).unwrap();
        assert!(matches!(
            source.check(&candidate, &fixture.graph),
            Err(MirTypeBridgeSourceJoinError::Record(
                MirTypeBridgeSourceRecordV1::InitializationUses
            ))
        ));
    }
}

#[test]
fn foreign_owned_type_cannot_be_reexported_as_local_source() {
    let fixture = ObjectFixture::new();
    let mut source = SourceTables::new(exports(&ObjectFixture::new()));
    let mut candidate = exports(&fixture);
    let records = candidate
        .types()
        .records()
        .iter()
        .cloned()
        .chain([fixture.types.get(fixture.unit_exact).unwrap().clone()])
        .collect();
    candidate.types = CanonicalParamFreeMirTypeExportsV1::try_new(records).unwrap();
    source.types = candidate
        .types()
        .records()
        .iter()
        .map(|record| record.exact())
        .collect();
    assert!(matches!(source.check(&candidate, &fixture.graph),
        Err(MirTypeBridgeSourceJoinError::Ownership(MirTypeBridgeSourceRecordV1::Type(exact)))
        if exact == fixture.unit_exact
    ));
}
