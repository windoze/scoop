use super::*;

#[test]
fn shared_source_construction_meters_each_call_before_selected_deduplication() {
    let core = Artifact::new(ConeCoordinate::reserved_core()).load(&[]);
    for kind in [
        SourceNominalKind::Class,
        SourceNominalKind::Struct,
        SourceNominalKind::Enum,
    ] {
        let mut source = Artifact::new(coordinate("provider"));
        let owner = source.nominal("Owner", kind, &[]);
        let mut provider = source.load(&[&core]);
        let target = construct(&mut provider, owner, kind);
        let dependencies = dependencies(&core, &provider);
        let mut consumer = Artifact::new(coordinate_for_consumer()).load(&dependencies);
        consumer.declaration_calls(&provider, &[target]);
        let expected = consumer.uses(&dependencies).unwrap();
        let mut single = meter();
        consumer
            .validate(&expected, &dependencies, &mut single)
            .unwrap();
        let mut consumer = Artifact::new(coordinate_for_consumer()).load(&dependencies);
        consumer.declaration_calls(&provider, &[target, target]);
        assert_eq!(consumer.uses(&dependencies).unwrap(), expected);
        let mut repeated = meter();
        consumer
            .validate(&expected, &dependencies, &mut repeated)
            .unwrap();
        let cost = repeated.usage().validation_work_units;
        assert!(cost > single.usage().validation_work_units);
        let mut bounded = BudgetMeter::new(DecodeLimits {
            validation_work_units: cost,
            ..DecodeLimits::default()
        });
        consumer
            .validate(&expected, &dependencies, &mut bounded)
            .unwrap();
        assert!(
            matches!(consumer.validate(&expected, &dependencies, &mut bounded), Err(Error::Resource(error))
            if matches!(error.kind(), WireErrorKind::LimitExceeded { resource: ResourceKind::ValidationWorkUnits, .. }))
        );
        let mut short = BudgetMeter::new(DecodeLimits {
            validation_work_units: cost - 1,
            ..DecodeLimits::default()
        });
        assert!(
            matches!(consumer.validate(&expected, &dependencies, &mut short), Err(Error::Resource(error))
            if matches!(error.kind(), WireErrorKind::LimitExceeded { resource: ResourceKind::ValidationWorkUnits, .. }))
        );
        let mut no_edges = BudgetMeter::new(DecodeLimits {
            decoded_edges: 0,
            ..DecodeLimits::default()
        });
        assert!(matches!(
            consumer.validate(&expected, &dependencies, &mut no_edges),
            Err(Error::Resource(error) | Error::Materialization(NominalMaterializationClosureError::Resource(error)))
                if matches!(error.kind(), WireErrorKind::LimitExceeded { resource: ResourceKind::DecodedEdges, .. })
        ));
    }
}
