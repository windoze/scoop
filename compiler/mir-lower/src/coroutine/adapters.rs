use super::*;

mod exits;
mod failure;
mod resume;

use exits::drive_exit_blocks;
use failure::generate_failure_method;
use resume::generate_resume_method;

pub(super) struct GeneratedAdapter {
    pub(super) class: mir::ClassId,
    pub(super) point: mir::CoroutineResumePointId,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn generate_adapter(
    lowerer: &mut Lowerer,
    module: &hir::Module,
    frame_class: mir::ClassId,
    frame: mir::CoroutineFrameId,
    destination: Option<FrameSlot>,
    failure_slot: FrameSlot,
    outer_step: &mir::Type,
    outer_continuation: mir::InterfaceId,
    outer_resume: mir::FunctionId,
    outer_failure: mir::FunctionId,
    source_symbol: &str,
    driver: mir::FunctionId,
    state: mir::CoroutineSuspendStateId,
    failure_state: mir::CoroutineFrameState,
    result: &mir::Type,
    safe_latches: Option<(FrameSlot, FrameSlot)>,
) -> GeneratedAdapter {
    let protocol = lowerer.coroutine_protocol(module, result);
    let continuation = lowerer.interfaces.mir_id(protocol.continuation);
    let name = format!("CoroutineAdapter${}${state}", sanitize(source_symbol));
    let mut fields = vec![
        mir::Field {
            name: "frame".to_string(),
            ty: mir::Type::Class(frame_class),
        },
        mir::Field {
            name: "status".to_string(),
            ty: mir::Type::MachineScalar(mir::MachineScalarKind::CoroutineAdapterState),
        },
    ];
    if let Some((success, failure)) = safe_latches.as_ref() {
        fields.push(mir::Field {
            name: "result".to_string(),
            ty: success.slot_ty.clone(),
        });
        fields.push(mir::Field {
            name: "failure".to_string(),
            ty: failure.slot_ty.clone(),
        });
    }
    let class = generated_class(lowerer, name, fields, vec![continuation], Vec::new());
    let resume = generate_resume_method(
        lowerer,
        module,
        class,
        frame_class,
        destination,
        outer_step,
        outer_continuation,
        outer_resume,
        outer_failure,
        driver,
        source_symbol,
        state,
        result,
        safe_latches.as_ref().map(|(success, _)| success.clone()),
    );
    let failure = generate_failure_method(
        lowerer,
        module,
        class,
        frame_class,
        failure_slot,
        outer_step,
        outer_continuation,
        outer_resume,
        outer_failure,
        driver,
        source_symbol,
        state,
        failure_state,
        safe_latches.as_ref().map(|(_, failure)| failure.clone()),
    );
    lowerer.classes[class].itables = vec![mir::ItableRecord {
        interface: continuation,
        slots: vec![
            mir::TableSlot::Function(resume),
            mir::TableSlot::Function(failure),
        ],
    }];
    let point = lowerer
        .coroutines
        .resume_points
        .alloc(mir::CoroutineResumePoint {
            frame,
            state,
            result: result.clone(),
            adapter: class,
            resume,
            resume_with_exception: failure,
        });
    GeneratedAdapter { class, point }
}
