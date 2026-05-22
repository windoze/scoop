use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use crate::effect_facts::StepSchemaId;

use super::ir::{
    BoundaryId, ContinuationObjectId, FrameSlotId, LateLoweredBoundary,
    LateLoweredBoundaryLowering, LateLoweredCallBoundaryContinuationComposition,
    LateLoweredCallable, LateLoweredContinuationCapture, LateLoweredContinuationObject,
    LateLoweredFrameSlot, LateLoweredFrameSlotKind, LateLoweredHandleBoundaryCaseRoutingAction,
    LateLoweredHandleDispatchContract, LateLoweredProgram, LateLoweredState,
    LateLoweredStateTerminator, LateLoweredStepCaseEmission, LateLoweredStepDispatchPlan,
    ResumeInterfaceId, StateId,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LirOptVerifyError {
    detail: String,
}

impl LirOptVerifyError {
    fn new(detail: impl Into<String>) -> Self {
        Self {
            detail: detail.into(),
        }
    }
}

impl fmt::Display for LirOptVerifyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.detail)
    }
}

impl Error for LirOptVerifyError {}

pub(crate) fn verify_post_opt_program(
    program: &LateLoweredProgram,
) -> Result<(), LirOptVerifyError> {
    let step_schemas = program
        .step_types()
        .iter()
        .map(|step_type| step_type.step_schema())
        .collect::<BTreeSet<_>>();
    let resume_packings = program
        .resume_packings()
        .iter()
        .map(|packing| packing.interface_id())
        .collect::<BTreeSet<_>>();
    let continuation_objects = program
        .continuation_objects()
        .iter()
        .map(|object| object.object_id())
        .collect::<BTreeSet<_>>();

    for callable in program.callables() {
        if let Some(step_schema) = callable.body_step_schema()
            && !step_schemas.contains(&step_schema)
        {
            return Err(verify_error(
                callable,
                format!("references missing StepSchema s{}", step_schema.as_u32()),
            ));
        }
        if !callable.has_control_body() {
            continue;
        }
        let object_id = callable.continuation_object();
        if !continuation_objects.contains(&object_id) {
            return Err(verify_error(
                callable,
                format!(
                    "references missing continuation object cont_obj#{}",
                    object_id.as_u32()
                ),
            ));
        }
        for packing_id in callable.resume_packings() {
            if !resume_packings.contains(packing_id) {
                return Err(verify_error(
                    callable,
                    format!(
                        "references missing resume packing packing#{}",
                        packing_id.as_u32()
                    ),
                ));
            }
        }
        verify_control_body(callable, &step_schemas, &continuation_objects)?;
        let object = program
            .continuation_object(object_id)
            .expect("object id set was built from the same program");
        verify_continuation_object(callable, object, &resume_packings, &step_schemas)?;
    }

    for interface in program.resume_packings() {
        if !step_schemas.contains(&interface.return_step_schema()) {
            return Err(LirOptVerifyError::new(format!(
                "resume packing packing#{} references missing return StepSchema s{}",
                interface.interface_id().as_u32(),
                interface.return_step_schema().as_u32(),
            )));
        }
        for method in interface.methods() {
            if !step_schemas.contains(&method.out_step_schema()) {
                return Err(LirOptVerifyError::new(format!(
                    "resume packing packing#{} method c{} references missing out StepSchema s{}",
                    interface.interface_id().as_u32(),
                    method.case_tag().as_u32(),
                    method.out_step_schema().as_u32(),
                )));
            }
        }
    }

    Ok(())
}

