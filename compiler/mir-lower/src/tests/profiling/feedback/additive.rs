use super::*;

fn contention_program(copies: usize, nested: bool) -> hir::ExportHirOutput {
    let mut h = Harness::new();
    let (int, boolean, unit) = (h.int, h.boolean, h.unit);
    let mut callees = Vec::new();
    for (name, operations) in [("baseline", 16), ("extra", 55)] {
        let mut locals = Arena::new();
        let input = locals.alloc(local("input", int));
        let value = locals.alloc(local("value", int));
        let mut statements = vec![val_decl(value, local_ref(input, int))];
        for _ in 0..operations {
            let updated = integer_binary(
                &mut h,
                hir::IntegerKind::SIGNED_32,
                hir::IntegerBinaryOperator::Add,
                local_ref(value, int),
                local_ref(input, int),
            );
            statements.push(stmt(hir::StatementKind::Assign {
                target: hir::AssignTarget::Local(value),
                value: updated,
            }));
        }
        statements.push(stmt(hir::StatementKind::Return {
            value: local_ref(value, int),
        }));
        callees.push(h.user_fn_full(
            name,
            vec![],
            vec![param("input", int, input)],
            int,
            hir::Body { locals, statements },
        ));
    }
    let mut baseline = callees[0];
    if nested {
        let mut locals = Arena::new();
        let input = locals.alloc(local("input", int));
        baseline = h.user_fn_full(
            "wrapper",
            vec![],
            vec![param("input", int, input)],
            int,
            hir::Body {
                locals,
                statements: vec![stmt(hir::StatementKind::Return {
                    value: call_typed(baseline, vec![local_ref(input, int)], int),
                })],
            },
        );
    }
    let mut locals = Arena::new();
    let input = locals.alloc(local("input", int));
    let active = locals.alloc(local("active", boolean));
    let mut body = vec![expr_stmt(call_typed(
        callees[1],
        vec![local_ref(input, int)],
        int,
    ))];
    body.extend(
        (0..copies).map(|_| expr_stmt(call_typed(baseline, vec![local_ref(input, int)], int))),
    );
    h.user_fn_full(
        "scope",
        vec![],
        vec![param("input", int, input), param("active", boolean, active)],
        unit,
        hir::Body {
            locals,
            statements: vec![stmt(hir::StatementKind::While {
                loop_kind: hir::LoopKind::While,
                target: hir::LoopId::from_raw(0),
                condition_setup: vec![],
                cond: local_ref(active, boolean),
                body,
            })],
        },
    );
    let main = h.user_fn(
        "main",
        hir::Body {
            locals: Arena::new(),
            statements: vec![],
        },
    );
    h.finish(main)
}

fn remaining_calls(module: &mir::Module, caller: &str, callee: &str) -> usize {
    let (_, function) = module
        .functions
        .iter()
        .find(|(_, function)| function.name == caller)
        .unwrap();
    function
        .body
        .blocks
        .iter()
        .flat_map(|(_, block)| &block.statements)
        .filter(|statement| {
            matches!(&statement.kind, mir::StatementKind::Call(
                mir::CallEffect::Unit(call) | mir::CallEffect::Value { call, .. })
                if matches!(call.target,
                    mir::CallTarget::Direct(mir::Callee::User(id))
                    if module.functions[id].name == callee))
        })
        .count()
}

#[test]
fn cold_feedback_and_callee_normalization_preserve_default_expansions() {
    for nested in [false, true] {
        let source = contention_program(1, nested);
        let trained = lower_profile(&source, ProfilingInput::Generate, false);
        let data = feedback(&trained, |plan, position| {
            if plan.name == "scope" {
                if position == 0 {
                    1
                } else if nested {
                    5_000_000
                } else {
                    0
                }
            } else {
                5_000_000
            }
        });
        let mut module = lower_profile(&source, ProfilingInput::Use(&data), false);
        let report =
            crate::optimization::run_observed(&mut module, MirOptimizationOptions::RELEASE);
        assert_eq!(remaining_calls(&module, "scope", "baseline"), 0);
        assert_eq!(remaining_calls(&module, "scope", "wrapper"), 0);
        assert!(report.events.iter().any(|event| {
            module.functions[event.site.function].name == "scope"
                && matches!(&event.decision, OptimizationDecision::Inline {
                    target: mir::CallTarget::Direct(mir::Callee::User(id)),
                    outcome: InlineOutcome::Inlined,
                    heat,
                    ..
                } if module.functions[*id].name == "baseline" && heat.profile.is_none())
        }));
        assert_mir_foundation_projection(&module);
    }
}

#[test]
fn feedback_candidates_cannot_displace_default_calls_or_refill_caller_growth() {
    let source = contention_program(64, false);
    let mut baseline = lower_profile(&source, ProfilingInput::Generate, false);
    let scope = layout(&baseline, "scope");
    let first_call = scope
        .positions
        .iter()
        .position(|position| position.kind == PositionKind::Call)
        .unwrap() as u32;
    let data = feedback(&baseline, |plan, position| {
        if plan.name == "scope" && position == first_call {
            3200
        } else if plan.name == "scope" && position != 0 {
            400
        } else {
            100
        }
    });
    crate::optimization::run_observed(&mut baseline, MirOptimizationOptions::RELEASE);
    let retained = remaining_calls(&baseline, "scope", "baseline");
    assert!(
        retained > 0 && retained < 64,
        "the baseline must spend growth"
    );

    let mut module = lower_profile(&source, ProfilingInput::Use(&data), false);
    let report = crate::optimization::run_observed(&mut module, MirOptimizationOptions::RELEASE);
    assert!(remaining_calls(&module, "scope", "baseline") <= retained);
    let estimates = report
        .events
        .iter()
        .filter_map(|event| {
            if module.functions[event.site.function].name != "scope" {
                return None;
            }
            match &event.decision {
                OptimizationDecision::Inline {
                    outcome,
                    estimate: Some(estimate),
                    heat,
                    ..
                } => Some((*outcome, estimate, heat)),
                _ => None,
            }
        })
        .collect::<Vec<_>>();
    let initial = estimates[0].1.caller_remaining_growth;
    let spent = estimates
        .iter()
        .filter(|(outcome, _, heat)| *outcome == InlineOutcome::Inlined && heat.profile.is_none())
        .map(|(_, estimate, _)| estimate.ordinary.growth)
        .sum::<usize>();
    let (_, first_feedback, _) = estimates
        .iter()
        .find(|(_, _, heat)| heat.profile.is_some())
        .unwrap();
    assert!(spent > 0);
    assert_eq!(first_feedback.caller_remaining_growth, initial - spent);
    assert_mir_foundation_projection(&module);
}

#[test]
fn default_only_expansions_do_not_claim_profile_use() {
    let source = plain_program(24, false);
    let trained = lower_profile(&source, ProfilingInput::Generate, false);
    let data = feedback(&trained, |_, _| 100);
    let mut module = lower_profile(&source, ProfilingInput::Use(&data), false);
    crate::optimization::run_observed(&mut module, MirOptimizationOptions::RELEASE);
    assert_eq!(remaining_calls(&module, "main", "leaf"), 0);
    assert!(!module.meta.profiling.use_summary().unwrap().used());
}
