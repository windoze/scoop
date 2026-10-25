use super::*;
use mir::optimization_report::{InlineOutcome, OptimizationDecision};

#[test]
fn loop_score_accounts_for_value_parameters_and_results() {
    for parameter_count in [0, 2] {
        let mut h = Harness::new();
        let (unit, boolean, int) = (h.unit, h.boolean, h.int);
        let pair = h.tuple(&[int, int]);
        let observe = h.user_fn(
            "observe",
            hir::Body {
                locals: Arena::new(),
                statements: Vec::new(),
            },
        );
        let mut locals = Arena::new();
        let values = (0..parameter_count)
            .map(|id| locals.alloc(local(&format!("value{id}"), pair)))
            .collect::<Vec<_>>();
        let parameters = values
            .iter()
            .enumerate()
            .map(|(id, &value)| param(&format!("value{id}"), pair, value))
            .collect();
        let result = values.first().map_or_else(
            || {
                expr(
                    hir::ExprKind::TupleLiteral(vec![int_lit(&h, 3), int_lit(&h, 7)]),
                    pair,
                )
            },
            |&value| local_ref(value, pair),
        );
        let mut statements = (0..if parameter_count == 0 { 9 } else { 10 })
            .map(|_| expr_stmt(call_typed(observe, Vec::new(), unit)))
            .collect::<Vec<_>>();
        statements.push(stmt(hir::StatementKind::Return { value: result }));
        let helper = h.user_fn_full(
            "value_call",
            Vec::new(),
            parameters,
            pair,
            hir::Body { locals, statements },
        );
        let mut locals = Arena::new();
        let enter = locals.alloc(local("enter", boolean));
        let input = locals.alloc(local("input", pair));
        h.user_fn_full(
            "scope",
            Vec::new(),
            vec![param("enter", boolean, enter), param("input", pair, input)],
            pair,
            hir::Body {
                locals,
                statements: vec![
                    stmt(hir::StatementKind::While {
                        loop_kind: hir::LoopKind::While,
                        target: hir::LoopId::from_raw(0),
                        condition_setup: Vec::new(),
                        cond: local_ref(enter, boolean),
                        body: vec![expr_stmt(call_typed(
                            helper,
                            (0..parameter_count)
                                .map(|_| local_ref(input, pair))
                                .collect(),
                            pair,
                        ))],
                    }),
                    stmt(hir::StatementKind::Return {
                        value: local_ref(input, pair),
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
        let report =
            crate::optimization::run_observed(&mut module, MirOptimizationOptions::RELEASE);
        assert_mir_foundation_projection(&module);
        assert!(report.events.iter().any(|event| {
            event.site.function == scope
                && matches!(&event.decision, OptimizationDecision::Inline {
                    outcome: InlineOutcome::Inlined, estimate: Some(estimate), ..
                } if estimate.in_loop
                    && estimate.ordinary.execution > if parameter_count == 0 { 36 } else { 40 }
                    && estimate.ordinary.penalty <= estimate.ordinary.weighted_benefit)
        }));
    }
}
