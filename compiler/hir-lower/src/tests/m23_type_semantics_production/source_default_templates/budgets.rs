use super::*;
use scoop_wire::{ResourceKind, WireErrorKind};

#[test]
fn source_template_decode_resolve_and_index_consume_the_same_remaining_budget() {
    with_hir_source(SOURCE, |output, _| {
        let value = template(output, "SourceBase.callback", 1);
        let bytes = bytes(&value);
        let mut measured = meter();
        let input: Decoded = decode_canonical_with_meter(&bytes, &mut measured).unwrap();
        let restored = input
            .resolve(&mut identity_closure(output), &mut measured)
            .unwrap();
        restored.index_locals(&mut measured).unwrap();
        let work = measured.usage().validation_work_units;
        let mut shared = BudgetMeter::new(DecodeLimits {
            validation_work_units: work * 2 - 1,
            ..DecodeLimits::default()
        });
        let input: Decoded = decode_canonical_with_meter(&bytes, &mut shared).unwrap();
        let restored = input
            .resolve(&mut identity_closure(output), &mut shared)
            .unwrap();
        restored.index_locals(&mut shared).unwrap();
        let input: Decoded = decode_canonical_with_meter(&bytes, &mut shared).unwrap();
        let restored = input
            .resolve(&mut identity_closure(output), &mut shared)
            .unwrap();
        assert!(restored.index_locals(&mut shared).is_err());
    });
}

#[test]
fn source_template_reader_preflights_all_resource_dimensions() {
    with_hir_source(SOURCE, |output, _| {
        let value = template(output, "SourceBase.callback", 1);
        let bytes = bytes(&value);
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
                semantic_recursion: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                semantic_table_entries: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                semantic_leaf_bytes: 0,
                ..DecodeLimits::default()
            },
        ] {
            let input: Decoded = decode_canonical(&bytes, DecodeLimits::default()).unwrap();
            assert!(matches!(
                input.resolve(&mut identity_closure(output), &mut BudgetMeter::new(limits)),
                Err(hir::DefaultSourceTemplateResolutionError::Resource(_))
            ));
        }
    });
}

#[test]
fn source_template_indexing_checks_depth_and_scratch_allocation_before_building() {
    with_hir_source(SOURCE, |output, _| {
        let value = template(output, "local", 1);
        for (limits, expected) in [
            (
                DecodeLimits {
                    semantic_recursion: 1,
                    ..DecodeLimits::default()
                },
                ResourceKind::SemanticRecursion,
            ),
            (
                DecodeLimits {
                    owned_bytes: 0,
                    ..DecodeLimits::default()
                },
                ResourceKind::OwnedBytes,
            ),
            (
                DecodeLimits {
                    semantic_table_entries: 0,
                    ..DecodeLimits::default()
                },
                ResourceKind::SemanticTableEntries,
            ),
        ] {
            let error = value
                .index_locals(&mut BudgetMeter::new(limits))
                .unwrap_err();
            let hir::DefaultSourceTemplateIndexError::Resource(error) = error else {
                panic!("index tree must fail during preflight")
            };
            assert!(
                matches!(error.kind(), WireErrorKind::LimitExceeded { resource, .. } if *resource == expected)
            );
        }
    });
}
