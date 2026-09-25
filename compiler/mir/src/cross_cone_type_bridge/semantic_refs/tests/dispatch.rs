use super::*;
use crate::cross_cone_type_bridge::dispatch::tests::support::{
    BASE, DERIVED, DIAMOND, Fixture, LEFT, RIGHT, ROOT, UNIT, VALUE,
};

#[test]
fn class_and_diamond_edges_keep_slot_declaration_and_chosen_target() {
    let fixture = Fixture::new();
    let record = fixture.record(DERIVED);
    let references =
        MirTypeBridgeSemanticReferencesV1::of_dispatch(&record, &fixture.graph, &fixture.types)
            .unwrap();
    let mut targets = [BASE, DERIVED, ROOT, LEFT, RIGHT, DIAMOND, UNIT]
        .map(|index| MirTypeBridgeTargetV1::Type(fixture.exact(index)))
        .to_vec();
    targets.extend(
        [BASE, ROOT, LEFT, RIGHT, DIAMOND]
            .map(|index| MirTypeBridgeTargetV1::Dispatch(fixture.exact(index))),
    );
    targets.extend(
        [0, 1, 3, 4, 5].map(|index| MirTypeBridgeTargetV1::Callable(fixture.target(index))),
    );
    assert_eq!(references.targets(), expected(targets));
}

#[test]
fn boxed_default_preserves_payload_and_both_interface_receivers() {
    let fixture = Fixture::new();
    let target = StrongCallableDefinitionOwner::GeneratedCallable(fixture.boxing[3].id());
    let references = MirTypeBridgeSemanticReferencesV1::of_callable(
        fixture.callables.get(target).unwrap(),
        &fixture.graph,
    )
    .unwrap();
    assert_eq!(
        references.targets(),
        expected(vec![
            MirTypeBridgeTargetV1::Type(fixture.exact(ROOT)),
            MirTypeBridgeTargetV1::Type(fixture.exact(LEFT)),
            MirTypeBridgeTargetV1::Type(fixture.exact(VALUE)),
            MirTypeBridgeTargetV1::Type(fixture.exact(UNIT)),
            MirTypeBridgeTargetV1::Callable(fixture.target(4)),
        ])
    );
}

#[test]
fn abstract_interface_and_value_dispatch_keep_traps_and_real_adjusts() {
    let fixture = Fixture::new();
    for owner in [LEFT, VALUE] {
        let record = fixture.record(owner);
        let references =
            MirTypeBridgeSemanticReferencesV1::of_dispatch(&record, &fixture.graph, &fixture.types)
                .unwrap();
        assert!(
            references
                .targets()
                .contains(&MirTypeBridgeTargetV1::Callable(fixture.target(3)))
        );
        for table in record.itables() {
            for entry in table.entries() {
                assert!(
                    references
                        .targets()
                        .contains(&MirTypeBridgeTargetV1::Callable(
                            entry.implementation().target()
                        ))
                );
            }
        }
    }
}

#[test]
fn owner_type_is_required_even_for_an_empty_schema() {
    let fixture = Fixture::new();
    let empty = CanonicalParamFreeMirTypeExportsV1::try_new(vec![]).unwrap();
    assert!(matches!(MirTypeBridgeSemanticReferencesV1::of_dispatch(
        &fixture.record(BASE), &fixture.graph, &empty,
    ), Err(MirTypeBridgeReferenceError::MissingType(exact)) if exact == fixture.exact(BASE)));
}