fn verify_control_body(
    callable: &LateLoweredCallable,
    step_schemas: &BTreeSet<StepSchemaId>,
    continuation_objects: &BTreeSet<ContinuationObjectId>,
) -> Result<(), LirOptVerifyError> {
    let state_ids = callable
        .state_graph()
        .states()
        .iter()
        .map(LateLoweredState::state_id)
        .collect::<BTreeSet<_>>();
    let boundary_ids = callable
        .boundary_map()
        .entries()
        .iter()
        .map(LateLoweredBoundary::boundary_id)
        .collect::<BTreeSet<_>>();
    let frame_slots = callable
        .frame_schema()
        .slots()
        .iter()
        .map(LateLoweredFrameSlot::slot_id)
        .collect::<BTreeSet<_>>();

    ensure_state(
        callable,
        &state_ids,
        callable.state_graph().entry_state(),
        "entry_state",
    )?;
    ensure_state(
        callable,
        &state_ids,
        callable.state_graph().complete_state(),
        "complete_state",
    )?;
    if let Some(cleanup_state) = callable.state_graph().cleanup_state() {
        ensure_state(callable, &state_ids, cleanup_state, "cleanup_state")?;
    }
    if let Some(drop_state) = callable.state_graph().drop_state() {
        ensure_state(callable, &state_ids, drop_state, "drop_state")?;
    }

    for state in callable.state_graph().states() {
        for successor in state.successors() {
            ensure_state(callable, &state_ids, *successor, "state successor")?;
        }
        verify_terminator_refs(
            callable,
            state.terminator(),
            &state_ids,
            &boundary_ids,
            &frame_slots,
            step_schemas,
            continuation_objects,
        )?;
    }

    if let Some(effect_step) = callable.effect_step_abi() {
        ensure_state(
            callable,
            &state_ids,
            effect_step.dynamic_invoke_entry().entry_state(),
            "dynamic invoke entry_state",
        )?;
        ensure_state(
            callable,
            &state_ids,
            effect_step.dynamic_invoke_entry().complete_state(),
            "dynamic invoke complete_state",
        )?;
    }

    for boundary in callable.boundary_map().entries() {
        ensure_state(
            callable,
            &state_ids,
            boundary.owner_state(),
            "boundary owner_state",
        )?;
        ensure_state(
            callable,
            &state_ids,
            boundary.resume_state(),
            "boundary resume_state",
        )?;
        if let Some(lowering) = boundary.lowering() {
            verify_boundary_lowering(
                callable,
                lowering,
                &state_ids,
                &boundary_ids,
                &frame_slots,
                step_schemas,
                continuation_objects,
            )?;
        }
    }
    for resume in callable.resume_state_map().entries() {
        ensure_boundary(
            callable,
            &boundary_ids,
            resume.boundary_id(),
            "resume_state_map",
        )?;
        ensure_state(callable, &state_ids, resume.state_id(), "resume_state_map")?;
    }
    for slot in callable.frame_schema().slots() {
        for state_id in slot.write_points() {
            ensure_state(callable, &state_ids, *state_id, "frame slot write point")?;
        }
        for state_id in slot.read_points() {
            ensure_state(callable, &state_ids, *state_id, "frame slot read point")?;
        }
        verify_frame_slot_kind(callable, slot.kind(), &boundary_ids)?;
    }
    for binding in callable.frame_schema().resume_payload_bindings() {
        ensure_boundary(
            callable,
            &boundary_ids,
            binding.boundary_id(),
            "resume payload binding",
        )?;
        ensure_state(
            callable,
            &state_ids,
            binding.resume_state(),
            "resume payload binding",
        )?;
        if let Some(slot_id) = binding.consumer_frame_slot() {
            ensure_frame_slot(callable, &frame_slots, slot_id, "resume payload binding")?;
        }
    }
    for binding in callable.frame_schema().completion_payload_bindings() {
        ensure_state(
            callable,
            &state_ids,
            binding.return_state(),
            "completion payload binding",
        )?;
        ensure_state(
            callable,
            &state_ids,
            binding.complete_state(),
            "completion payload binding",
        )?;
        if let Some(slot_id) = binding.payload_frame_slot() {
            ensure_frame_slot(
                callable,
                &frame_slots,
                slot_id,
                "completion payload binding",
            )?;
        }
    }

    Ok(())
}

