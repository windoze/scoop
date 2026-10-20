use super::coroutine_tail::sample::{Shape, lower_tail, small_program};
use super::*;

#[test]
fn synchronous_nontail_chain_has_no_frames_before_the_execution_abi() {
    let module = lower_tail(&small_program(Shape::Add, 4, false), true);
    assert!(
        module
            .meta
            .coroutine_functions
            .iter()
            .all(|(_, coroutine)| {
                matches!(coroutine.lowering, mir::CoroutineLowering::Immediate)
            })
    );
    assert!(module.meta.coroutine_frames.is_empty());
    assert!(module.meta.coroutine_resume_points.is_empty());
    for (_, coroutine) in module.meta.coroutine_functions.iter() {
        let function = &module.functions[coroutine.function];
        if function.name != "leaf" && !function.name.starts_with("wrapper") {
            continue;
        }
        assert_eq!(
            function.params.len(),
            1,
            "only its original completion is added"
        );
        assert!(function.body.blocks.iter().all(|(_, block)| {
            block
                .statements
                .iter()
                .all(|statement| !matches!(statement.kind, mir::StatementKind::Call(_)))
        }));
    }
    assert_mir_foundation_projection(&module);
}

#[test]
fn unit_and_multiple_value_calls_remove_their_actual_sites() {
    for (shape, unit) in [(Shape::Exact, true), (Shape::EarlierCall, false)] {
        let module = lower_tail(&small_program(shape, 2, unit), true);
        assert!(module.meta.coroutine_frames.is_empty());
        assert!(
            module
                .meta
                .coroutine_functions
                .iter()
                .all(|(_, coroutine)| {
                    matches!(coroutine.lowering, mir::CoroutineLowering::Immediate)
                })
        );
        assert_mir_foundation_projection(&module);
    }
}

#[test]
fn depth_limit_keeps_remaining_calls_as_real_suspend_sites() {
    let module = lower_tail(&small_program(Shape::Add, 10, false), true);
    let outer = module
        .meta
        .coroutine_functions
        .iter()
        .find_map(|(_, coroutine)| {
            (module.functions[coroutine.function].name == "wrapper9").then_some(coroutine)
        })
        .unwrap();
    assert!(matches!(
        outer.lowering,
        mir::CoroutineLowering::StateMachine { .. }
    ));
    assert!(!module.meta.coroutine_resume_points.is_empty());
    assert_mir_foundation_projection(&module);
}

#[test]
fn debug_keeps_the_original_synchronous_call_state_machine() {
    let module = lower_tail(&small_program(Shape::Add, 2, false), false);
    assert!(!module.meta.coroutine_frames.is_empty());
    assert!(!module.meta.coroutine_resume_points.is_empty());
}
