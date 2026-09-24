use super::*;

fn change(
    output: hir::DependencyHirOutput,
    modify: impl FnOnce(&mut hir::concrete::Module),
) -> Result<hir::DependencyHirOutput, hir::DependencyHirOutputError> {
    let witnesses = output.binding_witness_uses().to_vec();
    let (output, selected) = output.into_parts();
    let shape_plan = output.local.materialization().clone();
    let kind = output.local.output_kind().clone();
    let mut module = output.local.into_module();
    modify(&mut module);
    let local = hir::LocalConcreteHirOutput::try_new(module, kind, shape_plan).unwrap();
    hir::DependencyHirOutput::try_new(
        hir::Output::try_new(
            output.export,
            local,
            output.native_boundary_types,
            output.warnings,
        )
        .unwrap(),
        selected,
        witnesses,
    )
}

fn second_call(module: &mut hir::concrete::Module) -> &mut hir::concrete::Expr {
    let (_, function) = module
        .functions
        .iter_mut()
        .find(|(_, function)| function.name == "repeated")
        .unwrap();
    let hir::concrete::FunctionKind::User(body) = &mut function.kind else {
        panic!("source function")
    };
    let hir::concrete::StatementKind::Expr(expression) = &mut body.statements[1].kind else {
        panic!("second call")
    };
    assert!(matches!(
        expression.kind,
        hir::concrete::ExprKind::ImportedDependencyCall { .. }
    ));
    expression
}

fn error(
    result: Result<hir::DependencyHirOutput, hir::DependencyHirOutputError>,
) -> hir::DependencyCallOccurrenceError {
    match result {
        Err(hir::DependencyHirOutputError::CallOccurrence(error)) => error,
        Err(other) => panic!("wrong sealing error: {other}"),
        Ok(_) => panic!("corrupt occurrence was accepted"),
    }
}

#[test]
fn repeated_target_does_not_hide_a_foreign_winner_binding() {
    let foreign = lower(&fixture("routes"));
    let call = foreign
        .committed_dependency_call_occurrences(&mut meter())
        .unwrap()[1];
    let binding = std::sync::Arc::new(call.binding().clone());
    let failure = change(lower(&fixture("standalone")), |module| {
        let hir::concrete::ExprKind::ImportedDependencyCall {
            binding: actual, ..
        } = &mut second_call(module).kind
        else {
            unreachable!()
        };
        *actual = binding;
    });
    assert!(
        matches!(error(failure), hir::DependencyCallOccurrenceError::Binding(position) if position.expression_index == 1)
    );
}

#[test]
fn repeated_target_does_not_hide_invalid_definition_or_evaluation_origin() {
    for definition in [false, true] {
        let failure = change(lower(&fixture("standalone")), |module| {
            let origin = &mut second_call(module).origin;
            if definition {
                origin.definition.file = u32::MAX;
            } else {
                origin.evaluation.file = u32::MAX;
            }
        });
        assert!(
            matches!(error(failure), hir::DependencyCallOccurrenceError::Origin { position, .. } if position.expression_index == 1)
        );
    }
}

#[test]
fn body_occurrences_require_unique_materialization_roots() {
    let failure = change(lower(&fixture("standalone")), |module| {
        let (_, function) = module
            .functions
            .iter()
            .find(|(_, function)| function.name == "repeated")
            .unwrap();
        module.functions.alloc(function.clone());
    });
    assert!(matches!(
        error(failure),
        hir::DependencyCallOccurrenceError::Structure(
            hir::concrete::ExecutableExpressionStructureError::DuplicateRoot(_)
        )
    ));
}

#[test]
fn actual_expressions_cannot_reference_a_missing_callable_or_closure() {
    let missing = la_arena::RawIdx::from_u32(u32::MAX);
    let failure = change(lower(&fixture("standalone")), |module| {
        let hir::concrete::ExprKind::ImportedDependencyCall { callee, .. } =
            &mut second_call(module).kind
        else {
            unreachable!()
        };
        *callee = la_arena::Idx::from_raw(missing);
    });
    assert!(
        matches!(error(failure), hir::DependencyCallOccurrenceError::MissingUse(position) if position.expression_index == 1)
    );
    let failure = change(lower(&fixture("standalone")), |module| {
        second_call(module).kind =
            hir::concrete::ExprKind::Lambda(la_arena::Idx::from_raw(missing));
    });
    assert!(matches!(
        error(failure),
        hir::DependencyCallOccurrenceError::Structure(
            hir::concrete::ExecutableExpressionStructureError::MissingLambda(_)
        )
    ));
}

#[test]
fn occurrence_traversal_and_results_consume_one_budget() {
    let output = lower(&fixture("standalone"));
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
        let error = output
            .committed_dependency_call_occurrences(&mut BudgetMeter::new(limits))
            .unwrap_err();
        let (hir::DependencyCallOccurrenceError::Resource(error)
        | hir::DependencyCallOccurrenceError::Structure(
            hir::concrete::ExecutableExpressionStructureError::Resource(error),
        )) = error
        else {
            panic!("expected a resource rejection")
        };
        assert!(matches!(
            error.kind(),
            scoop_wire::WireErrorKind::LimitExceeded { .. }
        ));
    }
    let mut accumulated = meter();
    output
        .committed_dependency_call_occurrences(&mut accumulated)
        .unwrap();
    let once = accumulated.usage();
    output
        .committed_dependency_call_occurrences(&mut accumulated)
        .unwrap();
    assert!(accumulated.usage().validation_work_units > once.validation_work_units);
    assert!(accumulated.usage().logical_heap_bytes > once.logical_heap_bytes);
    let mut bounded = BudgetMeter::new(DecodeLimits {
        validation_work_units: once.validation_work_units,
        ..DecodeLimits::default()
    });
    output
        .committed_dependency_call_occurrences(&mut bounded)
        .unwrap();
    assert!(
        output
            .committed_dependency_call_occurrences(&mut bounded)
            .is_err()
    );
}
