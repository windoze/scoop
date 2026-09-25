use super::*;

#[test]
fn shared_extension_receiver_relations_share_budget_and_meter_every_call() {
    let (core, mut provider, base, derived) = provider();
    let actual_type =
        provider.register_signature(&function(vec![base.clone(); 32], derived.clone()));
    let extension = provider.extension(
        function(vec![derived; 32], base),
        "accept",
        CallForm::Function,
    );
    let dependencies = dependencies(&core, &provider);
    let mut consumer = Artifact::new(coordinate_for_consumer()).load(&dependencies);
    consumer.calls(&provider, &[extension, extension]);
    consumer.change_last_receiver(actual_type);
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
    ] {
        assert!(
            consumer
                .validate(&actual, &dependencies, &mut BudgetMeter::new(limits))
                .is_err()
        );
    }
    let mut consumer = Artifact::new(coordinate_for_consumer()).load(&dependencies);
    consumer.calls(&provider, &[extension, extension, extension]);
    consumer.change_last_receiver(actual_type);
    let mut repeated = meter();
    consumer
        .validate(&actual, &dependencies, &mut repeated)
        .unwrap();
    assert!(repeated.usage().validation_work_units > usage.validation_work_units);
}
