use super::*;
use mir::optimization_report::{InlineHeatSource, InlineOutcome, OptimizationDecision};
use scoop_profile::{
    Counter, Feedback, FeedbackConsumer, IndexedRecord, Location, ProfileMap, Record,
};

mod additive;

fn feedback(
    module: &mir::Module,
    count: impl Fn(&scoop_profile::BodyLayout, u32) -> u64,
) -> Feedback {
    let plans = &module.meta.profiling.plans;
    let counters = plans
        .iter()
        .flat_map(|plan| {
            (0..plan.positions.len()).map(|position| Counter {
                location: Location {
                    body: plan.key,
                    position: position as u32,
                },
                inlined_at: Vec::new(),
            })
        })
        .collect();
    let record = Record::new("test".into(), counters);
    let counts = record
        .counters
        .iter()
        .map(|counter| {
            let plan = plans
                .iter()
                .find(|plan| plan.key == counter.location.body)
                .unwrap();
            count(plan, counter.location.position)
        })
        .collect();
    Feedback::from_records(
        "test",
        &[ProfileMap {
            schema: scoop_profile::SCHEMA,
            target: "test".into(),
            bodies: plans.clone(),
            records: vec![record.clone()],
        }],
        &[IndexedRecord {
            name: record.name.into_bytes(),
            hash: record.hash,
            counts,
        }],
    )
    .unwrap()
}

