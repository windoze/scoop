use super::*;

#[test]
fn complete_default_publication_shares_resource_limits_across_calls() {
    with_hir_source(REFERENCES, |output, _| {
        let production =
            produce_cross_cone_type_semantics(output, &public_interface(output)).unwrap();
        let section = production.section();
        let project = |meter: &mut BudgetMeter| {
            hir::CanonicalProtectedDefaultTemplatesV1::from_dependency_hir(
                output,
                section.protected_source_interfaces(),
                section.inheritance(),
                meter,
            )
        };
        let mut measured = meter();
        project(&mut measured).unwrap();
        let limits = DecodeLimits {
            validation_work_units: measured.usage().validation_work_units * 2 - 1,
            ..DecodeLimits::default()
        };
        let mut shared = BudgetMeter::new(limits);
        project(&mut shared).unwrap();
        assert!(project(&mut shared).is_err());
        for limits in [
            DecodeLimits {
                validation_work_units: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                logical_heap_bytes: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                owned_bytes: 0,
                ..DecodeLimits::default()
            },
        ] {
            assert!(project(&mut BudgetMeter::new(limits)).is_err());
        }
    });
}
