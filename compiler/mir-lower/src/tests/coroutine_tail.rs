use super::*;

pub(super) mod sample;
use sample::{Shape, lower_tail, program};

fn coroutine<'a>(module: &'a mir::Module, name: &str) -> &'a mir::CoroutineFunction {
    module
        .meta
        .coroutine_functions
        .iter()
        .find_map(|(_, metadata)| {
            (module.functions[metadata.function].name == name).then_some(metadata)
        })
        .unwrap()
}

#[test]
fn exact_tail_chain_forwards_original_completion_and_step_without_frames() {
    let module = lower_tail(&program(Shape::Exact, 4, false), true);
    assert!(matches!(
        coroutine(&module, "leaf").lowering,
        mir::CoroutineLowering::Immediate
    ));
    for index in 0..4 {
        let metadata = coroutine(&module, &format!("wrapper{index}"));
        assert!(matches!(
            metadata.lowering,
            mir::CoroutineLowering::TailForwarding
        ));
        let function = &module.functions[metadata.function];
        let completion = function.params.last().unwrap().local;
        let mut calls = 0;
        for (_, block) in function.body.blocks.iter() {
            for statement in &block.statements {
                if let mir::StatementKind::Call(mir::CallEffect::Value { destination, call }) =
                    &statement.kind
                {
                    calls += 1;
                    assert!(
                        matches!(call.args.last().unwrap().kind, mir::ExprKind::Local(local) if local == completion)
                    );
                    assert_eq!(function.body.locals[*destination].ty, function.return_ty);
                    assert!(
                        matches!(block.terminator, mir::Terminator::Return { ref value }
                        if matches!(value.kind, mir::ExprKind::Local(local) if local == *destination))
                    );
                    assert!(module.meta.local_values.iter().any(|value| value.owner()
                        == mir::LocalValueOwner::Function(metadata.function)
                        && value.local() == *destination));
                }
            }
        }
        assert_eq!(calls, 1);
    }
    assert!(module.meta.coroutine_frames.is_empty());
    assert!(module.meta.coroutine_resume_points.is_empty());
    assert_mir_foundation_projection(&module);
}

#[test]
fn unit_tail_call_returns_callee_step_instead_of_nested_completed() {
    let module = lower_tail(&program(Shape::Exact, 1, true), true);
    let metadata = coroutine(&module, "wrapper0");
    assert_eq!(metadata.source_return, mir::Type::Unit);
    assert!(matches!(
        metadata.lowering,
        mir::CoroutineLowering::TailForwarding
    ));
    let function = &module.functions[metadata.function];
    assert!(function.body.blocks.iter().any(|(_, block)| {
        matches!(block.terminator, mir::Terminator::Return { ref value }
            if value.ty == function.return_ty && matches!(value.kind, mir::ExprKind::Local(_)))
    }));
}

#[test]
fn result_processing_or_an_earlier_suspension_preserves_state_machine() {
    for shape in [Shape::Add, Shape::EarlierCall] {
        let module = lower_tail(&program(shape, 1, false), true);
        assert!(matches!(
            coroutine(&module, "wrapper0").lowering,
            mir::CoroutineLowering::StateMachine { .. }
        ));
        assert!(!module.meta.coroutine_frames.is_empty());
        assert!(!module.meta.coroutine_resume_points.is_empty());
    }
}

#[test]
fn unoptimized_tail_call_retains_existing_lowering() {
    let module = lower_tail(&program(Shape::Exact, 1, false), false);
    assert!(matches!(
        coroutine(&module, "wrapper0").lowering,
        mir::CoroutineLowering::StateMachine { .. }
    ));
}

#[test]
fn private_coroutine_helpers_do_not_change_dependency_signatures() {
    let source = program(Shape::Exact, 2, false);
    let debug = lower_tail(&source, false);
    let release = lower_tail(&source, true);
    let debug = mir::CanonicalMirFoundation::from_module(&debug).unwrap();
    let release = mir::CanonicalMirFoundation::from_module(&release).unwrap();
    assert!(debug.callable_signatures().len() > release.callable_signatures().len());
    assert_eq!(
        mir::StrongCallableBridgeSurfaceV1::from_foundation(&debug),
        mir::StrongCallableBridgeSurfaceV1::from_foundation(&release)
    );
}

#[test]
fn forwarding_callee_is_still_a_suspension_in_its_nontail_caller() {
    let module = lower_tail(&program(Shape::TailThenAdd, 2, false), true);
    assert!(matches!(
        coroutine(&module, "wrapper0").lowering,
        mir::CoroutineLowering::TailForwarding
    ));
    assert!(matches!(
        coroutine(&module, "wrapper1").lowering,
        mir::CoroutineLowering::StateMachine { .. }
    ));
}
