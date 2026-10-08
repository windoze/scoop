use super::*;

mod exits;
mod failure;
mod resume;

use exits::drive_exit_blocks;
use failure::generate_failure_method;
use resume::generate_resume_method;

pub(super) struct GeneratedAdapter {
    pub(super) class: mir::ClassId,
    continuation: mir::InterfaceId,
    pub(super) resume: mir::FunctionId,
    pub(super) resume_with_exception: mir::FunctionId,
    pub(super) identity: mir::ContinuationAdapterIdentity,
}

impl GeneratedAdapter {
    pub(super) fn reference(&self, local: mir::LocalId) -> mir::Expr {
        let ty = mir::Type::Interface(self.continuation);
        mir::Expr::new(
            ty.clone(),
            mir::ExprKind::Retype {
                operand: Box::new(mir::Expr::local(local, mir::Type::Class(self.class))),
                ty: Box::new(ty),
            },
        )
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn generate_adapter(
    lowerer: &mut Lowerer,
    module: &hir::Module,
    frame_class: mir::ClassId,
    frame_layout: FrameLayout,
    destination: Option<FrameSlot>,
    failure_slot: FrameSlot,
    outer_step: &mir::Type,
    outer_continuation: mir::InterfaceId,
    outer_resume: &mir::CallTarget,
    outer_failure: &mir::CallTarget,
    source_name: &str,
    driver: mir::FunctionId,
    source: hir::CallableMaterialization,
    source_odr_group: Option<hir::OdrGroupId>,
    suspension_site: hir::StructuralDefinitionPath,
    state: mir::CoroutineSuspendStateId,
    result: &mir::Type,
    safe_latches: Option<(FrameSlot, FrameSlot)>,
) -> GeneratedAdapter {
    let protocol = lowerer.coroutine_protocol(module, result);
    let continuation = lowerer.interfaces.mir_id(protocol.continuation);
    let name = format!("CoroutineAdapter<{source_name}>${state}");
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
    let success_signature =
        crate::source_callables::exact_function_signature(module, protocol.continuation_resume);
    let failure_signature = crate::source_callables::exact_function_signature(
        module,
        protocol.continuation_resume_with_exception,
    );
    let identity = if safe_latches.is_some() {
        mir::ContinuationAdapterIdentity::latched(
            source,
            suspension_site,
            success_signature,
            failure_signature,
            source_odr_group,
        )
    } else {
        mir::ContinuationAdapterIdentity::direct(
            source,
            suspension_site,
            success_signature,
            failure_signature,
            source_odr_group,
        )
    }
    .expect("a suspension site has one persistent continuation-adapter identity");
    let class = generated_class(lowerer, name, fields, vec![continuation], Vec::new());
    let resume = generate_resume_method(
        lowerer,
        module,
        class,
        frame_class,
        frame_layout,
        destination,
        outer_step,
        outer_continuation,
        outer_resume,
        outer_failure,
        driver,
        state,
        result,
        safe_latches.as_ref().map(|(success, _)| success.clone()),
    );
    let failure = generate_failure_method(
        lowerer,
        module,
        class,
        frame_class,
        frame_layout,
        failure_slot,
        outer_step,
        outer_continuation,
        outer_resume,
        outer_failure,
        driver,
        state,
        safe_latches.as_ref().map(|(_, failure)| failure.clone()),
    );
    let mut slots = [
        (protocol.continuation_resume, resume),
        (protocol.continuation_resume_with_exception, failure),
    ]
    .map(|(source, function)| {
        let method = module.functions[source]
            .receiver
            .method()
            .expect("a continuation member has a receiver");
        let hir::MethodDispatch::Interface { slot, .. } = method.dispatch else {
            unreachable!("continuation methods retain interface slots")
        };
        (slot, mir::TableSlot::Function(function))
    });
    slots.sort_by_key(|(slot, _)| *slot);
    lowerer.classes[class].itables = vec![mir::ItableRecord {
        interface: continuation,
        slots: slots.into_iter().map(|(_, target)| target).collect(),
    }];
    GeneratedAdapter {
        class,
        continuation,
        resume,
        resume_with_exception: failure,
        identity,
    }
}
