use super::*;
use mir::optimization_report::{InlineHeatSource, InlineOutcome, OptimizationDecision};

#[test]
fn one_large_callee_is_selected_by_heat_and_actual_simplification() {
    let mut h = Harness::new();
    let (int, boolean, unit) = (h.int, h.boolean, h.unit);
    let mut locals = Arena::new();
    let input = locals.alloc(local("input", int));
    let active = locals.alloc(local("active", boolean));
    let value = locals.alloc(local("value", int));
    let mut work = vec![val_decl(value, local_ref(input, int))];
    for _ in 0..55 {
        let updated = integer_binary(
            &mut h,
            hir::IntegerKind::SIGNED_32,
            hir::IntegerBinaryOperator::Add,
            local_ref(value, int),
            local_ref(input, int),
        );
        work.push(stmt(hir::StatementKind::Assign {
            target: hir::AssignTarget::Local(value),
            value: updated,
        }));
    }
    work.push(stmt(hir::StatementKind::Return {
        value: local_ref(value, int),
    }));
    let large = h.user_fn_full(
        "large",
        Vec::new(),
        vec![param("input", int, input), param("active", boolean, active)],
        int,
        hir::Body {
            locals,
            statements: vec![
                stmt(hir::StatementKind::If {
                    cond: local_ref(active, boolean),
                    then_body: work,
                    else_body: None,
                }),
                stmt(hir::StatementKind::Return {
                    value: local_ref(input, int),
                }),
            ],
        },
    );
    for (name, depth, constant) in [
        ("ordinary", 0, None),
        ("hot", 2, None),
        ("folded", 0, Some(false)),
    ] {
        let mut locals = Arena::new();
        let input = locals.alloc(local("input", int));
        let active = locals.alloc(local("active", boolean));
        let argument = constant.map_or_else(|| local_ref(active, boolean), |v| bool_lit(&h, v));
        let mut statements = vec![expr_stmt(call_typed(
            large,
            vec![local_ref(input, int), argument],
            int,
        ))];
        for level in 0..depth {
            statements = vec![stmt(hir::StatementKind::While {
                loop_kind: hir::LoopKind::While,
                target: hir::LoopId::from_raw(level),
                condition_setup: Vec::new(),
                cond: local_ref(active, boolean),
                body: statements,
            })];
        }
        h.user_fn_full(
            name,
            Vec::new(),
            vec![param("input", int, input), param("active", boolean, active)],
            unit,
            hir::Body { locals, statements },
        );
    }
    let main = h.user_fn(
        "main",
        hir::Body {
            locals: Arena::new(),
            statements: Vec::new(),
        },
    );
    let mut module = lower(&h.finish(main));
    let find = |name| {
        module
            .functions
            .iter()
            .find(|(_, f)| f.name == name)
            .unwrap()
            .0
    };
    let (large, ordinary, hot, folded) =
        (find("large"), find("ordinary"), find("hot"), find("folded"));
    let report = crate::optimization::run_observed(&mut module, MirOptimizationOptions::RELEASE);
    assert_mir_foundation_projection(&module);
    for (caller, expected) in [
        (ordinary, InlineOutcome::Profitability),
        (hot, InlineOutcome::Inlined),
        (folded, InlineOutcome::Inlined),
    ] {
        let event = report
            .events
            .iter()
            .find(|event| {
                event.site.function == caller
                    && matches!(&event.decision, OptimizationDecision::Inline {
                target: mir::CallTarget::Direct(mir::Callee::User(id)), ..
            } if *id == large)
            })
            .unwrap();
        let OptimizationDecision::Inline {
            outcome,
            estimate: Some(estimate),
            ..
        } = &event.decision
        else {
            panic!("assessed call")
        };
        assert_eq!(*outcome, expected, "{estimate:?}");
        assert!(estimate.baseline_size > 100);
        if caller == hot {
            assert_eq!(estimate.heat.loop_depth, 2);
            assert!(estimate.ordinary.size > 100);
        } else if caller == folded {
            assert!(estimate.ordinary.size < 12);
            assert!(
                estimate.ordinary.benefit.simplification < 10,
                "untaken arithmetic is not saved runtime work"
            );
        }
    }
}

#[test]
fn a_small_body_is_not_unconditionally_selected_on_a_cold_exit() {
    let mut h = Harness::new();
    let (int, boolean, unit) = (h.int, h.boolean, h.unit);
    let error = h.exception("ColdError");
    let error = h.class_ty(error);
    let mut locals = Arena::new();
    let input = locals.alloc(local("input", int));
    let mut value = local_ref(input, int);
    for _ in 0..8 {
        value = integer_binary(
            &mut h,
            hir::IntegerKind::SIGNED_32,
            hir::IntegerBinaryOperator::Add,
            value,
            local_ref(input, int),
        );
    }
    let small = h.user_fn_full(
        "small",
        Vec::new(),
        vec![param("input", int, input)],
        int,
        hir::Body {
            locals,
            statements: vec![stmt(hir::StatementKind::Return { value })],
        },
    );
    let mut locals = Arena::new();
    let input = locals.alloc(local("input", int));
    let fail = locals.alloc(local("fail", boolean));
    let failure = locals.alloc(local("failure", error));
    let call = expr_stmt(call_typed(small, vec![local_ref(input, int)], int));
    h.user_fn_full(
        "scope",
        Vec::new(),
        vec![
            param("input", int, input),
            param("fail", boolean, fail),
            param("failure", error, failure),
        ],
        unit,
        hir::Body {
            locals,
            statements: vec![
                call.clone(),
                stmt(hir::StatementKind::If {
                    cond: local_ref(fail, boolean),
                    then_body: vec![
                        call,
                        stmt(hir::StatementKind::Throw(local_ref(failure, error))),
                    ],
                    else_body: None,
                }),
            ],
        },
    );
    let main = h.user_fn(
        "main",
        hir::Body {
            locals: Arena::new(),
            statements: Vec::new(),
        },
    );
    let mut module = lower(&h.finish(main));
    let scope = module
        .functions
        .iter()
        .find(|(_, f)| f.name == "scope")
        .unwrap()
        .0;
    let report = crate::optimization::run_observed(&mut module, MirOptimizationOptions::RELEASE);
    assert_mir_foundation_projection(&module);
    let mut hot = false;
    let mut cold = false;
    for event in &report.events {
        if event.site.function == scope
            && let OptimizationDecision::Inline {
                outcome,
                estimate: Some(estimate),
                ..
            } = &event.decision
        {
            assert!(estimate.baseline_size <= 12);
            if matches!(estimate.heat.source, InlineHeatSource::ColdExit) {
                assert_eq!(*outcome, InlineOutcome::Profitability);
                cold = true;
            } else {
                assert_eq!(*outcome, InlineOutcome::Inlined);
                hot = true;
            }
        }
    }
    assert!(hot && cold);
}
