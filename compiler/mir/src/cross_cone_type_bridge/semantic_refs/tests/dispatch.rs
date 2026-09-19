use super::*;
use crate::cross_cone_type_bridge::dispatch::tests::support::{
    BASE, DERIVED, DIAMOND, Fixture, LEFT, RIGHT, ROOT, UNIT, VALUE,
};

#[test]
fn class_and_diamond_edges_keep_slot_declaration_and_chosen_target() {
    let fixture = Fixture::new();
    let record = fixture.record(DERIVED);
    let references = MirTypeBridgeSemanticReferencesV1::of_dispatch(
        &record,
        &fixture.graph,
        &fixture.types,
        &mut meter(),
    )
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
        &mut meter(),
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
        let references = MirTypeBridgeSemanticReferencesV1::of_dispatch(
            &record,
            &fixture.graph,
            &fixture.types,
            &mut meter(),
        )
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
        &fixture.record(BASE), &fixture.graph, &empty, &mut meter(),
    ), Err(MirTypeBridgeReferenceError::MissingType(exact)) if exact == fixture.exact(BASE)));
}

#[test]
fn dependency_collection_uses_one_inclusive_shared_budget() {
    let fixture = Fixture::new();
    let record = fixture.record(DERIVED);
    let mut full = meter();
    let first = MirTypeBridgeSemanticReferencesV1::of_dispatch(
        &record,
        &fixture.graph,
        &fixture.types,
        &mut full,
    )
    .unwrap();
    let usage = full.usage();
    let mut limited = BudgetMeter::new(DecodeLimits {
        validation_work_units: usage.validation_work_units,
        logical_heap_bytes: usage.logical_heap_bytes,
        decoded_nodes: usage.decoded_nodes,
        decoded_edges: usage.decoded_edges,
        ..DecodeLimits::default()
    });
    assert_eq!(
        MirTypeBridgeSemanticReferencesV1::of_dispatch(
            &record,
            &fixture.graph,
            &fixture.types,
            &mut limited,
        )
        .unwrap(),
        first
    );
    assert!(matches!(
        MirTypeBridgeSemanticReferencesV1::of_dispatch(
            &record,
            &fixture.graph,
            &fixture.types,
            &mut limited,
        ),
        Err(MirTypeBridgeReferenceError::Resource(_))
    ));
}
