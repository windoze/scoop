use super::*;

#[test]
fn dispatch_join_enforces_shared_resource_limits() {
    with_sources(SOURCE, |fixture, sources, dispatch, core| {
        let foundation = fixture.bind().unwrap();
        let bound = dispatch.bind(&foundation, &mut meter()).unwrap();
        let slots = bound.bind_slot_sources(&mut meter()).unwrap();
        sources.with_bound(&foundation, core, |members, constructors| {
            let parameters = members
                .bind_parameter_protocols(constructors, &sources.protocols, &mut meter())
                .unwrap();
            for limits in [
                DecodeLimits {
                    semantic_table_entries: 0,
                    ..DecodeLimits::default()
                },
                DecodeLimits {
                    logical_heap_bytes: 0,
                    ..DecodeLimits::default()
                },
                DecodeLimits {
                    validation_work_units: 0,
                    ..DecodeLimits::default()
                },
                DecodeLimits {
                    decoded_nodes: 0,
                    ..DecodeLimits::default()
                },
                DecodeLimits {
                    semantic_recursion: 0,
                    ..DecodeLimits::default()
                },
            ] {
                assert!(
                    matches!(
                        parameters.bind_dispatch_sources(&slots, &mut BudgetMeter::new(limits)),
                        Err(Error::Resource(_))
                    ),
                    "{limits:?}"
                );
            }
        });
    });
}
