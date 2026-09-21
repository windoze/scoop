use super::*;
use hir::DefaultSourceReferenceClosureError as Error;
use scoop_wire::WirePath;

#[test]
fn source_reference_closure_enforces_shared_traversal_and_index_limits() {
    with_template(|template| {
        let path = WirePath::root().field(3);
        for limits in [
            DecodeLimits {
                decoded_nodes: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                decoded_edges: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                semantic_recursion: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                semantic_table_entries: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                owned_bytes: 0,
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
        ] {
            assert!(
                matches!(
                    template.bind_reference_occurrences(&mut BudgetMeter::new(limits), &path),
                    Err(Error::Resource(_))
                ),
                "{limits:?}"
            );
        }
        let mut measured = meter();
        template
            .bind_reference_occurrences(&mut measured, &path)
            .unwrap();
        let mut shared = BudgetMeter::new(DecodeLimits {
            validation_work_units: measured.usage().validation_work_units * 2 - 1,
            ..DecodeLimits::default()
        });
        template
            .bind_reference_occurrences(&mut shared, &path)
            .unwrap();
        assert!(
            matches!(template.bind_reference_occurrences(&mut shared, &path), Err(Error::Resource(error)) if error.path() == &path)
        );
    });
}
