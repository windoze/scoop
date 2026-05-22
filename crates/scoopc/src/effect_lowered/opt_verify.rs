use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use crate::effect_facts::StepSchemaId;

use super::ir::{
    BoundaryId, FrameSlotId, LateLoweredBoundary, LateLoweredCallable,
    LateLoweredContinuationCapture, LateLoweredContinuationObject, LateLoweredFrameSlot,
    LateLoweredFrameSlotKind, LateLoweredProgram, LateLoweredState, LateLoweredStateTerminator,
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
        verify_control_body(callable)?;
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

fn verify_control_body(callable: &LateLoweredCallable) -> Result<(), LirOptVerifyError> {
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
        verify_terminator_refs(callable, state.terminator(), &state_ids, &boundary_ids)?;
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
        }
        _ => {}
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