fn verify_boundary_lowering(
    callable: &LateLoweredCallable,
    lowering: &LateLoweredBoundaryLowering,
    state_ids: &BTreeSet<StateId>,
    boundary_ids: &BTreeSet<BoundaryId>,
    frame_slots: &BTreeSet<FrameSlotId>,
    step_schemas: &BTreeSet<StepSchemaId>,
    continuation_objects: &BTreeSet<ContinuationObjectId>,
) -> Result<(), LirOptVerifyError> {
    match lowering {
        LateLoweredBoundaryLowering::Call(lowering) => {
            verify_step_dispatch(
                callable,
                lowering.dispatch(),
                state_ids,
                step_schemas,
                continuation_objects,
                "call boundary dispatch",
            )?;
            for composition in lowering.continuation_compositions() {
                verify_continuation_composition(
                    callable,
                    composition,
                    state_ids,
                    boundary_ids,
                    frame_slots,
                    step_schemas,
                )?;
            }
            if let Some(runtime_error) = lowering.consumed_runtime_error_case() {
                ensure_state(
                    callable,
                    state_ids,
                    runtime_error.target_state(),
                    "call consumed runtime-error target",
                )?;
            }
        }
        LateLoweredBoundaryLowering::ClassCtor(lowering) => {
            for emission in lowering.emitted_steps() {
                verify_step_case_emission(
                    callable,
                    emission,
                    step_schemas,
                    continuation_objects,
                    "class-ctor emitted step",
                )?;
            }
        }
        LateLoweredBoundaryLowering::Perform(lowering) => verify_step_case_emission(
            callable,
            lowering.emitted_step(),
            step_schemas,
            continuation_objects,
            "perform emitted step",
        )?,
        LateLoweredBoundaryLowering::Resume(lowering) => {
            ensure_boundary(
                callable,
                boundary_ids,
                lowering.runtime_error_boundary(),
                "resume runtime-error boundary",
            )?;
            verify_step_dispatch(
                callable,
                lowering.dispatch(),
                state_ids,
                step_schemas,
                continuation_objects,
                "resume boundary dispatch",
            )?;
            for composition in lowering.continuation_compositions() {
                verify_continuation_composition(
                    callable,
                    composition,
                    state_ids,
                    boundary_ids,
                    frame_slots,
                    step_schemas,
                )?;
            }
        }
        LateLoweredBoundaryLowering::RuntimeError(lowering) => {
            ensure_boundary(
                callable,
                boundary_ids,
                lowering.resume_boundary(),
                "runtime-error resume boundary",
            )?;
            verify_step_case_emission(
                callable,
                lowering.emitted_step(),
                step_schemas,
                continuation_objects,
                "runtime-error emitted step",
            )?;
        }
        LateLoweredBoundaryLowering::Handle(lowering) => {
            for emission in lowering.outward_emissions() {
                verify_step_case_emission(
                    callable,
                    emission,
                    step_schemas,
                    continuation_objects,
                    "handle outward emission",
                )?;
            }
        }
    }
    Ok(())
}

fn verify_step_dispatch(
    callable: &LateLoweredCallable,
    dispatch: &LateLoweredStepDispatchPlan,
    state_ids: &BTreeSet<StateId>,
    step_schemas: &BTreeSet<StepSchemaId>,
    continuation_objects: &BTreeSet<ContinuationObjectId>,
    reference: &str,
) -> Result<(), LirOptVerifyError> {
    ensure_step_schema(
        callable,
        step_schemas,
        dispatch.input_step_schema(),
        reference,
    )?;
    ensure_state(
        callable,
        state_ids,
        dispatch.complete().target_state(),
        reference,
    )?;
    for forwarding in dispatch.outward_cases() {
        verify_step_case_emission(
            callable,
            forwarding.emission(),
            step_schemas,
            continuation_objects,
            reference,
        )?;
    }
    Ok(())
}

