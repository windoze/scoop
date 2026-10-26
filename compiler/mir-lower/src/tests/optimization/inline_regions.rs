use super::*;
use mir::optimization_report::{InlineOutcome, OptimizationDecision};

fn nested(enter: hir::Expr, statements: Vec<hir::Statement>, first: u32) -> hir::Statement {
    stmt(hir::StatementKind::While {
        loop_kind: hir::LoopKind::While,
        target: hir::LoopId::from_raw(first),
        condition_setup: Vec::new(),
        cond: enter.clone(),
        body: vec![stmt(hir::StatementKind::While {
            loop_kind: hir::LoopKind::While,
            target: hir::LoopId::from_raw(first + 1),
            condition_setup: Vec::new(),
            cond: enter,
            body: statements,
        })],
    })
}

#[test]
fn loops_keep_independent_growth_with_a_cumulative_caller_bound() {
    for (wrapped, busy_loops) in [(false, 1), (true, 1), (false, 3)] {
        let mut h = Harness::new();
        let (int, boolean, unit) = (h.int, h.boolean, h.unit);
        let mut locals = Arena::new();
        let input = locals.alloc(local("input", int));
        let mut value = local_ref(input, int);
        for _ in 0..12 {
            value = integer_binary(
                &mut h,
                hir::IntegerKind::SIGNED_32,
                hir::IntegerBinaryOperator::Add,
                value,
                local_ref(input, int),
            );
        }
        let step = h.user_fn_full(
            "step",
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
        let enter = locals.alloc(local("enter", boolean));
        let wrapper = h.user_fn_full(
            "wrapper",
            Vec::new(),
            vec![param("input", int, input), param("enter", boolean, enter)],
            unit,
            hir::Body {
                locals,
                statements: vec![nested(
                    local_ref(enter, boolean),
                    vec![expr_stmt(call_typed(
                        step,
                        vec![local_ref(input, int)],
                        int,
                    ))],
                    0,
                )],
            },
        );
        let mut locals = Arena::new();
        let input = locals.alloc(local("input", int));
        let enter = locals.alloc(local("enter", boolean));
        let tail = if wrapped {
            nested(
                local_ref(enter, boolean),
                vec![expr_stmt(call_typed(
                    wrapper,
                    vec![local_ref(input, int), local_ref(enter, boolean)],
                    unit,
                ))],
                2 * busy_loops,
            )
        } else {
            nested(
                local_ref(enter, boolean),
                vec![expr_stmt(call_typed(
                    step,
                    vec![local_ref(input, int)],
                    int,
                ))],
                2 * busy_loops,
            )
        };
        let mut statements = (0..busy_loops)
            .map(|id| {
                nested(
                    local_ref(enter, boolean),
                    (0..256)
                        .map(|_| expr_stmt(call_typed(step, vec![local_ref(input, int)], int)))
                        .collect(),
                    2 * id,
                )
            })
            .collect::<Vec<_>>();
        statements.push(tail);
        h.user_fn_full(
            "scope",
            Vec::new(),
            vec![param("input", int, input), param("enter", boolean, enter)],
            unit,
            hir::Body { locals, statements },
        );
        let main = empty_main(&mut h);
        let mut module = lower(&h.finish(main));
        let find = |name| {
            module
                .functions
                .iter()
                .find(|(_, f)| f.name == name)
                .unwrap()
                .0
        };
        let (scope, step, wrapper) = (find("scope"), find("step"), find("wrapper"));
        let report =
            crate::optimization::run_observed(&mut module, MirOptimizationOptions::RELEASE);
        assert_mir_foundation_projection(&module);
        let mut regions = std::collections::HashSet::new();
        let mut retained = false;
        let mut exposed = false;
        let mut limit = None;
        let mut spent = 0;
        let mut smallest = usize::MAX;
        for event in &report.events {
            if event.site.function == scope
                && let OptimizationDecision::Inline {
                    target: mir::CallTarget::Direct(mir::Callee::User(target)),
                    outcome,
                    estimate,
                } = &event.decision
                && *target == step
            {
                if *outcome == InlineOutcome::Inlined {
                    let estimate = estimate.as_ref().unwrap();
                    assert!(estimate.heat.loop_depth >= 2);
                    regions.insert(estimate.heat.loop_header.unwrap());
                    assert_eq!(
                        estimate.remaining_growth,
                        estimate
                            .region_remaining_growth
                            .min(estimate.caller_remaining_growth)
                    );
                    exposed |= event.site.inline_path.contains(&mir::Callee::User(wrapper));
                    if busy_loops == 3 {
                        let limit = *limit.get_or_insert(estimate.caller_remaining_growth);
                        assert_eq!(estimate.caller_remaining_growth, limit - spent);
                        spent += estimate.ordinary.growth;
                        smallest = smallest.min(estimate.ordinary.growth);
                    }
                } else {
                    retained |= matches!(
                        outcome,
                        InlineOutcome::GrowthExhausted | InlineOutcome::CallerGrowth
                    );
                }
            }
        }
        assert!(
            retained,
            "the busy loop must still exhaust its bounded allowance"
        );
        if busy_loops == 3 {
            assert!(
                limit.unwrap() - spent < smallest,
                "new headers cannot bypass the caller bound"
            );
        } else {
            assert_eq!(
                regions.len(),
                2,
                "a sibling cannot spend the later loop's allowance: wrapped={wrapped}"
            );
        }
        assert_eq!(exposed, wrapped);
    }
}
