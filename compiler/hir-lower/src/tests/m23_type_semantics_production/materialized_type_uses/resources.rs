use super::*;

#[test]
fn materialized_type_query_has_one_cumulative_budget() {
    with_hir_source(&fixture("combined"), |output, _| {
        let local = &output.output().local;
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
                semantic_recursion: 1,
                ..DecodeLimits::default()
            },
        ] {
            assert!(
                local
                    .materialized_type_closure(&mut BudgetMeter::new(limits))
                    .is_err()
            );
        }
        let mut measured = meter();
        local.materialized_type_closure(&mut measured).unwrap();
        let once = measured.usage();
        local.materialized_type_closure(&mut measured).unwrap();
        assert!(measured.usage().validation_work_units > once.validation_work_units);
        assert!(measured.usage().logical_heap_bytes > once.logical_heap_bytes);
        let mut bounded = BudgetMeter::new(DecodeLimits {
            validation_work_units: once.validation_work_units,
            ..DecodeLimits::default()
        });
        local.materialized_type_closure(&mut bounded).unwrap();
        assert!(local.materialized_type_closure(&mut bounded).is_err());
    });
}

#[test]
fn materialized_type_query_rejects_missing_types_and_duplicate_roots() {
    with_hir_source(&fixture("standalone"), |output, _| {
        let local = &output.output().local;
        let corrupt = |duplicate| {
            let mut module = local.module().clone();
            let id = module
                .functions
                .iter()
                .find(|(_, function)| function.name == "flag")
                .unwrap()
                .0;
            if duplicate {
                module.functions.alloc(module.functions[id].clone());
            } else {
                module.functions[id].return_ty = hir::concrete::TypeId::from_raw(u32::MAX.into());
            }
            hir::LocalConcreteHirOutput::try_new(
                module,
                local.output_kind().clone(),
                local.materialization().clone(),
            )
            .unwrap()
            .materialized_type_closure(&mut meter())
            .unwrap_err()
        };
        assert!(matches!(
            corrupt(false),
            hir::MaterializedTypeClosureError::MissingType(_)
        ));
        assert!(matches!(
            corrupt(true),
            hir::MaterializedTypeClosureError::Executable(
                hir::concrete::ExecutableExpressionStructureError::DuplicateRoot(_)
            )
        ));
    });
}
