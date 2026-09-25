use super::*;

#[test]
fn shared_member_receiver_ancestry_uses_the_original_cumulative_budget() {
    let core = Artifact::new(ConeCoordinate::reserved_core()).load(&[]);
    let mut source = Artifact::new(coordinate("members"));
    let owner = source.nominal("Owner", SourceNominalKind::Class, &[]);
    let middle = source.nominal("Middle", SourceNominalKind::Class, &[owner]);
    let derived = source.nominal("Derived", SourceNominalKind::Class, &[middle]);
    let mut provider = source.load(&[&core]);
    let member = provider.callable(nominal_owner(owner), "run", CallForm::Function);
    let dependencies = dependencies(&core, &provider);
    let mut consumer = Artifact::new(coordinate_for_consumer()).load(&dependencies);
    consumer.calls(&provider, &[member, member]);
    consumer.change_last_receiver(exact(derived));
    let actual = consumer.uses(&dependencies).unwrap();
    let mut measured = meter();
    consumer
        .validate(&actual, &dependencies, &mut measured)
        .unwrap();
    let usage = measured.usage();
    let limits = DecodeLimits {
        validation_work_units: usage.validation_work_units,
        decoded_nodes: usage.decoded_nodes,
        decoded_edges: usage.decoded_edges,
        logical_heap_bytes: usage.logical_heap_bytes,
        ..DecodeLimits::default()
    };
    let mut exact = BudgetMeter::new(limits);
    consumer
        .validate(&actual, &dependencies, &mut exact)
        .unwrap();
    assert!(
        consumer
            .validate(&actual, &dependencies, &mut exact)
            .is_err()
    );
    for limits in [
        DecodeLimits {
            validation_work_units: usage.validation_work_units - 1,
            ..limits
        },
        DecodeLimits {
            decoded_nodes: usage.decoded_nodes - 1,
            ..limits
        },
        DecodeLimits {
            decoded_edges: usage.decoded_edges - 1,
            ..limits
        },
        DecodeLimits {
            logical_heap_bytes: usage.logical_heap_bytes - 1,
            ..limits
        },
        DecodeLimits {
            semantic_recursion: 2,
            ..limits
        },
    ] {
        assert!(
            consumer
                .validate(&actual, &dependencies, &mut BudgetMeter::new(limits))
                .is_err()
        );
    }
}
