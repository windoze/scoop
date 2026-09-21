use super::*;

#[test]
fn default_declaration_binding_uses_one_budget_and_rejects_invalid_dependency_routing() {
    for source in [
        SOURCE,
        super::nested_identities::SOURCE,
        super::nested_parents::SOURCE,
        super::dependency_binders::SOURCE,
        super::dependency_binders::COMBINATIONS,
    ] {
        with_sources(source, |output, fixture, sources, core| {
            let table = templates(output);
            let foundation = fixture.bind().unwrap();
            sources.with_bound(&foundation, core, |members, constructors| {
            let parameters = members.bind_parameter_protocols(constructors, &sources.protocols, &mut meter()).unwrap();
            assert!(matches!(parameters.bind_default_declarations(&table, &[&parameters], &mut meter()),
                Err(Error::Origins(error)) if matches!(*error, hir::DefaultSourceOriginBindingError::DependencyOrder(_))));
            for limits in [
                DecodeLimits { validation_work_units: 0, ..DecodeLimits::default() },
                DecodeLimits { decoded_nodes: 0, ..DecodeLimits::default() },
                DecodeLimits { semantic_recursion: 0, ..DecodeLimits::default() },
                DecodeLimits { semantic_table_entries: 0, ..DecodeLimits::default() },
                DecodeLimits { semantic_leaf_bytes: 0, ..DecodeLimits::default() },
            ] {
                assert!(parameters.bind_default_declarations(&table, &[], &mut BudgetMeter::new(limits)).is_err(), "{limits:?}");
            }
            let mut measured = meter();
            parameters.bind_default_declarations(&table, &[], &mut measured).unwrap();
            let work = measured.usage().validation_work_units;
            let mut shared = BudgetMeter::new(DecodeLimits { validation_work_units: work * 2 - 1, ..DecodeLimits::default() });
            parameters.bind_default_declarations(&table, &[], &mut shared).unwrap();
            assert!(parameters.bind_default_declarations(&table, &[], &mut shared).is_err());
        });
        });
    }
}
