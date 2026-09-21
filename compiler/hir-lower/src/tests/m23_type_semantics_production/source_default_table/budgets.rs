use super::*;
use scoop_wire::{ResourceKind, WireErrorKind};
#[test]
fn complete_default_source_production_uses_one_remaining_budget() {
    with_hir_source(SOURCE, |output, _| {
        let mut measured = meter();
        Production::from_dependency_hir(output, &mut measured).unwrap();
        let work = measured.usage().validation_work_units;
        let mut shared = BudgetMeter::new(DecodeLimits {
            validation_work_units: work * 2 - 1,
            ..DecodeLimits::default()
        });
        Production::from_dependency_hir(output, &mut shared).unwrap();
        assert!(Production::from_dependency_hir(output, &mut shared).is_err());
        assert!(shared.usage().validation_work_units >= work);
    });
}
#[test]
fn source_table_resolution_and_indexing_preflight_table_and_allocation_limits() {
    with_hir_source(SOURCE, |output, _| {
        let production = Production::from_dependency_hir(output, &mut meter()).unwrap();
        let value = production.templates();
        for (limits, expected) in [
            (
                DecodeLimits {
                    semantic_table_entries: 0,
                    ..DecodeLimits::default()
                },
                ResourceKind::SemanticTableEntries,
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
                    logical_heap_bytes: 0,
                    ..DecodeLimits::default()
                },
                ResourceKind::LogicalHeapBytes,
            ),
            (
                DecodeLimits {
                    decoded_nodes: 0,
                    ..DecodeLimits::default()
                },
                ResourceKind::DecodedNodes,
            ),
            (
                DecodeLimits {
                    semantic_recursion: 0,
                    ..DecodeLimits::default()
                },
                ResourceKind::SemanticRecursion,
            ),
            (
                DecodeLimits {
                    validation_work_units: 0,
                    ..DecodeLimits::default()
                },
                ResourceKind::ValidationWorkUnits,
            ),
        ] {
            let input: Decoded = decode_canonical(&bytes(value), DecodeLimits::default()).unwrap();
            let error = input
                .resolve(&mut identity_closure(output), &mut BudgetMeter::new(limits))
                .unwrap_err();
            let hir::DefaultSourceTemplateTableResolutionError::Resource(error) = error else {
                panic!("table preflight must reject limits")
            };
            assert!(
                matches!(error.kind(), WireErrorKind::LimitExceeded { resource, .. } if *resource == expected)
            );
            assert!(matches!(
                value.index_locals(&mut BudgetMeter::new(limits)),
                Err(hir::DefaultSourceTemplateTableIndexError::Resource(_))
            ));
        }
        assert!(matches!(
            Table::try_new(
                value.records().to_vec(),
                &mut BudgetMeter::new(DecodeLimits {
                    semantic_table_entries: 0,
                    ..DecodeLimits::default()
                })
            ),
            Err(hir::DefaultSourceTemplateTableBuildError::Resource(_))
        ));
        assert!(matches!(
            value.validate_parameter_coverage(
                production.parameters(),
                &mut BudgetMeter::new(DecodeLimits {
                    validation_work_units: 0,
                    ..DecodeLimits::default()
                })
            ),
            Err(hir::DefaultSourceTemplateCoverageError::Resource(_))
        ));
    });
}
#[test]
fn source_table_reader_shares_budget_across_all_records() {
    with_hir_source(SOURCE, |output, _| {
        let production = Production::from_dependency_hir(output, &mut meter()).unwrap();
        let input: Decoded =
            decode_canonical(&bytes(production.templates()), DecodeLimits::default()).unwrap();
        let mut measured = meter();
        input
            .clone()
            .resolve(&mut identity_closure(output), &mut measured)
            .unwrap();
        let work = measured.usage().validation_work_units;
        let mut shared = BudgetMeter::new(DecodeLimits {
            validation_work_units: work * 2 - 1,
            ..DecodeLimits::default()
        });
        input
            .clone()
            .resolve(&mut identity_closure(output), &mut shared)
            .unwrap();
        assert!(
            input
                .resolve(&mut identity_closure(output), &mut shared)
                .is_err()
        );
    });
}
