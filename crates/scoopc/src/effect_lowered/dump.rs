use std::fmt::Write;

use crate::effect_facts::{CaseTag, ConcreteOpKey, ImplPlan};
use crate::ty::{EffectRow, TypeId};

use super::ir::{
    BoundarySiteKind, LateLoweredBodyVersionKey, LateLoweredBoundary, LateLoweredBoundarySource,
    LateLoweredCallable, LateLoweredContinuationCapture, LateLoweredContinuationMethod,
    LateLoweredContinuationMethodReachability, LateLoweredContinuationObject,
    LateLoweredFrameSchema, LateLoweredFrameSlot, LateLoweredFrameSlotKind, LateLoweredProgram,
    LateLoweredResumeInterface, LateLoweredResumeMethod, LateLoweredResumeStateMap,
    LateLoweredState, LateLoweredStateGraph, LateLoweredStateRole, LateLoweredStateSlice,
    LateLoweredStepCase, LateLoweredStepType, ResumeInterfaceId, StateId, SystemSlotKind,
};

/// 渲染 late-lowered program 的稳定文本格式。
pub fn render_late_lowered_program(program: &LateLoweredProgram) -> String {
    let mut rendered = String::new();
    writeln!(&mut rendered, "LateLoweredProgram").unwrap();
    writeln!(
        &mut rendered,
        "  step_type_count: {}",
        program.step_types().len()
    )
    .unwrap();
    writeln!(
        &mut rendered,
        "  resume_interface_count: {}",
        program.resume_interfaces().len()
    )
    .unwrap();
    writeln!(
        &mut rendered,
        "  continuation_object_count: {}",
        program.continuation_objects().len()
    )
    .unwrap();
    writeln!(&mut rendered, "  callable_count: {}", program.len()).unwrap();

    writeln!(&mut rendered, "  step_types:").unwrap();
    if program.step_types().is_empty() {
        writeln!(&mut rendered, "    <none>").unwrap();
    } else {
        for step_type in program.step_types() {
            render_step_type(&mut rendered, step_type);
        }
    }

    writeln!(&mut rendered, "  resume_interfaces:").unwrap();
    if program.resume_interfaces().is_empty() {
        writeln!(&mut rendered, "    <none>").unwrap();
    } else {
        for interface in program.resume_interfaces() {
            render_resume_interface(&mut rendered, interface);
        }
    }

    writeln!(&mut rendered, "  continuation_objects:").unwrap();
    if program.continuation_objects().is_empty() {
        writeln!(&mut rendered, "    <none>").unwrap();
    } else {
        for object in program.continuation_objects() {
            render_continuation_object(&mut rendered, object);
        }
    }

    writeln!(&mut rendered, "  callables:").unwrap();
    if program.is_empty() {
        writeln!(&mut rendered, "    <none>").unwrap();
    } else {
        for callable in program.callables() {
            render_callable(&mut rendered, callable);
        }
    }

    rendered
}

fn render_step_type(rendered: &mut String, step_type: &LateLoweredStepType) {
    writeln!(
        rendered,
        "    - step_schema: s{}",
        step_type.step_schema().as_u32()
    )
    .unwrap();
    writeln!(
        rendered,
        "      invoke_args_tuple_ty: {}",
        render_type_id(step_type.invoke_args_tuple_ty())
    )
    .unwrap();
    writeln!(
        rendered,
        "      complete_variant: Complete({})",
        render_type_id(step_type.complete_ty())
    )
    .unwrap();
    writeln!(
        rendered,
        "      continuation_obj_ty: {}",
        render_type_id(step_type.continuation_obj_ty())
    )
    .unwrap();
    writeln!(rendered, "      case_variants:").unwrap();
    if step_type.cases().is_empty() {
        writeln!(rendered, "        <none>").unwrap();
        return;
    }
    for case in step_type.cases() {
        render_step_case(rendered, case);
    }
}

fn render_step_case(rendered: &mut String, case: &LateLoweredStepCase) {
    writeln!(
        rendered,
        "        - Case(c{}) payload_tuple_ty={} continuation_schema=k{} concrete_op={}",
        case.case_tag().as_u32(),
        render_type_id(case.payload_tuple_ty()),
        case.continuation_schema().as_u32(),
        render_concrete_op_key(case.concrete_op_key()),
    )
    .unwrap();
}