fn cost_program(addend: i32) -> hir::ExportHirOutput {
    let mut h = Harness::new();
    let (int, boolean, unit) = (h.int, h.boolean, h.unit);
    let mut locals = Arena::new();
    let input = locals.alloc(local("input", int));
    let value = locals.alloc(local("value", int));
    let mut statements = vec![val_decl(value, local_ref(input, int))];
    for _ in 0..55 {
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
    let large = h.user_fn_full(
        "large",
        vec![],
        vec![param("input", int, input)],
        int,
        hir::Body { locals, statements },
    );
    let mut locals = Arena::new();
    let input = locals.alloc(local("input", int));
    let active = locals.alloc(local("active", boolean));
    let addend = int_lit(&h, addend);
    let argument = integer_binary(
        &mut h,
        hir::IntegerKind::SIGNED_32,
        hir::IntegerBinaryOperator::Add,
        local_ref(input, int),
        addend,
    );
    let call = expr_stmt(call_typed(large, vec![argument], int));
    h.user_fn_full(
        "scope",
        vec![],
        vec![param("input", int, input), param("active", boolean, active)],
        unit,
        hir::Body {
            locals,
            statements: vec![
                stmt(hir::StatementKind::If {
                    cond: local_ref(active, boolean),
                    then_body: vec![call.clone()],
                    else_body: None,
                }),
                stmt(hir::StatementKind::While {
                    loop_kind: hir::LoopKind::While,
                    target: hir::LoopId::from_raw(0),
                    condition_setup: vec![],
                    cond: local_ref(active, boolean),
                    body: vec![call],
                }),
            ],
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

#[test]
fn feedback_changes_cost_and_orders_the_hot_call_before_the_cold_call() {
    let source = cost_program(0);
    let trained = lower_profile(&source, ProfilingInput::Generate, false);
    let scope = layout(&trained, "scope");
    let calls = scope
        .positions
        .iter()
        .enumerate()
        .filter_map(|(i, p)| (p.kind == PositionKind::Call).then_some(i as u32))
        .collect::<Vec<_>>();
    assert_eq!(calls.len(), 2);
    let data = feedback(&trained, |plan, position| {
        if plan.key == scope.key && position == calls[0] {
            0
        } else if plan.key == scope.key && position == calls[1] {
            3200
        } else {
            100
        }
    });
    let mut module = lower_profile(&source, ProfilingInput::Use(&data), false);
    let report = crate::optimization::run_observed(&mut module, MirOptimizationOptions::RELEASE);
    let events = report
        .events
        .iter()
        .filter_map(|event| {
            if module.functions[event.site.function].name != "scope" {
                return None;
            }
            match &event.decision {
                OptimizationDecision::Inline {
                    target: mir::CallTarget::Direct(mir::Callee::User(id)),
                    outcome,
                    heat,
                    ..
                } if module.functions[*id].name == "large" && heat.profile.is_some() => {
                    Some((*outcome, *heat))
                }
                _ => None,
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(events[0].0, InlineOutcome::Inlined);
    assert_eq!(events[0].1.profile.unwrap().frequency.unwrap().calls, 3200);
    assert!(
        events
            .iter()
            .any(|(outcome, heat)| *outcome == InlineOutcome::Profitability
                && heat.profile.unwrap().frequency.unwrap().calls == 0)
    );
    assert!(
        events
            .iter()
            .all(|(_, heat)| matches!(heat.source, InlineHeatSource::Profile))
    );
    let summary = module.meta.profiling.use_summary().unwrap();
    assert_eq!(summary.consumers[&FeedbackConsumer::Inline], 2);
    assert!(summary.used());
    assert_eq!(
        module.meta.profiling.instrumentation,
        scoop_profile::Instrumentation::Off
    );
    assert_mir_foundation_projection(&module);
}

#[test]
fn feedback_zero_entry_and_changed_body_fall_back_without_claiming_use() {
    let source = cost_program(0);
    let trained = lower_profile(&source, ProfilingInput::Generate, false);
    for (source, data, reason) in [
        (
            cost_program(0),
            feedback(&trained, |_, _| 0),
            scoop_profile::MissingCount::ZeroEntry,
        ),
        (
            cost_program(1),
            feedback(&trained, |_, _| 1),
            scoop_profile::MissingCount::BodyChanged,
        ),
    ] {
        let mut module = lower_profile(&source, ProfilingInput::Use(&data), false);
        let report =
            crate::optimization::run_observed(&mut module, MirOptimizationOptions::RELEASE);
        let event = report
            .events
            .iter()
            .find(|event| {
                module.functions[event.site.function].name == "scope"
                    && matches!(&event.decision, OptimizationDecision::Inline { heat, .. }
                        if heat.profile.is_some())
            })
            .unwrap();
        let OptimizationDecision::Inline { heat, .. } = &event.decision else {
            panic!("call decision")
        };
        assert_eq!(heat.profile.unwrap().frequency, Err(reason));
        assert!(!matches!(heat.source, InlineHeatSource::Profile));
        assert!(!module.meta.profiling.use_summary().unwrap().used());
    }
}

#[test]
fn feedback_block_frequency_controls_single_string_fusion() {
    let mut h = Harness::new();
    let string = h.string;
    let mut locals = Arena::new();
    let value = locals.alloc(local("value", string));
    h.user_fn_full(
        "concat",
        vec![],
        vec![param("value", string, value)],
        string,
        hir::Body {
            locals,
            statements: vec![stmt(hir::StatementKind::Return {
                value: primitive_binary(
                    hir::PrimitiveBinaryKind::StringConcat,
                    local_ref(value, string),
                    str_lit(&h, "!"),
                    string,
                ),
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
    let source = h.finish(main);
    let trained = lower_profile(&source, ProfilingInput::Generate, false);
    for (block_count, expected) in [(0, false), (800, true)] {
        let data = feedback(&trained, |plan, position| {
            if plan.name == "concat"
                && plan.positions[position as usize].kind == PositionKind::Block
            {
                block_count
            } else {
                100
            }
        });
        let mut module = lower_profile(&source, ProfilingInput::Use(&data), false);
        let report =
            crate::optimization::run_observed(&mut module, MirOptimizationOptions::RELEASE);
        let decision = report
            .events
            .iter()
            .find_map(|event| match &event.decision {
                OptimizationDecision::StringFusion { heat, fused, .. }
                    if module.functions[event.site.function].name == "concat" =>
                {
                    Some((*heat, *fused))
                }
                _ => None,
            })
            .unwrap();
        assert_eq!(decision.1, expected);
        assert_eq!(
            decision.0.profile.unwrap().frequency.unwrap().calls,
            block_count
        );
        assert_eq!(
            module.meta.profiling.use_summary().unwrap().consumers[&FeedbackConsumer::StringFusion],
            1
        );
        assert_mir_foundation_projection(&module);
    }
}