fn verify_continuation_composition(
    callable: &LateLoweredCallable,
    composition: &LateLoweredCallBoundaryContinuationComposition,
    state_ids: &BTreeSet<StateId>,
    boundary_ids: &BTreeSet<BoundaryId>,
    frame_slots: &BTreeSet<FrameSlotId>,
    step_schemas: &BTreeSet<StepSchemaId>,
) -> Result<(), LirOptVerifyError> {
    ensure_boundary(
        callable,
        boundary_ids,
        composition.boundary_id(),
        "continuation composition boundary",
    )?;
    ensure_step_schema(
        callable,
        step_schemas,
        composition.input_step_schema(),
        "continuation composition input StepSchema",
    )?;
    ensure_step_schema(
        callable,
        step_schemas,
        composition.callee_continuation_contract().out_step_schema(),
        "callee continuation contract out StepSchema",
    )?;
    ensure_step_schema(
        callable,
        step_schemas,
        composition.caller_continuation_contract().out_step_schema(),
        "caller continuation contract out StepSchema",
    )?;
    ensure_state(
        callable,
        state_ids,
        composition.caller_resume_state(),
        "continuation composition caller_resume_state",
    )?;
    if let Some(slot_id) = composition.caller_result_frame_slot() {
        ensure_frame_slot(
            callable,
            frame_slots,
            slot_id,
            "continuation composition caller_result_frame_slot",
        )?;
    }
    Ok(())
}

fn verify_step_case_emission(
    callable: &LateLoweredCallable,
    emission: &LateLoweredStepCaseEmission,
    step_schemas: &BTreeSet<StepSchemaId>,
    continuation_objects: &BTreeSet<ContinuationObjectId>,
    reference: &str,
) -> Result<(), LirOptVerifyError> {
    ensure_step_schema(
        callable,
        step_schemas,
        emission.continuation_contract().out_step_schema(),
        reference,
    )?;
    ensure_continuation_object(
        callable,
        continuation_objects,
        emission.continuation_object(),
        reference,
    )
}

fn verify_continuation_object(
    callable: &LateLoweredCallable,
    object: &LateLoweredContinuationObject,
    resume_packings: &BTreeSet<ResumeInterfaceId>,
    step_schemas: &BTreeSet<StepSchemaId>,
) -> Result<(), LirOptVerifyError> {
    let state_ids = callable
        .state_graph()
        .states()
        .iter()
        .map(LateLoweredState::state_id)
        .collect::<BTreeSet<_>>();
    let frame_slots = callable
        .frame_schema()
        .slots()
        .iter()
        .map(LateLoweredFrameSlot::slot_id)
        .collect::<BTreeSet<_>>();

    for packing_id in object.implemented_packings() {
        if !resume_packings.contains(packing_id) {
            return Err(verify_error(
                callable,
                format!(
                    "continuation object cont_obj#{} references missing packing#{}",
                    object.object_id().as_u32(),
                    packing_id.as_u32(),
                ),
            ));
        }
    }
    for capture in object.captures() {
        match *capture {
            LateLoweredContinuationCapture::FrameSlot(slot_id) => {
                ensure_frame_slot(callable, &frame_slots, slot_id, "continuation capture")?;
            }
            LateLoweredContinuationCapture::State(state_id) => {
                ensure_state(callable, &state_ids, state_id, "continuation capture")?;
            }
        }
    }
    for surface in object.surface_resumes() {
        if !step_schemas.contains(&surface.out_step_schema()) {
            return Err(verify_error(
                callable,
                format!(
                    "surface resume c{} references missing out StepSchema s{}",
                    surface.case_tag().as_u32(),
                    surface.out_step_schema().as_u32(),
                ),
            ));
        }
    }
    for method in object.methods() {
        if !resume_packings.contains(&method.packing_interface_id()) {
            return Err(verify_error(
                callable,
                format!(
                    "continuation method c{} references missing packing#{}",
                    method.case_tag().as_u32(),
                    method.packing_interface_id().as_u32(),
                ),
            ));
        }
        if !step_schemas.contains(&method.out_step_schema()) {
            return Err(verify_error(
                callable,
                format!(
                    "continuation method c{} references missing out StepSchema s{}",
                    method.case_tag().as_u32(),
                    method.out_step_schema().as_u32(),
                ),
            ));
        }
    }
    Ok(())
}