fn render_resume_interface(rendered: &mut String, interface: &LateLoweredResumeInterface) {
    writeln!(
        rendered,
        "    - resume_interface: ri{}",
        interface.interface_id().as_u32()
    )
    .unwrap();
    writeln!(
        rendered,
        "      return_step_schema: s{}",
        interface.return_step_schema().as_u32()
    )
    .unwrap();
    writeln!(rendered, "      methods:").unwrap();
    if interface.methods().is_empty() {
        writeln!(rendered, "        <none>").unwrap();
        return;
    }
    for method in interface.methods() {
        render_resume_method(rendered, method);
    }
}

fn render_resume_method(rendered: &mut String, method: &LateLoweredResumeMethod) {
    writeln!(
        rendered,
        "        - case: c{} resume_tuple_ty={} continuation_schema=k{} concrete_op={}",
        method.case_tag().as_u32(),
        render_type_id(method.resume_tuple_ty()),
        method.continuation_schema().as_u32(),
        render_concrete_op_key(method.concrete_op_key()),
    )
    .unwrap();
}

fn render_continuation_object(rendered: &mut String, object: &LateLoweredContinuationObject) {
    writeln!(
        rendered,
        "    - continuation_object: ko{}",
        object.object_id().as_u32()
    )
    .unwrap();
    writeln!(
        rendered,
        "      owner_version: {}",
        render_body_version_key(object.owner_version_key())
    )
    .unwrap();
    writeln!(
        rendered,
        "      continuation_obj_ty: {}",
        render_type_id(object.continuation_obj_ty())
    )
    .unwrap();
    writeln!(
        rendered,
        "      implemented_interfaces: {}",
        render_resume_interface_ids(object.implemented_interfaces())
    )
    .unwrap();
    writeln!(rendered, "      captures:").unwrap();
    if object.captures().is_empty() {
        writeln!(rendered, "        <none>").unwrap();
    } else {
        for capture in object.captures() {
            writeln!(rendered, "        - {}", render_capture(*capture)).unwrap();
        }
    }
    writeln!(rendered, "      methods:").unwrap();
    if object.methods().is_empty() {
        writeln!(rendered, "        <none>").unwrap();
        return;
    }
    for method in object.methods() {
        render_continuation_method(rendered, method);
    }
}

fn render_continuation_method(rendered: &mut String, method: &LateLoweredContinuationMethod) {
    writeln!(
        rendered,
        "        - ri{}::c{} => {}",
        method.interface_id().as_u32(),
        method.case_tag().as_u32(),
        render_method_reachability(method.reachability()),
    )
    .unwrap();
}

fn render_callable(rendered: &mut String, callable: &LateLoweredCallable) {
    writeln!(rendered, "    - root: {}", callable.root_fqn()).unwrap();
    writeln!(
        rendered,
        "      body_version_key: {}",
        render_body_version_key(callable.body_version_key())
    )
    .unwrap();
    writeln!(
        rendered,
        "      step_schema: s{}",
        callable.step_schema().as_u32()
    )
    .unwrap();
    writeln!(
        rendered,
        "      resolved_outward_cases: {}",
        render_cases(callable.resolved_outward_cases())
    )
    .unwrap();
    writeln!(
        rendered,
        "      dynamic_invoke_entry: invoke({}) -> s{}",
        render_type_id(callable.dynamic_invoke_entry().invoke_args_tuple_ty()),
        callable.dynamic_invoke_entry().step_schema().as_u32(),
    )
    .unwrap();
    render_state_graph(rendered, callable.state_graph());
    render_frame_schema(rendered, callable.frame_schema());
    render_boundary_map(rendered, callable.boundary_map().entries());
    render_resume_state_map(rendered, callable.resume_state_map());
    writeln!(
        rendered,
        "      resume_interfaces: {}",
        render_resume_interface_ids(callable.resume_interfaces())
    )
    .unwrap();
    writeln!(
        rendered,
        "      continuation_object: ko{}",
        callable.continuation_object().as_u32()
    )
    .unwrap();
}

fn render_state_graph(rendered: &mut String, state_graph: &LateLoweredStateGraph) {
    writeln!(rendered, "      state_graph:").unwrap();
    writeln!(
        rendered,
        "        entry_state: st{}",
        state_graph.entry_state().as_u32()
    )
    .unwrap();
    writeln!(
        rendered,
        "        complete_state: st{}",
        state_graph.complete_state().as_u32()
    )
    .unwrap();
    writeln!(
        rendered,
        "        cleanup_state: {}",
        render_optional_state(state_graph.cleanup_state())
    )
    .unwrap();
    writeln!(
        rendered,
        "        drop_state: {}",
        render_optional_state(state_graph.drop_state())
    )
    .unwrap();
    writeln!(rendered, "        states:").unwrap();
    if state_graph.states().is_empty() {
        writeln!(rendered, "          <none>").unwrap();
        return;
    }
    for state in state_graph.states() {
        render_state(rendered, state);
    }
}

