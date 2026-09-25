use super::*;

#[test]
fn shared_call_signatures_meter_each_occurrence_before_selected_deduplication() {
    let core = Artifact::new(ConeCoordinate::reserved_core()).load(&[]);
    let mut source = Artifact::new(coordinate("provider"));
    let owner = source.nominal("Owner", SourceNominalKind::Class, &[]);
    let mut provider = source.load(&[&core]);
    let member = provider.callable(nominal_owner(owner), "run", CallForm::Function);
    let dependencies = dependencies(&core, &provider);
    let mut consumer = Artifact::new(coordinate_for_consumer()).load(&dependencies);
    consumer.calls(&provider, &[member]);
    let expected = consumer.uses(&dependencies).unwrap();
    let mut single = meter();
    consumer
        .validate(&expected, &dependencies, &mut single)
        .unwrap();
    let mut consumer = Artifact::new(coordinate_for_consumer()).load(&dependencies);
    consumer.calls(&provider, &[member, member]);
    assert_eq!(consumer.uses(&dependencies).unwrap(), expected);
    let mut repeated = meter();
    consumer
        .validate(&expected, &dependencies, &mut repeated)
        .unwrap();
    assert!(repeated.usage().validation_work_units > single.usage().validation_work_units);
    let cost = repeated.usage().validation_work_units;
    let mut bounded = BudgetMeter::new(DecodeLimits {
        validation_work_units: cost,
        ..DecodeLimits::default()
    });
    consumer
        .validate(&expected, &dependencies, &mut bounded)
        .unwrap();
    assert!(
        matches!(consumer.validate(&expected, &dependencies, &mut bounded), Err(Error::Resource(error)) if matches!(error.kind(), WireErrorKind::LimitExceeded { resource: ResourceKind::ValidationWorkUnits, .. }))
    );
    let mut short = BudgetMeter::new(DecodeLimits {
        validation_work_units: cost - 1,
        ..DecodeLimits::default()
    });
    assert!(
        consumer
            .validate(&expected, &dependencies, &mut short)
            .is_err()
    );
    let mut no_edges = BudgetMeter::new(DecodeLimits {
        decoded_edges: 0,
        ..DecodeLimits::default()
    });
    assert!(matches!(
        consumer.validate(&expected, &dependencies, &mut no_edges),
        Err(Error::Resource(_))
    ));
}