fn verify_terminator_refs(
    callable: &LateLoweredCallable,
    terminator: &LateLoweredStateTerminator,
    state_ids: &BTreeSet<StateId>,
    boundary_ids: &BTreeSet<BoundaryId>,
    frame_slots: &BTreeSet<FrameSlotId>,
    step_schemas: &BTreeSet<StepSchemaId>,
    continuation_objects: &BTreeSet<ContinuationObjectId>,
) -> Result<(), LirOptVerifyError> {
    match terminator {
        LateLoweredStateTerminator::Suspend {
            boundary_ids: refs,
            drop_state,
            ..
        } => {
            for boundary_id in refs {
                ensure_boundary(callable, boundary_ids, *boundary_id, "suspend boundary")?;
            }
            if let Some(drop_state) = drop_state {
                ensure_state(callable, state_ids, *drop_state, "suspend drop_state")?;
            }
        }
        LateLoweredStateTerminator::HandleDispatch {
            exit_state,
            contract,
            boundary_ids: refs,
            drop_state,
            ..
        } => {
            ensure_state(callable, state_ids, *exit_state, "handle exit_state")?;
            for boundary_id in refs {
                ensure_boundary(callable, boundary_ids, *boundary_id, "handle boundary")?;
            }
            if let Some(drop_state) = drop_state {
                ensure_state(callable, state_ids, *drop_state, "handle drop_state")?;
            }
            verify_handle_dispatch_contract(
                callable,
                contract,
                state_ids,
                boundary_ids,
                frame_slots,
                step_schemas,
                continuation_objects,
            )?;
        }
        _ => {}
    }
    Ok(())
}

fn verify_handle_dispatch_contract(
    callable: &LateLoweredCallable,
    contract: &LateLoweredHandleDispatchContract,
    state_ids: &BTreeSet<StateId>,
    boundary_ids: &BTreeSet<BoundaryId>,
    frame_slots: &BTreeSet<FrameSlotId>,
    step_schemas: &BTreeSet<StepSchemaId>,
    continuation_objects: &BTreeSet<ContinuationObjectId>,
) -> Result<(), LirOptVerifyError> {
    ensure_state(
        callable,
        state_ids,
        contract.body_complete_target(),
        "handle body_complete_target",
    )?;
    ensure_state(
        callable,
        state_ids,
        contract.arm_complete_target(),
        "handle arm_complete_target",
    )?;
    if let Some(finally_complete_target) = contract.finally_complete_target() {
        ensure_state(
            callable,
            state_ids,
            finally_complete_target,
            "handle finally_complete_target",
        )?;
    }
    if let Some(abandon_target) = contract.abandon_target() {
        ensure_state(callable, state_ids, abandon_target, "handle abandon_target")?;
    }

    for arm in contract.handled_arms() {
        ensure_state(callable, state_ids, arm.arm_state(), "handle arm_state")?;
        for binder in arm.payload_binders() {
            if let Some(slot_id) = binder.frame_slot() {
                ensure_frame_slot(callable, frame_slots, slot_id, "handle payload binder")?;
            }
        }
        if let Some(binder) = arm.continuation_binder() {
            if let Some(slot_id) = binder.frame_slot() {
                ensure_frame_slot(callable, frame_slots, slot_id, "handle continuation binder")?;
            }
            ensure_continuation_object(
                callable,
                continuation_objects,
                binder.continuation_object(),
                "handle continuation binder",
            )?;
        }
    }
    for emission in contract.outward_emissions() {
        verify_step_case_emission(
            callable,
            emission,
            step_schemas,
            continuation_objects,
            "handle contract outward emission",
        )?;
    }
    for origin in contract.pending_completion_origins() {
        ensure_boundary(
            callable,
            boundary_ids,
            origin.boundary_id(),
            "handle pending completion origin",
        )?;
        ensure_state(
            callable,
            state_ids,
            origin.owner_state(),
            "handle pending completion owner_state",
        )?;
        ensure_state(
            callable,
            state_ids,
            origin.resume_state(),
            "handle pending completion resume_state",
        )?;
    }
    for transport in contract.pending_payload_transports() {
        ensure_frame_slot(
            callable,
            frame_slots,
            transport.frame_slot(),
            "handle pending payload transport",
        )?;
    }
    for region in contract.state_regions() {
        ensure_state(
            callable,
            state_ids,
            region.state_id(),
            "handle state region",
        )?;
    }
    for routing in contract.boundary_routings() {
        ensure_boundary(
            callable,
            boundary_ids,
            routing.boundary_id(),
            "handle boundary routing",
        )?;
        ensure_state(
            callable,
            state_ids,
            routing.owner_state(),
            "handle boundary routing owner_state",
        )?;
        ensure_state(
            callable,
            state_ids,
            routing.resume_state(),
            "handle boundary routing resume_state",
        )?;
        for case_routing in routing.case_routings() {
            match case_routing.action() {
                LateLoweredHandleBoundaryCaseRoutingAction::ConsumeToArm {
                    arm_state,
                    continuation_resume_state,
                    ..
                } => {
                    ensure_state(callable, state_ids, arm_state, "handle routing arm_state")?;
                    ensure_state(
                        callable,
                        state_ids,
                        continuation_resume_state,
                        "handle routing continuation_resume_state",
                    )?;
                }
                LateLoweredHandleBoundaryCaseRoutingAction::PendingCompletion { .. }
                | LateLoweredHandleBoundaryCaseRoutingAction::EmitOutward => {}
            }
        }
    }
    Ok(())
}

