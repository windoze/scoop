use super::*;

fn guarded(cold_calls: usize, in_loop: bool) -> mir::Module {
    let mut h = Harness::new();
    let (int, boolean, unit) = (h.int, h.boolean, h.unit);
    let error = h.exception("GuardError");
    let error = h.class_ty(error);
    let mut locals = Arena::new();
    let value = locals.alloc(local("value", int));
    let observe = h.user_fn_full(
        "observe",
        Vec::new(),
        vec![param("value", int, value)],
        unit,
        hir::Body {
            locals,
            statements: Vec::new(),
        },
    );
    let mut locals = Arena::new();
    let fail = locals.alloc(local("fail", boolean));
    let value = locals.alloc(local("value", int));
    let failure = locals.alloc(local("error", error));
    let mut cold = (0..cold_calls)
        .map(|_| expr_stmt(call_typed(observe, vec![local_ref(value, int)], unit)))
        .collect::<Vec<_>>();
    cold.push(stmt(hir::StatementKind::Throw(local_ref(failure, error))));
    let checked = h.user_fn_full(
        "checked",
        Vec::new(),
        vec![
            param("fail", boolean, fail),
            param("value", int, value),
            param("error", error, failure),
        ],
        int,
        hir::Body {
            locals,
            statements: vec![
                stmt(hir::StatementKind::If {
                    cond: local_ref(fail, boolean),
                    then_body: cold,
                    else_body: None,
                }),
                stmt(hir::StatementKind::Return {
                    value: local_ref(value, int),
                }),
            ],
        },
    );
    let mut locals = Arena::new();
    let fail = locals.alloc(local("fail", boolean));
    let value = locals.alloc(local("value", int));
    let failure = locals.alloc(local("error", error));
    let call = call_typed(
        checked,
        vec![
            local_ref(fail, boolean),
            local_ref(value, int),
            local_ref(failure, error),
        ],
        int,
    );
    let statements = if in_loop {
        vec![
            stmt(hir::StatementKind::While {
                loop_kind: hir::LoopKind::While,
                target: hir::LoopId::from_raw(0),
                condition_setup: Vec::new(),
                cond: local_ref(fail, boolean),
                body: vec![expr_stmt(call)],
            }),
            stmt(hir::StatementKind::Return {
                value: local_ref(value, int),
            }),
        ]
    } else {
        vec![stmt(hir::StatementKind::Return { value: call })]
    };
    h.user_fn_full(
        "scope",
        Vec::new(),
        vec![
            param("fail", boolean, fail),
            param("value", int, value),
            param("error", error, failure),
        ],
        int,
        hir::Body { locals, statements },
    );
    let main = h.user_fn(
        "main",
        hir::Body {
            locals: Arena::new(),
            statements: Vec::new(),
        },
    );
    lower(&h.finish(main))
}

fn has_checked_call(module: &mir::Module) -> bool {
    let checked = module
        .functions
        .iter()
        .find(|(_, function)| function.name == "checked")
        .unwrap()
        .0;
    let scope = module
        .functions
        .iter()
        .find(|(_, function)| function.name == "scope")
        .unwrap()
        .1;
    scope.body.blocks.iter().any(|(_, block)| block.statements.iter().any(|statement| {
        matches!(statement.kind, mir::StatementKind::Call(_))
            && matches!(statement_call(statement).0.target, mir::CallTarget::Direct(mir::Callee::User(id)) if id == checked)
    }))
}

#[test]
fn a_cold_failure_branch_does_not_hide_a_small_normal_path() {
    let mut module = guarded(6, false);
    optimize(&mut module);
    assert!(!has_checked_call(&module));
    let scope = module
        .functions
        .iter()
        .find(|(_, function)| function.name == "scope")
        .unwrap()
        .1;
    assert!(
        scope
            .body
            .blocks
            .iter()
            .any(|(_, block)| matches!(block.terminator, mir::Terminator::Throw { .. }))
    );
}

#[test]
fn cold_paths_still_count_toward_actual_code_growth() {
    let mut module = guarded(24, true);
    let report = crate::optimization::run_observed(&mut module, MirOptimizationOptions::RELEASE);
    assert_mir_foundation_projection(&module);
    assert!(has_checked_call(&module));
    let scope = module
        .functions
        .iter()
        .find(|(_, function)| function.name == "scope")
        .unwrap()
        .0;
    assert!(report.events.iter().any(|event| {
        event.site.function == scope
            && matches!(
                event.decision,
                mir::optimization_report::OptimizationDecision::Inline {
                    outcome: mir::optimization_report::InlineOutcome::CallerGrowth,
                    ..
                }
            )
    }));
}
