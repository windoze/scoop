use super::tables::{SourceTables, empty_shapes};
use super::*;
use crate::cross_cone_type_bridge::dispatch::tests::support::{
    DERIVED, Fixture as DispatchFixture,
};

fn exports(fixture: &DispatchFixture) -> MirTypeBridgeExportConstituentsV1 {
    MirTypeBridgeExportConstituentsV1::new(
        fixture.types.clone(),
        fixture.callables.clone(),
        fixture.table(),
        CanonicalMirObjectValuesV1::try_new(vec![]).unwrap(),
        empty_shapes(&fixture.graph, &fixture.types),
        CanonicalMirExternalInitializationUsesV1::try_new(vec![]).unwrap(),
    )
}

#[test]
fn source_join_covers_override_default_abstract_and_boxing_bindings() {
    let fixture = DispatchFixture::new();
    let source = SourceTables::new(exports(&DispatchFixture::new()));
    let candidate = exports(&fixture);
    source.check(&candidate, &fixture.graph).unwrap();
    assert_eq!(candidate.callables().entries().len(), 11);
    assert_eq!(candidate.dispatch().records().len(), 8);
}

#[test]
fn same_callable_identity_and_exact_signature_do_not_hide_gc_change() {
    let fixture = DispatchFixture::new();
    let mut source = SourceTables::new(exports(&DispatchFixture::new()));
    let previous = fixture.callables.get(fixture.target(6)).unwrap();
    let signature = MirBridgeCallableSignatureV1::new(
        previous.lowered_signature().exact().clone(),
        crate::GcEffect::Managed,
    );
    let changed = ParamFreeMirCallableBindingV1::try_new(
        MirCallableBridgeAuthority {
            identities: &fixture.graph,
            foundation: &fixture.foundation,
            types: &fixture.types,
        },
        previous.origin().clone(),
        previous.implementation(),
        signature.clone(),
        signature,
        *previous.lowering_role(),
    )
    .unwrap();
    source.expected.callables = CanonicalMirCallableBindingsV1::try_new(
        source
            .expected
            .callables()
            .entries()
            .iter()
            .map(|record| {
                if record.implementation() == changed.implementation() {
                    changed.clone()
                } else {
                    record.clone()
                }
            })
            .collect(),
    )
    .unwrap();
    assert!(matches!(source.check(&exports(&fixture), &fixture.graph),
        Err(MirTypeBridgeSourceJoinError::Record(MirTypeBridgeSourceRecordV1::Callable(target)))
        if target == fixture.target(6)
    ));
}

#[test]
fn independently_valid_dispatch_cannot_change_the_source_override_selection() {
    let fixture = DispatchFixture::new();
    let mut source = SourceTables::new(exports(&DispatchFixture::new()));
    let actual = fixture.record(DERIVED);
    let inherited = ParamFreeMirDispatchSchemaV1::try_new(
        fixture.authority(),
        actual.owner(),
        MirClassVtableSchemaV1::ClassVtable(vec![fixture.virtual_entry(false)]),
        actual.itables().to_vec(),
    )
    .unwrap();
    source.expected.dispatch = CanonicalMirDispatchSchemasV1::try_new(
        fixture.authority(),
        source
            .expected
            .dispatch()
            .records()
            .iter()
            .map(|record| {
                if record.owner() == inherited.owner() {
                    inherited.clone()
                } else {
                    record.clone()
                }
            })
            .collect(),
    )
    .unwrap();
    assert!(matches!(source.check(&exports(&fixture), &fixture.graph),
        Err(MirTypeBridgeSourceJoinError::Record(MirTypeBridgeSourceRecordV1::Dispatch(owner)))
        if owner == fixture.exact(DERIVED)
    ));
}
