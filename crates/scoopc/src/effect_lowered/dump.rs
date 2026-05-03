use std::fmt::Write;

use crate::effect_facts::{CaseTag, ConcreteOpKey, EffectFamilyKey, ImplPlan};
use crate::ty::{EffectRow, TypeId};

use super::ir::{
    BoundarySiteKind, LateLoweredBodyVersionKey, LateLoweredBoundary, LateLoweredBoundaryLowering,
    LateLoweredBoundarySource, LateLoweredCallable, LateLoweredCompleteStepDispatch,
    LateLoweredConsumedRuntimeErrorCase, LateLoweredContinuationCapture,
    LateLoweredContinuationMethod, LateLoweredContinuationObject,
    LateLoweredContinuationResumeBody, LateLoweredContinuationSurfaceResume,
    LateLoweredFrameSchema, LateLoweredFrameSlot, LateLoweredFrameSlotKind,
    LateLoweredOneShotPolicy, LateLoweredProgram, LateLoweredResumeInterface,
    LateLoweredResumeMethod, LateLoweredResumeStateMap, LateLoweredState, LateLoweredStateGraph,
    LateLoweredStateRole, LateLoweredStateSlice, LateLoweredStateTerminator, LateLoweredStepCase,
    LateLoweredStepCaseEmission, LateLoweredStepCaseForwarding, LateLoweredStepDispatchPlan,
    LateLoweredStepType, ResumeInterfaceId, StateId, SystemSlotKind,
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
        "        - Case(c{}) payload_tuple_ty={} continuation_schema=k{} resume_tuple_ty={} answer_ty={} surface_ty={} out_step_schema=s{} concrete_op={}",
        case.case_tag().as_u32(),
        render_type_id(case.payload_tuple_ty()),
        case.continuation_schema().as_u32(),
        render_type_id(case.resume_tuple_ty()),
        render_type_id(case.answer_ty()),
        render_type_id(case.surface_ty()),
        case.out_step_schema().as_u32(),
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
        "      effect_family: {}",
        render_effect_family_key(interface.effect_family())
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
        "        - case: c{} resume_tuple_ty={} answer_ty={} surface_ty={} out_step_schema=s{} continuation_schema=k{} concrete_op={}",
        method.case_tag().as_u32(),
        render_type_id(method.resume_tuple_ty()),
        render_type_id(method.answer_ty()),
        render_type_id(method.surface_ty()),
        method.out_step_schema().as_u32(),
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
    writeln!(rendered, "      surface_resumes:").unwrap();
    if object.surface_resumes().is_empty() {
        writeln!(rendered, "        <none>").unwrap();
    } else {
        for surface_resume in object.surface_resumes() {
            render_surface_resume(rendered, surface_resume);
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
        "        - ri{}::c{} resume_tuple_ty={} answer_ty={} surface_ty={} out_step_schema=s{} continuation_schema=k{} concrete_op={} => {}",
        method.interface_id().as_u32(),
        method.case_tag().as_u32(),
        render_type_id(method.resume_tuple_ty()),
        render_type_id(method.answer_ty()),
        render_type_id(method.surface_ty()),
        method.out_step_schema().as_u32(),
        method.continuation_schema().as_u32(),
        render_concrete_op_key(method.concrete_op_key()),
        render_continuation_resume_body(method.body()),
    )
    .unwrap();
}

fn render_surface_resume(
    rendered: &mut String,
    surface_resume: &LateLoweredContinuationSurfaceResume,
) {
    writeln!(
        rendered,
        "        - c{} resume_tuple_ty={} answer_ty={} surface_ty={} out_step_schema=s{} continuation_schema=k{} concrete_op={} => {}",
        surface_resume.case_tag().as_u32(),
        render_type_id(surface_resume.resume_tuple_ty()),
        render_type_id(surface_resume.answer_ty()),
        render_type_id(surface_resume.surface_ty()),
        surface_resume.out_step_schema().as_u32(),
        surface_resume.continuation_schema().as_u32(),
        render_concrete_op_key(surface_resume.concrete_op_key()),
        render_continuation_resume_body(surface_resume.body()),
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
        "      dynamic_invoke_entry: invoke({}) -> s{} entry=st{} complete=st{}",
        render_type_id(callable.dynamic_invoke_entry().invoke_args_tuple_ty()),
        callable.dynamic_invoke_entry().step_schema().as_u32(),
        callable.dynamic_invoke_entry().entry_state().as_u32(),
        callable.dynamic_invoke_entry().complete_state().as_u32(),
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
        "          - st{} {} term={} successors={}",
        state.state_id().as_u32(),
        render_state_role(state.role()),
        render_state_terminator(state.terminator()),
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
        "          - slot{} {} ty={} writes={} reads={}",
        slot.slot_id().as_u32(),
        render_frame_slot_kind(slot.kind()),
        render_type_id(slot.ty()),
        render_state_successors(slot.write_points()),
        render_state_successors(slot.read_points()),
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
        if let Some(lowering) = boundary.lowering() {
            render_boundary_lowering(rendered, lowering);
        }
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

fn render_boundary_lowering(rendered: &mut String, lowering: &LateLoweredBoundaryLowering) {
    match lowering {
        LateLoweredBoundaryLowering::Call(lowering) => {
            writeln!(
                rendered,
                "          lowering: Call kind={:?} target_mode={:?} callee_step=s{} result=local{} target={}",
                lowering.facts().kind(),
                lowering.facts().target_mode(),
                lowering.facts().callee_schema().as_u32(),
                lowering.result_local().as_u32(),
                render_call_target(lowering.facts().target()),
            )
            .unwrap();
            if let Some(consumed_runtime_error_case) = lowering.consumed_runtime_error_case() {
                render_consumed_runtime_error_case(rendered, consumed_runtime_error_case);
            }
            render_step_dispatch_plan(rendered, lowering.dispatch());
        }
        LateLoweredBoundaryLowering::Perform(lowering) => {
            writeln!(
                rendered,
                "          lowering: Perform emitted_case=c{} captured_cont_schema=k{} payload_tuple_ty={}",
                lowering.facts().emitted_case().as_u32(),
                lowering.facts().captured_cont_schema().as_u32(),
                render_type_id(lowering.facts().payload_tuple_ty()),
            )
            .unwrap();
            render_step_case_emission(rendered, lowering.emitted_step());
        }
        LateLoweredBoundaryLowering::Resume(lowering) => {
            writeln!(
                rendered,
                "          lowering: Resume continuation_schema=k{} out_step_schema=s{} result=local{} runtime_error_boundary=bd{}",
                lowering.facts().continuation_schema().as_u32(),
                lowering.facts().out_step_schema().as_u32(),
                lowering.result_local().as_u32(),
                lowering.runtime_error_boundary().as_u32(),
            )
            .unwrap();
            render_step_dispatch_plan(rendered, lowering.dispatch());
        }
        LateLoweredBoundaryLowering::RuntimeError(lowering) => {
            writeln!(
                rendered,
                "          lowering: RuntimeError origin=site{} paired_resume=bd{}",
                lowering.origin_site().as_u32(),
                lowering.resume_boundary().as_u32(),
            )
            .unwrap();
            render_step_case_emission(rendered, lowering.emitted_step());
        }
        LateLoweredBoundaryLowering::Handle(lowering) => {
            writeln!(
                rendered,
                "          lowering: Handle result_ty={} classification={:?} handled={} body_outward={} finally_outward={}",
                render_type_id(lowering.facts().result_ty()),
                lowering.facts().nested_handle_classification(),
                render_cases(lowering.facts().handled_cases().tags()),
                render_cases(lowering.facts().body_outward_cases().tags()),
                render_cases(lowering.facts().finally_outward_cases().tags()),
            )
            .unwrap();
            writeln!(rendered, "            arm_outward_cases:").unwrap();
            if lowering.facts().arm_facts().is_empty() {
                writeln!(rendered, "              <none>").unwrap();
            } else {
                for arm in lowering.facts().arm_facts() {
                    writeln!(
                        rendered,
                        "              - handled=c{} continuation_schema=k{} outward={}",
                        arm.handled_case().as_u32(),
                        arm.continuation_schema().as_u32(),
                        render_cases(arm.arm_outward_cases().tags()),
                    )
                    .unwrap();
                }
            }
            writeln!(rendered, "            outward_emissions:").unwrap();
            if lowering.outward_emissions().is_empty() {
                writeln!(rendered, "              <none>").unwrap();
            } else {
                for emission in lowering.outward_emissions() {
                    render_step_case_emission(rendered, emission);
                }
            }
        }
    }
}

fn render_step_dispatch_plan(rendered: &mut String, dispatch: &LateLoweredStepDispatchPlan) {
    writeln!(
        rendered,
        "            dispatch_input_step_schema: s{}",
        dispatch.input_step_schema().as_u32()
    )
    .unwrap();
    render_complete_step_dispatch(rendered, dispatch.complete());
    writeln!(rendered, "            outward_cases:").unwrap();
    if dispatch.outward_cases().is_empty() {
        writeln!(rendered, "              <none>").unwrap();
    } else {
        for forwarding in dispatch.outward_cases() {
            render_step_case_forwarding(rendered, forwarding);
        }
    }
}

fn render_consumed_runtime_error_case(
    rendered: &mut String,
    runtime_error_case: &LateLoweredConsumedRuntimeErrorCase,
) {
    writeln!(
        rendered,
        "            consumed_runtime_error_case: in c{} op={} payload_tuple_ty={} target=st{}",
        runtime_error_case.input_case_tag().as_u32(),
        render_concrete_op_key(runtime_error_case.input_concrete_op_key()),
        render_type_id(runtime_error_case.payload_tuple_ty()),
        runtime_error_case.target_state().as_u32(),
    )
    .unwrap();
}

fn render_complete_step_dispatch(
    rendered: &mut String,
    complete: &LateLoweredCompleteStepDispatch,
) {
    writeln!(
        rendered,
        "            complete: answer_ty={} target=st{} result={}",
        render_type_id(complete.answer_ty()),
        complete.target_state().as_u32(),
        complete
            .result_local()
            .map(|local| format!("local{}", local.as_u32()))
            .unwrap_or_else(|| "<none>".to_string()),
    )
    .unwrap();
}

fn render_step_case_forwarding(rendered: &mut String, forwarding: &LateLoweredStepCaseForwarding) {
    writeln!(
        rendered,
        "              - in c{} op={} -> out c{} op={} payload_tuple_ty={} ko{} cont_schema=k{} out_step_schema=s{}",
        forwarding.input_case_tag().as_u32(),
        render_concrete_op_key(forwarding.input_concrete_op_key()),
        forwarding.emission().case_tag().as_u32(),
        render_concrete_op_key(forwarding.emission().concrete_op_key()),
        render_type_id(forwarding.emission().payload_tuple_ty()),
        forwarding.emission().continuation_object().as_u32(),
        forwarding
            .emission()
            .continuation_contract()
            .continuation_schema()
            .as_u32(),
        forwarding
            .emission()
            .continuation_contract()
            .out_step_schema()
            .as_u32(),
    )
    .unwrap();
}

fn render_step_case_emission(rendered: &mut String, emission: &LateLoweredStepCaseEmission) {
    writeln!(
        rendered,
        "            emit: c{} op={} payload_tuple_ty={} ko{} cont_schema=k{} out_step_schema=s{}",
        emission.case_tag().as_u32(),
        render_concrete_op_key(emission.concrete_op_key()),
        render_type_id(emission.payload_tuple_ty()),
        emission.continuation_object().as_u32(),
        emission
            .continuation_contract()
            .continuation_schema()
            .as_u32(),
        emission.continuation_contract().out_step_schema().as_u32(),
    )
    .unwrap();
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

fn render_effect_family_key(key: &EffectFamilyKey) -> String {
    if key.type_args().is_empty() {
        return key.effect_fqn().to_string();
    }
    format!(
        "{}<{}>",
        key.effect_fqn(),
        key.type_args()
            .iter()
            .copied()
            .map(render_type_id)
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn render_call_target(target: &crate::effect_facts::CallSiteTarget) -> String {
    match target {
        crate::effect_facts::CallSiteTarget::KnownInstance(instance) => {
            render_instance_key(instance)
        }
        crate::effect_facts::CallSiteTarget::CandidateSet(instances) => format!(
            "[{}]",
            instances
                .iter()
                .map(render_instance_key)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        crate::effect_facts::CallSiteTarget::DynamicFallback => "DynamicFallback".to_string(),
    }
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

fn render_continuation_resume_body(body: LateLoweredContinuationResumeBody) -> String {
    match body {
        LateLoweredContinuationResumeBody::ResumeCapturedState { repeated_resume } => {
            format!(
                "ResumeCapturedState(one_shot={})",
                render_one_shot_policy(repeated_resume)
            )
        }
        LateLoweredContinuationResumeBody::Unreachable => "Unreachable".to_string(),
    }
}

fn render_one_shot_policy(policy: LateLoweredOneShotPolicy) -> &'static str {
    match policy {
        LateLoweredOneShotPolicy::OrdinaryRuntimeErrorOutward => "RuntimeErrorOutward",
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

fn render_state_terminator(terminator: &LateLoweredStateTerminator) -> String {
    match terminator {
        LateLoweredStateTerminator::Suspend {
            boundary_ids,
            resume_state,
            local_runtime_error_states,
            cleanup_state,
            drop_state,
        } => format!(
            "Suspend(boundaries={}, resume=st{}, local_runtime_error={}, cleanup={}, drop={})",
            render_boundary_ids(boundary_ids),
            resume_state.as_u32(),
            render_state_successors(local_runtime_error_states),
            render_optional_state(*cleanup_state),
            render_optional_state(*drop_state),
        ),
        LateLoweredStateTerminator::Goto { target } => format!("Goto(st{})", target.as_u32()),
        LateLoweredStateTerminator::Branch {
            cond_local,
            then_state,
            else_state,
        } => format!(
            "Branch(local{} ? st{} : st{})",
            cond_local.as_u32(),
            then_state.as_u32(),
            else_state.as_u32(),
        ),
        LateLoweredStateTerminator::Return {
            value_local,
            complete_state,
        } => format!(
            "Return({} -> st{})",
            value_local
                .map(|local| format!("local{}", local.as_u32()))
                .unwrap_or_else(|| "Unit".to_string()),
            complete_state.as_u32(),
        ),
        LateLoweredStateTerminator::HandleDispatch {
            site_id,
            body_state,
            arm_states,
            finally_state,
            exit_state,
            boundary_ids,
            drop_state,
        } => format!(
            "Handle(site{} body=st{} arms={} finally={} exit=st{} boundaries={} drop={})",
            site_id.as_u32(),
            body_state.as_u32(),
            render_state_successors(arm_states),
            render_optional_state(*finally_state),
            exit_state.as_u32(),
            render_boundary_ids(boundary_ids),
            render_optional_state(*drop_state),
        ),
        LateLoweredStateTerminator::LocalRuntimeError { payload_tuple_ty } => {
            format!(
                "LocalRuntimeError(payload_tuple_ty={})",
                render_type_id(*payload_tuple_ty)
            )
        }
        LateLoweredStateTerminator::ResumeUnwind => "ResumeUnwind".to_string(),
        LateLoweredStateTerminator::Unreachable => "Unreachable".to_string(),
        LateLoweredStateTerminator::Abandon => "Abandon".to_string(),
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
        LateLoweredFrameSlotKind::JoinValue {
            local,
            block,
            ordinal,
        } => {
            format!(
                "JoinValue(local{}, bb{}, #{ordinal})",
                local.as_u32(),
                block.as_u32()
            )
        }
        LateLoweredFrameSlotKind::HandleBinder {
            site_id,
            local,
            ordinal,
        } => {
            format!(
                "HandleBinder(site{}, local{}, #{ordinal})",
                site_id.as_u32(),
                local.as_u32(),
            )
        }
        LateLoweredFrameSlotKind::ResumePayload { boundary, case_tag } => {
            format!(
                "ResumePayload(bd{}, c{})",
                boundary.as_u32(),
                case_tag.as_u32(),
            )
        }
        LateLoweredFrameSlotKind::BoundaryResult { boundary, local } => {
            format!(
                "BoundaryResult(bd{}, local{})",
                boundary.as_u32(),
                local.as_u32()
            )
        }
        LateLoweredFrameSlotKind::System(system) => render_system_slot_kind(system).to_string(),
    }
}

fn render_boundary_ids(boundary_ids: &[super::ir::BoundaryId]) -> String {
    if boundary_ids.is_empty() {
        return "[]".to_string();
    }
    format!(
        "[{}]",
        boundary_ids
            .iter()
            .map(|boundary| format!("bd{}", boundary.as_u32()))
            .collect::<Vec<_>>()
            .join(", ")
    )
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