fn render_state(rendered: &mut String, state: &LateLoweredState) {
    writeln!(
        rendered,
        "          - st{} {} successors={}",
        state.state_id().as_u32(),
        render_state_role(state.role()),
        render_state_successors(state.successors())
    )
    .unwrap();
    writeln!(rendered, "            source_slices:").unwrap();
    if state.source_slices().is_empty() {
        writeln!(rendered, "              <synthetic>").unwrap();
        return;
    }
    for slice in state.source_slices() {
        render_state_slice(rendered, *slice);
    }
}

fn render_state_slice(rendered: &mut String, slice: LateLoweredStateSlice) {
    let terminator = if slice.includes_terminator() {
        " + term"
    } else {
        ""
    };
    writeln!(
        rendered,
        "              - bb{} stmts[{}..{}]{terminator}",
        slice.block_id().as_u32(),
        slice.start_statement_index(),
        slice.end_statement_index(),
    )
    .unwrap();
}

fn render_frame_schema(rendered: &mut String, frame_schema: &LateLoweredFrameSchema) {
    writeln!(rendered, "      frame_schema:").unwrap();
    writeln!(rendered, "        slots:").unwrap();
    if frame_schema.slots().is_empty() {
        writeln!(rendered, "          <none>").unwrap();
        return;
    }
    for slot in frame_schema.slots() {
        render_frame_slot(rendered, slot);
    }
}

fn render_frame_slot(rendered: &mut String, slot: &LateLoweredFrameSlot) {
    writeln!(
        rendered,
        "          - slot{} {} ty={}",
        slot.slot_id().as_u32(),
        render_frame_slot_kind(slot.kind()),
        render_type_id(slot.ty()),
    )
    .unwrap();
}

fn render_boundary_map(rendered: &mut String, boundaries: &[LateLoweredBoundary]) {
    writeln!(rendered, "      boundary_map:").unwrap();
    if boundaries.is_empty() {
        writeln!(rendered, "        <none>").unwrap();
        return;
    }
    for boundary in boundaries {
        writeln!(
            rendered,
            "        - bd{} {} owner=st{} resume=st{}",
            boundary.boundary_id().as_u32(),
            render_boundary_source(boundary.source()),
            boundary.owner_state().as_u32(),
            boundary.resume_state().as_u32(),
        )
        .unwrap();
    }
}

fn render_resume_state_map(rendered: &mut String, resume_state_map: &LateLoweredResumeStateMap) {
    writeln!(rendered, "      resume_state_map:").unwrap();
    if resume_state_map.entries().is_empty() {
        writeln!(rendered, "        <none>").unwrap();
        return;
    }
    for entry in resume_state_map.entries() {
        writeln!(
            rendered,
            "        - bd{} -> st{}",
            entry.boundary_id().as_u32(),
            entry.state_id().as_u32(),
        )
        .unwrap();
    }
}

fn render_body_version_key(key: &LateLoweredBodyVersionKey) -> String {
    format!(
        "instance={} allowed_row={} impl_plan={} needs_reentry={}",
        render_instance_key(key.surface_instance()),
        render_effect_row(key.allowed_row()),
        render_impl_plan(key.impl_plan()),
        key.needs_reentry(),
    )
}

fn render_instance_key(key: &crate::mir::InstanceKey) -> String {
    let mut args = key
        .type_args
        .iter()
        .copied()
        .map(render_type_id)
        .collect::<Vec<_>>();
    args.extend(
        key.eff_args
            .iter()
            .map(|row| format!("eff {}", render_effect_row(row))),
    );
    if args.is_empty() {
        key.template.fqn.clone()
    } else {
        format!("{}<{}>", key.template.fqn, args.join(", "))
    }
}

fn render_concrete_op_key(key: &ConcreteOpKey) -> String {
    render_instance_key(key.instance_key())
}