fn verify_frame_slot_kind(
    callable: &LateLoweredCallable,
    kind: LateLoweredFrameSlotKind,
    boundary_ids: &BTreeSet<BoundaryId>,
) -> Result<(), LirOptVerifyError> {
    match kind {
        LateLoweredFrameSlotKind::ResumePayload { boundary, .. }
        | LateLoweredFrameSlotKind::BoundaryResult { boundary, .. } => {
            ensure_boundary(callable, boundary_ids, boundary, "frame slot kind")?;
        }
        _ => {}
    }
    Ok(())
}

fn ensure_step_schema(
    callable: &LateLoweredCallable,
    step_schemas: &BTreeSet<StepSchemaId>,
    step_schema: StepSchemaId,
    reference: &str,
) -> Result<(), LirOptVerifyError> {
    if step_schemas.contains(&step_schema) {
        return Ok(());
    }
    Err(verify_error(
        callable,
        format!(
            "{reference} references missing StepSchema s{}",
            step_schema.as_u32()
        ),
    ))
}

fn ensure_continuation_object(
    callable: &LateLoweredCallable,
    continuation_objects: &BTreeSet<ContinuationObjectId>,
    object_id: ContinuationObjectId,
    reference: &str,
) -> Result<(), LirOptVerifyError> {
    if continuation_objects.contains(&object_id) {
        return Ok(());
    }
    Err(verify_error(
        callable,
        format!(
            "{reference} references missing continuation object cont_obj#{}",
            object_id.as_u32()
        ),
    ))
}

fn ensure_state(
    callable: &LateLoweredCallable,
    states: &BTreeSet<StateId>,
    state_id: StateId,
    reference: &str,
) -> Result<(), LirOptVerifyError> {
    if states.contains(&state_id) {
        return Ok(());
    }
    Err(verify_error(
        callable,
        format!(
            "{reference} references missing state st{}",
            state_id.as_u32()
        ),
    ))
}

fn ensure_boundary(
    callable: &LateLoweredCallable,
    boundaries: &BTreeSet<BoundaryId>,
    boundary_id: BoundaryId,
    reference: &str,
) -> Result<(), LirOptVerifyError> {
    if boundaries.contains(&boundary_id) {
        return Ok(());
    }
    Err(verify_error(
        callable,
        format!(
            "{reference} references missing boundary bd{}",
            boundary_id.as_u32(),
        ),
    ))
}

fn ensure_frame_slot(
    callable: &LateLoweredCallable,
    frame_slots: &BTreeSet<FrameSlotId>,
    slot_id: FrameSlotId,
    reference: &str,
) -> Result<(), LirOptVerifyError> {
    if frame_slots.contains(&slot_id) {
        return Ok(());
    }
    Err(verify_error(
        callable,
        format!(
            "{reference} references missing frame slot slot#{}",
            slot_id.as_u32()
        ),
    ))
}

fn verify_error(callable: &LateLoweredCallable, detail: impl Into<String>) -> LirOptVerifyError {
    LirOptVerifyError::new(format!(
        "LIR opt verifier rejected `{}`: {}",
        callable.root_fqn(),
        detail.into(),
    ))
}
