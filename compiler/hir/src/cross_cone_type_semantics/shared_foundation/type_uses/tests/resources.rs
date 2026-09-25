use super::*;

#[test]
fn shared_type_uses_charge_each_inheritance_edge_and_share_the_artifact_budget() {
    let mut source = Artifact::new(coordinate("provider"));
    let base = source.nominal("Base", SourceNominalKind::Class, &[]);
    let provider = source.load(&[]);
    let mut source = Artifact::new(coordinate("consumer"));
    source.nominal("First", SourceNominalKind::Class, &[base]);
    source.nominal("Second", SourceNominalKind::Class, &[base]);
    let consumer = source.load(&[&provider]);
    let expected = consumer.uses(&[&provider]).unwrap();
    let mut measured = meter();
    consumer
        .validate(&expected, &[&provider], &mut measured)
        .unwrap();
    let mut bounded = BudgetMeter::new(DecodeLimits {
        validation_work_units: measured.usage().validation_work_units,
        ..DecodeLimits::default()
    });
    consumer
        .validate(&expected, &[&provider], &mut bounded)
        .unwrap();
    assert!(
        matches!(consumer.validate(&expected, &[&provider], &mut bounded), Err(Error::Resource(error)) if matches!(error.kind(), WireErrorKind::LimitExceeded { resource: ResourceKind::ValidationWorkUnits, .. }))
    );
    for limits in [
        DecodeLimits {
            validation_work_units: 0,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            decoded_edges: 0,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            owned_bytes: 0,
            ..DecodeLimits::default()
        },
    ] {
        assert!(matches!(
            consumer.validate(&expected, &[&provider], &mut BudgetMeter::new(limits)),
            Err(Error::Resource(_))
        ));
    }
}
