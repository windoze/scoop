use super::*;

#[test]
fn nominal_binding_charges_all_shared_resource_dimensions() {
    with_source(DECLARATIONS, |output, _| {
        let mut fixture = Fixture::from_output(output);
        let table = sources(output, &mut fixture);
        let foundation = fixture.bind().unwrap();
        for limits in [
            DecodeLimits {
                semantic_table_entries: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                semantic_recursion: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                decoded_nodes: 0,
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
            DecodeLimits {
                validation_work_units: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                semantic_leaf_bytes: 0,
                ..DecodeLimits::default()
            },
        ] {
            let result = foundation.bind_nominal_sources(&table, &mut BudgetMeter::new(limits));
            assert!(matches!(result, Err(Error::Resource(_))), "{result:?}");
        }
    });
}
