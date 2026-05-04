use std::fmt::Write;

use crate::effect_facts::{CaseTag, ConcreteOpKey, EffectFamilyKey, ImplPlan};
use crate::ty::{EffectRow, TypeId};

use super::ir::{
    BoundarySiteKind, LateLoweredBodyVersionKey, LateLoweredBoundary, LateLoweredBoundaryLowering,
    LateLoweredBoundarySource, LateLoweredBoundarySourceConsumption,
    LateLoweredCallBoundaryOperandContract, LateLoweredCallable, LateLoweredCallableAbi,
    LateLoweredCompleteStepDispatch, LateLoweredCompletionPayloadBinding,
    LateLoweredCompletionPayloadSource, LateLoweredConsumedRuntimeErrorCase,
    LateLoweredContinuationCapture, LateLoweredContinuationMethod, LateLoweredContinuationObject,
    LateLoweredContinuationResumeBody, LateLoweredContinuationSurfaceResume,
    LateLoweredFrameSchema, LateLoweredFrameSlot, LateLoweredFrameSlotKind,
    LateLoweredHandleBoundaryCaseRoutingAction, LateLoweredHandleDispatchContract,
    LateLoweredHandlePendingCompletion, LateLoweredHandleStateRegion,
    LateLoweredLocalRuntimeErrorTerminalAction, LateLoweredOneShotPolicy, LateLoweredOperandSource,
    LateLoweredOperandValueSource, LateLoweredPerformBoundaryOperandContract,
    LateLoweredPlainBodySlice, LateLoweredPlainCallSite, LateLoweredPlainCallable,
    LateLoweredProgram, LateLoweredPublishedRuntimeEntry, LateLoweredResumeBoundaryOperandContract,
    LateLoweredResumeInterface, LateLoweredResumeMethod, LateLoweredResumePayloadBinding,
    LateLoweredResumeStateMap, LateLoweredSourceStatementClassification,
    LateLoweredSourceStatementClassificationKind, LateLoweredState, LateLoweredStateRole,
    LateLoweredStateSlice, LateLoweredStateTerminator, LateLoweredStepCase,
    LateLoweredStepCaseEmission, LateLoweredStepCaseForwarding, LateLoweredStepDispatchPlan,
    LateLoweredStepType, LateLoweredSurfaceResumeDispatchInventoryEntry,
    LateLoweredSurfaceResumeDispatchPublication, LateLoweredSurfaceResumeDispatchSourceKind,
    LateLoweredSurfaceResumeWrapperCaseProjection,
    LateLoweredSurfaceResumeWrapperCompletePayloadSource,
    LateLoweredSurfaceResumeWrapperCompleteProjection, LateLoweredSurfaceResumeWrapperProjection,
    ResumeInterfaceId, StateId, SystemSlotKind,
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
        "  resume_packing_interface_count: {}",
        program.resume_packings().len()
    )
    .unwrap();
    writeln!(
        &mut rendered,
        "  continuation_object_count: {}",
        program.continuation_objects().len()
    )
    .unwrap();
    writeln!(
        &mut rendered,
        "  surface_resume_dispatch_count: {}",
        program.surface_resume_dispatch_inventory().len()
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

    writeln!(&mut rendered, "  continuation_objects:").unwrap();
    if program.continuation_objects().is_empty() {
        writeln!(&mut rendered, "    <none>").unwrap();
    } else {
        for object in program.continuation_objects() {
            render_continuation_object(&mut rendered, object);
        }
    }

    writeln!(
        &mut rendered,
        "  authoritative_surface_resume_dispatch_inventory:"
    )
    .unwrap();
    if program.surface_resume_dispatch_inventory().is_empty() {
        writeln!(&mut rendered, "    <none>").unwrap();
    } else {
        for entry in program.surface_resume_dispatch_inventory() {
            render_surface_resume_dispatch_inventory_entry(&mut rendered, entry);
        }
    }

    writeln!(&mut rendered, "  resume_packing_interfaces:").unwrap();
    if program.resume_packings().is_empty() {
        writeln!(&mut rendered, "    <none>").unwrap();
    } else {
        for interface in program.resume_packings() {
            render_resume_interface(&mut rendered, interface);
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
        "    - resume_packing_interface: ri{}",
        interface.interface_id().as_u32()
    )
    .unwrap();
    writeln!(
        rendered,
        "      packing_effect_family: {}",
        render_effect_family_key(interface.effect_family())
    )
    .unwrap();
    writeln!(
        rendered,
        "      authoritative_step_schema: s{}",
        interface.return_step_schema().as_u32()
    )
    .unwrap();
    writeln!(rendered, "      packed_methods:").unwrap();
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
        "      implemented_packings: {}",
        render_resume_interface_ids(object.implemented_packings())
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
    writeln!(rendered, "      authoritative_surface_resumes:").unwrap();
    if object.surface_resumes().is_empty() {
        writeln!(rendered, "        <none>").unwrap();
    } else {
        for surface_resume in object.surface_resumes() {
            render_surface_resume(rendered, surface_resume);
        }
    }
    writeln!(rendered, "      authoritative_internal_methods:").unwrap();
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
        "        - case=c{} packed_by=ri{} resume_tuple_ty={} answer_ty={} surface_ty={} out_step_schema=s{} continuation_schema=k{} concrete_op={} => {}",
        method.case_tag().as_u32(),
        method.packing_interface_id().as_u32(),
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
        "        - case=c{} resume_tuple_ty={} answer_ty={} surface_ty={} out_step_schema=s{} continuation_schema=k{} concrete_op={} => {}",
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

fn render_surface_resume_dispatch_inventory_entry(
    rendered: &mut String,
    entry: &LateLoweredSurfaceResumeDispatchInventoryEntry,
) {
    let contract = entry.contract();
    writeln!(
        rendered,
        "    - continuation_schema: k{} source={} resume_tuple_ty={} answer_ty={} out_step_schema=s{}",
        entry.continuation_schema().as_u32(),
        render_surface_resume_dispatch_source_kind(entry.source_kind()),
        render_type_id(contract.resume_tuple_ty()),
        render_type_id(contract.answer_ty()),
        contract.out_step_schema().as_u32(),
    )
    .unwrap();
    writeln!(rendered, "      publications:").unwrap();
    if entry.publications().is_empty() {
        writeln!(rendered, "        <none>").unwrap();
        return;
    }
    for publication in entry.publications() {
        writeln!(
            rendered,
            "        - {}",
            render_surface_resume_dispatch_publication(publication)
        )
        .unwrap();
    }
    if let Some(projection) = entry.wrapper_projection() {
        render_surface_resume_wrapper_projection(rendered, projection);
    }
}

fn render_surface_resume_dispatch_source_kind(
    kind: LateLoweredSurfaceResumeDispatchSourceKind,
) -> &'static str {
    match kind {
        LateLoweredSurfaceResumeDispatchSourceKind::ContinuationObjectMethod => {
            "ContinuationObjectMethod"
        }
        LateLoweredSurfaceResumeDispatchSourceKind::ResumeBoundaryOnly => "ResumeBoundaryOnly",
        LateLoweredSurfaceResumeDispatchSourceKind::HandleContinuationBinderOnly => {
            "HandleContinuationBinderOnly"
        }
        LateLoweredSurfaceResumeDispatchSourceKind::OwnerTrampolineMixed => "OwnerTrampolineMixed",
        LateLoweredSurfaceResumeDispatchSourceKind::Unreachable => "Unreachable",
    }
}

fn render_surface_resume_dispatch_publication(
    publication: &LateLoweredSurfaceResumeDispatchPublication,
) -> String {
    match publication {
        LateLoweredSurfaceResumeDispatchPublication::SurfaceCase {
            object_id,
            case_tag,
            reachability,
        } => format!(
            "surface_case ko{} case=c{} reachability={reachability:?}",
            object_id.as_u32(),
            case_tag.as_u32(),
        ),
        LateLoweredSurfaceResumeDispatchPublication::InternalMethod {
            object_id,
            packing_interface_id,
            case_tag,
            reachability,
        } => format!(
            "internal_method ko{} case=c{} packed_by=ri{} reachability={reachability:?}",
            object_id.as_u32(),
            case_tag.as_u32(),
            packing_interface_id.as_u32(),
        ),
        LateLoweredSurfaceResumeDispatchPublication::ResumeBoundary {
            owner_version_key,
            owner_continuation_object,
            site_id,
        } => format!(
            "resume_boundary {} ko{} site{}",
            render_body_version_key(owner_version_key),
            owner_continuation_object.as_u32(),
            site_id.as_u32(),
        ),
        LateLoweredSurfaceResumeDispatchPublication::HandleContinuationBinder {
            owner_version_key,
            owner_continuation_object,
            site_id,
            arm_ordinal,
            handled_case,
        } => format!(
            "handle_continuation_binder {} ko{} site{} arm#{} handled_case=c{}",
            render_body_version_key(owner_version_key),
            owner_continuation_object.as_u32(),
            site_id.as_u32(),
            arm_ordinal,
            handled_case.as_u32(),
        ),
    }
}

fn render_surface_resume_wrapper_projection(
    rendered: &mut String,
    projection: &LateLoweredSurfaceResumeWrapperProjection,
) {
    writeln!(rendered, "      wrapper_projection:").unwrap();
    writeln!(
        rendered,
        "        underlying_route: continuation_schema=k{} via {}",
        projection.underlying_route().continuation_schema().as_u32(),
        render_surface_resume_dispatch_publication(projection.underlying_route().publication()),
    )
    .unwrap();
    writeln!(
        rendered,
        "        owner_step_schema: s{}",
        projection.owner_step_schema().as_u32(),
    )
    .unwrap();
    writeln!(
        rendered,
        "        wrapper_step_schema: s{}",
        projection.wrapper_step_schema().as_u32(),
    )
    .unwrap();
    render_surface_resume_wrapper_complete_projection(rendered, projection.complete());
    writeln!(rendered, "        outward_cases:").unwrap();
    if projection.outward_cases().is_empty() {
        writeln!(rendered, "          <none>").unwrap();
    } else {
        for case in projection.outward_cases() {
            render_surface_resume_wrapper_case_projection(rendered, case);
        }
    }
}

fn render_surface_resume_wrapper_complete_projection(
    rendered: &mut String,
    complete: &LateLoweredSurfaceResumeWrapperCompleteProjection,
) {
    writeln!(
        rendered,
        "        complete: owner_answer_ty={} -> wrapper_answer_ty={} payload={}",
        render_type_id(complete.owner_answer_ty()),
        render_type_id(complete.wrapper_answer_ty()),
        render_surface_resume_wrapper_complete_payload_source(complete.payload_source()),
    )
    .unwrap();
}

fn render_surface_resume_wrapper_complete_payload_source(
    source: &LateLoweredSurfaceResumeWrapperCompletePayloadSource,
) -> String {
    match source {
        LateLoweredSurfaceResumeWrapperCompletePayloadSource::OwnerComplete { answer_ty } => {
            format!("owner_complete:{}", render_type_id(*answer_ty))
        }
        LateLoweredSurfaceResumeWrapperCompletePayloadSource::WrapperPayload(source) => {
            render_completion_payload_source(source)
        }
    }
}

fn render_surface_resume_wrapper_case_projection(
    rendered: &mut String,
    projection: &LateLoweredSurfaceResumeWrapperCaseProjection,
) {
    writeln!(
        rendered,
        "          - owner c{} op={} payload_tuple_ty={} -> wrapper c{} op={} payload_tuple_ty={} cont_schema=k{} out_step_schema=s{}",
        projection.owner_case_tag().as_u32(),
        render_concrete_op_key(projection.owner_concrete_op_key()),
        render_type_id(projection.owner_payload_tuple_ty()),
        projection.wrapper_case_tag().as_u32(),
        render_concrete_op_key(projection.wrapper_concrete_op_key()),
        render_type_id(projection.wrapper_payload_tuple_ty()),
        projection
            .wrapper_continuation_contract()
            .continuation_schema()
            .as_u32(),
        projection
            .wrapper_continuation_contract()
            .out_step_schema()
            .as_u32(),
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
        "      resolved_outward_cases: {}",
        render_cases(callable.resolved_outward_cases())
    )
    .unwrap();
    match callable.abi() {
        LateLoweredCallableAbi::Plain(plain) => render_plain_callable(rendered, plain),
        LateLoweredCallableAbi::EffectStep(_) => render_effect_step_callable(rendered, callable),
    }
}

fn render_plain_callable(rendered: &mut String, plain: &LateLoweredPlainCallable) {
    writeln!(rendered, "      abi: Plain").unwrap();
    writeln!(
        rendered,
        "      ordinary_signature: fn_ty={} params=[{}] return={}",
        render_type_id(plain.function_ty()),
        plain
            .param_tys()
            .iter()
            .copied()
            .map(render_type_id)
            .collect::<Vec<_>>()
            .join(", "),
        render_type_id(plain.return_ty()),
    )
    .unwrap();
    writeln!(rendered, "      plain_source_slices:").unwrap();
    if plain.body_slices().is_empty() {
        writeln!(rendered, "        <none>").unwrap();
    } else {
        for slice in plain.body_slices() {
            writeln!(rendered, "        - {}", render_plain_body_slice(*slice)).unwrap();
        }
    }
    writeln!(rendered, "      plain_call_sites:").unwrap();
    if plain.call_sites().is_empty() {
        writeln!(rendered, "        <none>").unwrap();
    } else {
        for call_site in plain.call_sites() {
            render_plain_call_site(rendered, call_site);
        }
    }
    writeln!(rendered, "      effect_step_handoff: <none>").unwrap();
}

fn render_plain_call_site(rendered: &mut String, call_site: &LateLoweredPlainCallSite) {
    let facts = call_site.facts();
    writeln!(
        rendered,
        "        - site{} {:?} target_mode={:?} target={} callee_abi={} invoke_args_tuple_ty={} callee_step_schema={} resolved_cases={} anchor=bb{} stmt{} dispatch={}",
        call_site.site_id().as_u32(),
        facts.kind(),
        facts.target_mode(),
        render_call_target(facts.target()),
        render_callable_abi_kind(facts.callee_abi_kind()),
        render_type_id(facts.invoke_args_tuple_ty()),
        facts
            .callee_step_schema()
            .map(|schema| format!("s{}", schema.as_u32()))
            .unwrap_or_else(|| "<none>".to_string()),
        render_cases(facts.resolved_cases().tags()),
        call_site.source_slice().block_id().as_u32(),
        call_site.statement_index(),
        match facts.callee_abi_kind() {
            crate::effect_facts::CallableAbiKind::Plain => "PlainCall",
            crate::effect_facts::CallableAbiKind::EffectStep => "EffectStepDispatch",
        },
    )
    .unwrap();
}

fn render_effect_step_callable(rendered: &mut String, callable: &LateLoweredCallable) {
    writeln!(rendered, "      abi: EffectStep").unwrap();
    writeln!(
        rendered,
        "      authoritative_step_schema: s{}",
        callable.step_schema().as_u32()
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
    render_state_graph(rendered, callable);
    render_frame_schema(rendered, callable.frame_schema());
    render_boundary_map(rendered, callable.boundary_map().entries());
    render_resume_state_map(rendered, callable.resume_state_map());
    writeln!(
        rendered,
        "      resume_packing_interfaces: {}",
        render_resume_interface_ids(callable.resume_packings())
    )
    .unwrap();
    writeln!(
        rendered,
        "      continuation_object: ko{}",
        callable.continuation_object().as_u32()
    )
    .unwrap();
}

fn render_plain_body_slice(slice: LateLoweredPlainBodySlice) -> String {
    let terminator = if slice.includes_terminator() {
        " + term"
    } else {
        ""
    };
    format!(
        "bb{} stmts[{}..{}]{terminator}",
        slice.block_id().as_u32(),
        slice.start_statement_index(),
        slice.end_statement_index(),
    )
}

fn render_state_graph(rendered: &mut String, callable: &LateLoweredCallable) {
    let state_graph = callable.state_graph();
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
        render_state(rendered, state, callable.source_statement_classifications());
    }
}

fn render_state(
    rendered: &mut String,
    state: &LateLoweredState,
    classifications: &[LateLoweredSourceStatementClassification],
) {
    writeln!(
        rendered,
        "          - st{} {} term={} successors={}",
        state.state_id().as_u32(),
        render_state_role(state.role()),
        render_state_terminator(state.terminator()),
        render_state_successors(state.successors())
    )
    .unwrap();
    if let LateLoweredStateTerminator::HandleDispatch { contract, .. } = state.terminator() {
        render_handle_dispatch_contract(rendered, contract);
    }
    writeln!(rendered, "            source_slices:").unwrap();
    if state.source_slices().is_empty() {
        writeln!(rendered, "              <synthetic>").unwrap();
        return;
    }
    for slice in state.source_slices() {
        render_state_slice(rendered, *slice);
        render_state_slice_classifications(rendered, *slice, classifications);
    }
}

fn render_state_slice_classifications(
    rendered: &mut String,
    slice: LateLoweredStateSlice,
    classifications: &[LateLoweredSourceStatementClassification],
) {
    writeln!(rendered, "                statement_classification:").unwrap();
    let mut rendered_any = false;
    for classification in classifications.iter().filter(|classification| {
        classification.source_slice() == slice
            && classification.statement_index() >= slice.start_statement_index()
            && classification.statement_index() < slice.end_statement_index()
    }) {
        rendered_any = true;
        writeln!(
            rendered,
            "                  - stmt{}: {}",
            classification.statement_index(),
            render_source_statement_classification_kind(classification.kind()),
        )
        .unwrap();
    }
    if !rendered_any {
        let marker = if slice.start_statement_index() == slice.end_statement_index() {
            "<none>"
        } else {
            "<unclassified>"
        };
        writeln!(rendered, "                  {marker}").unwrap();
    }
}

fn render_source_statement_classification_kind(
    kind: LateLoweredSourceStatementClassificationKind,
) -> String {
    match kind {
        LateLoweredSourceStatementClassificationKind::EffectNeutralValue => {
            "effect-neutral-value".to_string()
        }
        LateLoweredSourceStatementClassificationKind::BoundaryConsumedAnchor { boundary_id } => {
            format!("boundary-consumed-anchor bd{}", boundary_id.as_u32())
        }
        LateLoweredSourceStatementClassificationKind::ResumePayloadInjection {
            boundary_id,
            resume_state,
            consumer_local,
        } => format!(
            "resume-payload-injection bd{} resume=st{} local{}",
            boundary_id.as_u32(),
            resume_state.as_u32(),
            consumer_local.as_u32(),
        ),
        LateLoweredSourceStatementClassificationKind::BoundaryResultInjection {
            boundary_id,
            resume_state,
            result_local,
        } => format!(
            "boundary-result-injection bd{} resume=st{} local{}",
            boundary_id.as_u32(),
            resume_state.as_u32(),
            result_local.as_u32(),
        ),
        LateLoweredSourceStatementClassificationKind::CompletionPayloadInjection {
            return_state,
            complete_state,
        } => format!(
            "completion-payload-injection return=st{} complete=st{}",
            return_state.as_u32(),
            complete_state.as_u32(),
        ),
        LateLoweredSourceStatementClassificationKind::HandleSyntheticCarrierBinder {
            site_id,
            state_id,
        } => format!(
            "handle-synthetic-carrier-binder site{} state=st{}",
            site_id.as_u32(),
            state_id.as_u32(),
        ),
        LateLoweredSourceStatementClassificationKind::ElidedUnreachable => {
            "elided-unreachable".to_string()
        }
        LateLoweredSourceStatementClassificationKind::Unsupported { reason } => {
            format!("unsupported reason={reason}")
        }
    }
}

fn render_handle_dispatch_contract(
    rendered: &mut String,
    contract: &LateLoweredHandleDispatchContract,
) {
    writeln!(rendered, "            handle_contract:").unwrap();
    writeln!(
        rendered,
        "              carriers: state={} completion={} payload={}",
        render_system_slot_kind(contract.carrier().state_tag_slot()),
        render_system_slot_kind(contract.carrier().completion_tag_slot()),
        render_system_slot_kind(contract.carrier().payload_carrier_slot()),
    )
    .unwrap();
    writeln!(
        rendered,
        "              body_complete_target: st{}",
        contract.body_complete_target().as_u32(),
    )
    .unwrap();
    writeln!(
        rendered,
        "              arm_complete_target: st{}",
        contract.arm_complete_target().as_u32(),
    )
    .unwrap();
    writeln!(
        rendered,
        "              finally_complete_target: {}",
        render_optional_state(contract.finally_complete_target()),
    )
    .unwrap();
    let body_completion_payload = contract
        .body_completion_payload_source()
        .map(render_completion_payload_source)
        .unwrap_or_else(|| "<unpublished>".to_string());
    writeln!(
        rendered,
        "              body_completion_payload: {body_completion_payload}",
    )
    .unwrap();
    writeln!(
        rendered,
        "              abandon_target: {}",
        render_optional_state(contract.abandon_target()),
    )
    .unwrap();
    writeln!(
        rendered,
        "              body_outward_cases: {}",
        render_cases(contract.body_outward_cases()),
    )
    .unwrap();
    writeln!(
        rendered,
        "              finally_outward_cases: {}",
        render_cases(contract.finally_outward_cases()),
    )
    .unwrap();
    writeln!(rendered, "              handled_arms:").unwrap();
    if contract.handled_arms().is_empty() {
        writeln!(rendered, "                <none>").unwrap();
    } else {
        for arm in contract.handled_arms() {
            writeln!(
                rendered,
                "                - handled=c{} ordinal={} -> st{} payload_tuple_ty={} outward={}",
                arm.handled_case().as_u32(),
                arm.arm_ordinal(),
                arm.arm_state().as_u32(),
                render_type_id(arm.payload_tuple_ty()),
                render_cases(arm.arm_outward_cases()),
            )
            .unwrap();
            writeln!(rendered, "                  payload_binders:").unwrap();
            if arm.payload_binders().is_empty() {
                writeln!(rendered, "                    <none>").unwrap();
            } else {
                for binder in arm.payload_binders() {
                    writeln!(
                        rendered,
                        "                    - #{} local{} slot={}",
                        binder.ordinal(),
                        binder.local().as_u32(),
                        render_optional_frame_slot(binder.frame_slot()),
                    )
                    .unwrap();
                }
            }
            let continuation_binder = arm.continuation_binder().map_or_else(
                || "<none>".to_string(),
                |binder| {
                    format!(
                        "local{} slot={} continuation_schema=k{} continuation_object=ko{}",
                        binder.local().as_u32(),
                        render_optional_frame_slot(binder.frame_slot()),
                        binder.continuation_schema().as_u32(),
                        binder.continuation_object().as_u32(),
                    )
                },
            );
            writeln!(
                rendered,
                "                  continuation_binder: {continuation_binder}",
            )
            .unwrap();
            writeln!(
                rendered,
                "                  completion_payload: {}",
                render_completion_payload_source(arm.completion_payload_source()),
            )
            .unwrap();
        }
    }
    writeln!(rendered, "              pending_completions:").unwrap();
    if contract.pending_completions().is_empty() {
        writeln!(rendered, "                <none>").unwrap();
    } else {
        for pending in contract.pending_completions() {
            writeln!(
                rendered,
                "                - {}",
                render_handle_pending_completion(*pending),
            )
            .unwrap();
        }
    }
    writeln!(rendered, "              pending_payload_transports:").unwrap();
    if contract.pending_payload_transports().is_empty() {
        writeln!(rendered, "                <none>").unwrap();
    } else {
        for transport in contract.pending_payload_transports() {
            writeln!(
                rendered,
                "                - {} payload_tuple_ty={} frame_slot=fs{}",
                render_handle_pending_completion(transport.completion()),
                render_type_id(transport.payload_tuple_ty()),
                transport.frame_slot().as_u32(),
            )
            .unwrap();
        }
    }
    writeln!(rendered, "              state_regions:").unwrap();
    if contract.state_regions().is_empty() {
        writeln!(rendered, "                <none>").unwrap();
    } else {
        for entry in contract.state_regions() {
            writeln!(
                rendered,
                "                - st{} => {}",
                entry.state_id().as_u32(),
                render_handle_state_region(entry.region()),
            )
            .unwrap();
        }
    }
    writeln!(rendered, "              boundary_routings:").unwrap();
    if contract.boundary_routings().is_empty() {
        writeln!(rendered, "                <none>").unwrap();
    } else {
        for routing in contract.boundary_routings() {
            writeln!(
                rendered,
                "                - bd{} owner=st{} region={} resume=st{}",
                routing.boundary_id().as_u32(),
                routing.owner_state().as_u32(),
                render_handle_state_region(routing.owner_region()),
                routing.resume_state().as_u32(),
            )
            .unwrap();
            writeln!(rendered, "                  case_routings:").unwrap();
            if routing.case_routings().is_empty() {
                writeln!(rendered, "                    <none>").unwrap();
            } else {
                for route in routing.case_routings() {
                    writeln!(
                        rendered,
                        "                    - c{} => {}",
                        route.case_tag().as_u32(),
                        render_handle_boundary_case_routing_action(route.action()),
                    )
                    .unwrap();
                }
            }
        }
    }
    writeln!(rendered, "              outward_emissions:").unwrap();
    if contract.outward_emissions().is_empty() {
        writeln!(rendered, "                <none>").unwrap();
    } else {
        for emission in contract.outward_emissions() {
            writeln!(
                rendered,
                "                - c{} op={} payload_tuple_ty={} ko{} cont_schema=k{} out_step_schema=s{}",
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
    }
}

fn render_state_slice(rendered: &mut String, slice: LateLoweredStateSlice) {
    writeln!(
        rendered,
        "              - {}",
        render_state_slice_inline(slice)
    )
    .unwrap();
}

fn render_state_slice_inline(slice: LateLoweredStateSlice) -> String {
    let terminator = if slice.includes_terminator() {
        " + term"
    } else {
        ""
    };
    format!(
        "bb{} stmts[{}..{}]{terminator}",
        slice.block_id().as_u32(),
        slice.start_statement_index(),
        slice.end_statement_index(),
    )
}

fn render_frame_schema(rendered: &mut String, frame_schema: &LateLoweredFrameSchema) {
    writeln!(rendered, "      frame_schema:").unwrap();
    writeln!(rendered, "        slots:").unwrap();
    if frame_schema.slots().is_empty() {
        writeln!(rendered, "          <none>").unwrap();
    } else {
        for slot in frame_schema.slots() {
            render_frame_slot(rendered, slot);
        }
    }
    writeln!(rendered, "        resume_payload_bindings:").unwrap();
    if frame_schema.resume_payload_bindings().is_empty() {
        writeln!(rendered, "          <none>").unwrap();
    } else {
        for binding in frame_schema.resume_payload_bindings() {
            render_resume_payload_binding(rendered, binding);
        }
    }
    writeln!(rendered, "        completion_payload_bindings:").unwrap();
    if frame_schema.completion_payload_bindings().is_empty() {
        writeln!(rendered, "          <none>").unwrap();
    } else {
        for binding in frame_schema.completion_payload_bindings() {
            render_completion_payload_binding(rendered, binding);
        }
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

fn render_resume_payload_binding(rendered: &mut String, binding: &LateLoweredResumePayloadBinding) {
    writeln!(
        rendered,
        "          - bd{} resume=st{} local{} home={}",
        binding.boundary_id().as_u32(),
        binding.resume_state().as_u32(),
        binding.consumer_local().as_u32(),
        binding
            .consumer_frame_slot()
            .map(|slot| format!("slot{}", slot.as_u32()))
            .unwrap_or_else(|| "<none>".to_string()),
    )
    .unwrap();
}

fn render_completion_payload_binding(
    rendered: &mut String,
    binding: &LateLoweredCompletionPayloadBinding,
) {
    writeln!(
        rendered,
        "          - return=st{} complete=st{} payload={} home={}",
        binding.return_state().as_u32(),
        binding.complete_state().as_u32(),
        render_completion_payload_source(binding.payload_source()),
        render_optional_frame_slot(binding.payload_frame_slot()),
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
            render_call_operand_contract(rendered, lowering.operand_contract());
            if let Some(consumed_runtime_error_case) = lowering.consumed_runtime_error_case() {
                render_consumed_runtime_error_case(rendered, consumed_runtime_error_case);
            }
            render_call_boundary_continuation_compositions(
                rendered,
                lowering.continuation_compositions(),
            );
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
            render_perform_operand_contract(rendered, lowering.operand_contract());
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
            render_resume_operand_contract(rendered, lowering.operand_contract());
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
        "            consumed_runtime_error_case: in c{} op={} payload_tuple_ty={} target=st{} terminal={}",
        runtime_error_case.input_case_tag().as_u32(),
        render_concrete_op_key(runtime_error_case.input_concrete_op_key()),
        render_type_id(runtime_error_case.payload_tuple_ty()),
        runtime_error_case.target_state().as_u32(),
        render_local_runtime_error_terminal_action(runtime_error_case.terminal_action()),
    )
    .unwrap();
}

fn render_call_boundary_continuation_compositions(
    rendered: &mut String,
    compositions: &[crate::effect_lowered::ir::LateLoweredCallBoundaryContinuationComposition],
) {
    writeln!(rendered, "            continuation_compositions:").unwrap();
    if compositions.is_empty() {
        writeln!(rendered, "              <none>").unwrap();
        return;
    }
    for composition in compositions {
        writeln!(
            rendered,
            "              - in c{} -> out c{} callee=k{} caller=k{} resume=st{} result=local{}{} result_ty={}",
            composition.input_case_tag().as_u32(),
            composition.output_case_tag().as_u32(),
            composition.callee_continuation_schema().as_u32(),
            composition.caller_continuation_schema().as_u32(),
            composition.caller_resume_state().as_u32(),
            composition.caller_result_local().as_u32(),
            composition
                .caller_result_frame_slot()
                .map(|slot| format!(" frame=fs{}", slot.as_u32()))
                .unwrap_or_default(),
            render_type_id(composition.caller_result_ty()),
        )
        .unwrap();
    }
}

fn render_call_operand_contract(
    rendered: &mut String,
    contract: &LateLoweredCallBoundaryOperandContract,
) {
    writeln!(rendered, "            operand_contract:").unwrap();
    render_source_consumption(rendered, contract.source_consumption());
    writeln!(
        rendered,
        "              carrier: {}",
        contract
            .carrier_source()
            .map(render_operand_source)
            .unwrap_or_else(|| "<none>".to_string()),
    )
    .unwrap();
    writeln!(rendered, "              ordered_args:").unwrap();
    render_operand_sources(rendered, contract.arg_sources());
}

fn render_perform_operand_contract(
    rendered: &mut String,
    contract: &LateLoweredPerformBoundaryOperandContract,
) {
    writeln!(rendered, "            operand_contract:").unwrap();
    render_source_consumption(rendered, contract.source_consumption());
    writeln!(rendered, "              payload_sources:").unwrap();
    render_operand_sources(rendered, contract.payload_sources());
}

fn render_resume_operand_contract(
    rendered: &mut String,
    contract: &LateLoweredResumeBoundaryOperandContract,
) {
    writeln!(rendered, "            operand_contract:").unwrap();
    render_source_consumption(rendered, contract.source_consumption());
    writeln!(
        rendered,
        "              continuation: {}",
        render_operand_source(contract.continuation_source()),
    )
    .unwrap();
    let route = contract.underlying_continuation_route();
    writeln!(
        rendered,
        "              underlying_route: continuation_schema=k{} via {}",
        route.continuation_schema().as_u32(),
        render_surface_resume_dispatch_publication(route.publication()),
    )
    .unwrap();
    writeln!(rendered, "              ordered_args:").unwrap();
    render_operand_sources(rendered, contract.arg_sources());
}

fn render_source_consumption(
    rendered: &mut String,
    consumption: LateLoweredBoundarySourceConsumption,
) {
    match consumption {
        LateLoweredBoundarySourceConsumption::Statement {
            source_slice,
            statement_index,
            consumes_last_statement,
        } => {
            writeln!(
                rendered,
                "              anchor: statement bb{} stmt{} slice={} slice_stmt_index={} last_in_slice={}",
                source_slice.block_id().as_u32(),
                statement_index,
                render_state_slice_inline(source_slice),
                statement_index.saturating_sub(source_slice.start_statement_index()),
                consumes_last_statement,
            )
            .unwrap();
        }
        LateLoweredBoundarySourceConsumption::Terminator { source_slice } => {
            writeln!(
                rendered,
                "              anchor: terminator slice={}",
                render_state_slice_inline(source_slice),
            )
            .unwrap();
        }
    }
}

fn render_operand_sources(rendered: &mut String, sources: &[LateLoweredOperandSource]) {
    if sources.is_empty() {
        writeln!(rendered, "                <none>").unwrap();
        return;
    }
    for source in sources {
        writeln!(
            rendered,
            "                - {}",
            render_operand_source(source)
        )
        .unwrap();
    }
}

fn render_operand_source(source: &LateLoweredOperandSource) -> String {
    let value = match source.value() {
        LateLoweredOperandValueSource::Local(local) => format!("local{}", local.as_u32()),
        LateLoweredOperandValueSource::Const(value) => format!("const({value:?})"),
    };
    format!("{value}:{}", render_type_id(source.source_ty()))
}

fn render_completion_payload_source(source: &LateLoweredCompletionPayloadSource) -> String {
    match source {
        LateLoweredCompletionPayloadSource::Unit { complete_ty } => {
            format!("Unit:{}", render_type_id(*complete_ty))
        }
        LateLoweredCompletionPayloadSource::Operand(source) => render_operand_source(source),
    }
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

fn render_callable_abi_kind(kind: crate::effect_facts::CallableAbiKind) -> &'static str {
    match kind {
        crate::effect_facts::CallableAbiKind::Plain => "Plain",
        crate::effect_facts::CallableAbiKind::EffectStep => "EffectStep",
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
            payload_source,
            complete_state,
        } => format!(
            "Return({} -> st{})",
            render_completion_payload_source(payload_source),
            complete_state.as_u32(),
        ),
        LateLoweredStateTerminator::HandleDispatch {
            site_id,
            body_state,
            arm_states,
            finally_state,
            exit_state,
            contract: _,
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
        LateLoweredStateTerminator::LocalRuntimeError {
            payload_tuple_ty,
            terminal_action,
        } => {
            format!(
                "LocalRuntimeError(payload_tuple_ty={}, terminal={})",
                render_type_id(*payload_tuple_ty),
                render_local_runtime_error_terminal_action(*terminal_action)
            )
        }
        LateLoweredStateTerminator::ResumeUnwind => "ResumeUnwind".to_string(),
        LateLoweredStateTerminator::Unreachable => "Unreachable".to_string(),
        LateLoweredStateTerminator::Abandon => "Abandon".to_string(),
    }
}

fn render_local_runtime_error_terminal_action(
    action: LateLoweredLocalRuntimeErrorTerminalAction,
) -> String {
    match action {
        LateLoweredLocalRuntimeErrorTerminalAction::RuntimeFatal { runtime_entry } => {
            format!(
                "RuntimeFatal(runtime_entry={})",
                render_published_runtime_entry(runtime_entry)
            )
        }
    }
}

fn render_handle_pending_completion(pending: LateLoweredHandlePendingCompletion) -> String {
    match pending {
        LateLoweredHandlePendingCompletion::ContinueToExit => "ContinueToExit".to_string(),
        LateLoweredHandlePendingCompletion::ReturnFromFunction => "ReturnFromFunction".to_string(),
        LateLoweredHandlePendingCompletion::PropagateOutward(case_tag) => {
            format!("PropagateOutward(c{})", case_tag.as_u32())
        }
    }
}

fn render_handle_state_region(region: LateLoweredHandleStateRegion) -> String {
    match region {
        LateLoweredHandleStateRegion::OutsideHandle => "outside".to_string(),
        LateLoweredHandleStateRegion::Dispatch => "dispatch".to_string(),
        LateLoweredHandleStateRegion::Body => "body".to_string(),
        LateLoweredHandleStateRegion::Arm {
            handled_case,
            arm_ordinal,
        } => format!("arm(c{}, ordinal={arm_ordinal})", handled_case.as_u32()),
        LateLoweredHandleStateRegion::Finally => "finally".to_string(),
        LateLoweredHandleStateRegion::Exit => "exit".to_string(),
    }
}

fn render_handle_boundary_case_routing_action(
    action: LateLoweredHandleBoundaryCaseRoutingAction,
) -> String {
    match action {
        LateLoweredHandleBoundaryCaseRoutingAction::ConsumeToArm {
            arm_state,
            arm_ordinal,
            continuation_resume_state,
        } => format!(
            "consume_to_arm(st{}, ordinal={}, resume=st{})",
            arm_state.as_u32(),
            arm_ordinal,
            continuation_resume_state.as_u32(),
        ),
        LateLoweredHandleBoundaryCaseRoutingAction::PendingCompletion { completion } => {
            format!("pending:{}", render_handle_pending_completion(completion))
        }
        LateLoweredHandleBoundaryCaseRoutingAction::EmitOutward => "emit_outward".to_string(),
    }
}

fn render_published_runtime_entry(entry: LateLoweredPublishedRuntimeEntry) -> &'static str {
    entry.symbol_name()
}

fn render_optional_frame_slot(slot: Option<super::ir::FrameSlotId>) -> String {
    slot.map_or_else(
        || "<none>".to_string(),
        |slot| format!("fs{}", slot.as_u32()),
    )
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
        LateLoweredFrameSlotKind::HandlePendingPayload { site_id, case_tag } => {
            format!(
                "HandlePendingPayload(site{}, c{})",
                site_id.as_u32(),
                case_tag.as_u32(),
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
