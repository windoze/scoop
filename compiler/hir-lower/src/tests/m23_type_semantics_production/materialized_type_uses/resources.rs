use super::*;

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
            .materialized_type_closure()
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