fn render_resume_interface_ids(interface_ids: &[ResumeInterfaceId]) -> String {
    if interface_ids.is_empty() {
        return "[]".to_string();
    }
    format!(
        "[{}]",
        interface_ids
            .iter()
            .map(|id| format!("ri{}", id.as_u32()))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn render_capture(capture: LateLoweredContinuationCapture) -> String {
    match capture {
        LateLoweredContinuationCapture::FrameSlot(slot) => {
            format!("FrameSlot(slot{})", slot.as_u32())
        }
        LateLoweredContinuationCapture::State(state) => format!("State(st{})", state.as_u32()),
    }
}

fn render_method_reachability(
    reachability: LateLoweredContinuationMethodReachability,
) -> &'static str {
    match reachability {
        LateLoweredContinuationMethodReachability::Reachable => "reachable",
        LateLoweredContinuationMethodReachability::Unreachable => "unreachable",
    }
}

fn render_boundary_source(source: LateLoweredBoundarySource) -> String {
    match source {
        LateLoweredBoundarySource::Site { site_id, kind } => {
            format!(
                "{}(site{})",
                render_boundary_site_kind(kind),
                site_id.as_u32()
            )
        }
        LateLoweredBoundarySource::RuntimeError { origin_site } => {
            format!("RuntimeError(site{})", origin_site.as_u32())
        }
    }
}

fn render_boundary_site_kind(kind: BoundarySiteKind) -> &'static str {
    match kind {
        BoundarySiteKind::Call => "Call",
        BoundarySiteKind::Perform => "Perform",
        BoundarySiteKind::Resume => "Resume",
        BoundarySiteKind::Handle => "Handle",
    }
}

fn render_state_role(role: LateLoweredStateRole) -> &'static str {
    match role {
        LateLoweredStateRole::Entry => "Entry",
        LateLoweredStateRole::Segment => "Segment",
        LateLoweredStateRole::Resume => "Resume",
        LateLoweredStateRole::Complete => "Complete",
        LateLoweredStateRole::Cleanup => "Cleanup",
        LateLoweredStateRole::Drop => "Drop",
    }
}

fn render_frame_slot_kind(kind: LateLoweredFrameSlotKind) -> String {
    match kind {
        LateLoweredFrameSlotKind::SourceLocal(local) => {
            format!("SourceLocal(local{})", local.as_u32())
        }
        LateLoweredFrameSlotKind::CompilerTemporary(local) => {
            format!("CompilerTemporary(local{})", local.as_u32())
        }
        LateLoweredFrameSlotKind::JoinValue { block, ordinal } => {
            format!("JoinValue(bb{}, #{ordinal})", block.as_u32())
        }
        LateLoweredFrameSlotKind::HandleBinder(site) => {
            format!("HandleBinder(site{})", site.as_u32())
        }
        LateLoweredFrameSlotKind::ResumePayload(boundary) => {
            format!("ResumePayload(bd{})", boundary.as_u32())
        }
        LateLoweredFrameSlotKind::System(system) => render_system_slot_kind(system).to_string(),
    }
}

fn render_system_slot_kind(kind: SystemSlotKind) -> &'static str {
    match kind {
        SystemSlotKind::StateTag => "StateTag",
        SystemSlotKind::ResumePayloadCarrier => "ResumePayloadCarrier",
        SystemSlotKind::CleanupFlag => "CleanupFlag",
        SystemSlotKind::OneShotFlag => "OneShotFlag",
        SystemSlotKind::CompletionTag => "CompletionTag",
    }
}

fn render_optional_state(state: Option<StateId>) -> String {
    state
        .map(|state| format!("st{}", state.as_u32()))
        .unwrap_or_else(|| "<none>".to_string())
}

fn render_state_successors(successors: &[StateId]) -> String {
    if successors.is_empty() {
        return "[]".to_string();
    }
    format!(
        "[{}]",
        successors
            .iter()
            .map(|state| format!("st{}", state.as_u32()))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn render_impl_plan(plan: ImplPlan) -> String {
    match plan {
        ImplPlan::NoOutward => "NoOutward".to_string(),
        ImplPlan::SingleCase(tag) => format!("SingleCase(c{})", tag.as_u32()),
        ImplPlan::CanonicalFull => "CanonicalFull".to_string(),
    }
}

fn render_cases(cases: &[CaseTag]) -> String {
    if cases.is_empty() {
        return "[]".to_string();
    }
    let rendered = cases
        .iter()
        .map(|tag| format!("c{}", tag.as_u32()))
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{rendered}]")
}

fn render_effect_row(row: &EffectRow) -> String {
    if row.is_pure() {
        return "Pure".to_string();
    }
    let rendered = row
        .terms
        .iter()
        .copied()
        .map(render_type_id)
        .collect::<Vec<_>>()
        .join(" + ");
    format!("({rendered})")
}

fn render_type_id(ty: TypeId) -> String {
    format!("t{}", ty.as_u32())
}
