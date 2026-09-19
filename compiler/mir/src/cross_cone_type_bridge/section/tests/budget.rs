use super::*;

#[test]
fn section_semantic_work_budget_is_inclusive_and_shared_across_replay() {
    let fixture = Fixture::new("budget");
    let run = |meter: &mut BudgetMeter| {
        CrossConeMirTypeBridgeSectionV1::try_new(
            fixture.authority(),
            fixture.source.exports(),
            &[],
            &fixture.source,
            &fixture.types.graph,
            meter,
        )
    };
    let mut measured = meter();
    run(&mut measured).unwrap();
    let required = measured.usage().validation_work_units;
    let mut exact = BudgetMeter::new(DecodeLimits {
        validation_work_units: required,
        ..DecodeLimits::default()
    });
    run(&mut exact).unwrap();
    assert!(run(&mut exact).is_err());
    let mut short = BudgetMeter::new(DecodeLimits {
        validation_work_units: required - 1,
        ..DecodeLimits::default()
    });
    assert!(run(&mut short).is_err());
}

#[test]
fn closure_rejects_small_node_edge_depth_and_allocation_budgets() {
    let provider = Fixture::new("bounded-provider");
    let mut consumer = Fixture::new("bounded-consumer");
    consumer.source.uses = vec![provider.shape_use()];
    let graph = graph(&[&provider, &consumer]);
    let terminal = provider.section(&[], &graph).unwrap();
    let limits = [
        DecodeLimits {
            decoded_nodes: 1,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            decoded_edges: 1,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            semantic_recursion: 1,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            logical_heap_bytes: 1,
            ..DecodeLimits::default()
        },
    ];
    for limit in limits {
        let mut meter = BudgetMeter::new(limit);
        assert!(
            CrossConeMirTypeBridgeSectionV1::try_new(
                consumer.authority(),
                consumer.source.exports(),
                &[&terminal],
                &consumer.source,
                &graph,
                &mut meter
            )
            .is_err()
        );
    }
}
