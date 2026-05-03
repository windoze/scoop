use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::effect_facts::{
    BodyEffectFacts, CallSiteEffectFacts, CaseTag, ConcreteOpKey, EffectFamilyKey,
    HandleSiteEffectFacts, ImplPlan, MaterializedEffectFacts, PerformSiteEffectFacts,
    ResumeSiteEffectFacts, SiteEffectFacts, StepCaseFact, StepSchema, StepSchemaId,
};
use crate::mir::{Body, CallKind, LocalId, ResumeMetadata, Rvalue, SiteId, StatementKind};
use crate::ty::{NominalType, RefTypeKind, TypeKind, TypeStore};

use super::EffectLoweringError;
use super::ir::{
    BoundaryId, BoundarySiteKind, ContinuationObjectId, LateLoweredBoundaryLowering,
    LateLoweredBoundaryMap, LateLoweredCallBoundaryLowering, LateLoweredCompleteStepDispatch,
    LateLoweredConsumedRuntimeErrorCase, LateLoweredContinuationCapture,
    LateLoweredContinuationContract, LateLoweredContinuationMethod, LateLoweredContinuationObject,
    LateLoweredContinuationResumeBody, LateLoweredContinuationSurfaceResume,
    LateLoweredDynamicInvokeEntry, LateLoweredFrameSchema, LateLoweredHandleArmDispatch,
    LateLoweredHandleBoundaryCaseRouting, LateLoweredHandleBoundaryCaseRoutingAction,
    LateLoweredHandleBoundaryLowering, LateLoweredHandleBoundaryRouting,
    LateLoweredHandleContinuationBinder, LateLoweredHandleDispatchCarrierContract,
    LateLoweredHandleDispatchContract, LateLoweredHandlePayloadBinder,
    LateLoweredHandlePendingCompletion, LateLoweredHandleStateRegion,
    LateLoweredHandleStateRegionEntry, LateLoweredLocalRuntimeErrorTerminalAction,
    LateLoweredOneShotPolicy, LateLoweredPerformBoundaryLowering, LateLoweredPublishedRuntimeEntry,
    LateLoweredResumeBoundaryLowering, LateLoweredResumeInterface, LateLoweredResumeMethod,
    LateLoweredRuntimeErrorBoundaryLowering, LateLoweredState, LateLoweredStateGraph,
    LateLoweredStateRole, LateLoweredStateTerminator, LateLoweredStepCase,
    LateLoweredStepCaseEmission, LateLoweredStepCaseForwarding, LateLoweredStepDispatchPlan,
    LateLoweredStepType, ResumeInterfaceId, StateId,
};
use super::ir::{LateLoweredBodyVersionKey, LateLoweredBoundarySource};

pub(crate) struct StepMaterialization {
    pub(crate) step_types: Vec<LateLoweredStepType>,
    pub(crate) resume_interfaces: Vec<LateLoweredResumeInterface>,
    pub(crate) resume_interface_ids_by_step: BTreeMap<StepSchemaId, Vec<ResumeInterfaceId>>,
    pub(crate) resume_interface_ids_by_group:
        BTreeMap<(StepSchemaId, EffectFamilyKey), ResumeInterfaceId>,
}

pub(crate) struct BoundaryMaterializationInputs<'a> {
    pub(crate) root_fqn: &'a str,
    pub(crate) body: &'a Body,
    pub(crate) body_facts: &'a BodyEffectFacts,
    pub(crate) step_type: &'a LateLoweredStepType,
    pub(crate) state_graph: &'a LateLoweredStateGraph,
    pub(crate) frame_schema: &'a LateLoweredFrameSchema,
    pub(crate) boundary_map: &'a LateLoweredBoundaryMap,
    pub(crate) continuation_object: ContinuationObjectId,
    pub(crate) step_types: &'a [LateLoweredStepType],
    pub(crate) types: &'a TypeStore,
}

pub(crate) struct ContinuationObjectMaterializationInputs<'a> {
    pub(crate) continuation_object_id: ContinuationObjectId,
    pub(crate) owner_version_key: LateLoweredBodyVersionKey,
    pub(crate) step_schema_id: StepSchemaId,
    pub(crate) step_schema: &'a StepSchema,
    pub(crate) implemented_interfaces: &'a [ResumeInterfaceId],
    pub(crate) resume_interface_ids_by_group:
        &'a BTreeMap<(StepSchemaId, EffectFamilyKey), ResumeInterfaceId>,
    pub(crate) captures: Vec<LateLoweredContinuationCapture>,
    pub(crate) effect_facts: &'a MaterializedEffectFacts,
}

struct CallBoundaryDispatchMaterialization {
    dispatch: LateLoweredStepDispatchPlan,
    consumed_runtime_error_case: Option<PendingConsumedRuntimeErrorCase>,
}

pub(crate) struct BoundaryMaterialization {
    pub(crate) state_graph: LateLoweredStateGraph,
    pub(crate) boundary_map: LateLoweredBoundaryMap,
}

struct PendingConsumedRuntimeErrorCase {
    input_case_tag: crate::effect_facts::CaseTag,
    input_concrete_op_key: ConcreteOpKey,
    payload_tuple_ty: crate::ty::TypeId,
    terminal_action: LateLoweredLocalRuntimeErrorTerminalAction,
}

struct LocalRuntimeErrorStateTarget {
    boundary_id: BoundaryId,
    owner_state: StateId,
    target_state: StateId,
    payload_tuple_ty: crate::ty::TypeId,
    terminal_action: LateLoweredLocalRuntimeErrorTerminalAction,
}

struct CallBoundaryDispatchInputs<'a> {
    root_fqn: &'a str,
    input_step: &'a LateLoweredStepType,
    output_step: &'a LateLoweredStepType,
    outward_case_tags: &'a [crate::effect_facts::CaseTag],
    continuation_object: ContinuationObjectId,
    target_state: StateId,
    result_local: Option<LocalId>,
    types: &'a TypeStore,
}

pub(crate) fn materialize_step_and_resume_interfaces(
    effect_facts: &MaterializedEffectFacts,
) -> Result<StepMaterialization, EffectLoweringError> {
    let mut step_types = Vec::with_capacity(effect_facts.step_schemas().len());
    let mut resume_interfaces = Vec::new();
    let mut resume_interface_ids_by_step = BTreeMap::new();
    let mut resume_interface_ids_by_group = BTreeMap::new();
    let mut next_interface_raw = 0u32;

    for (&step_schema_id, step_schema) in effect_facts.step_schemas() {
        step_types.push(build_step_type(step_schema_id, step_schema, effect_facts)?);

        let grouped_cases = group_cases_by_effect_family(step_schema);
        let mut interface_ids = Vec::with_capacity(grouped_cases.len());
        for (effect_family, cases) in grouped_cases {
            let interface_id = ResumeInterfaceId::new(next_interface_raw);
            next_interface_raw = next_interface_raw.saturating_add(1);
            resume_interfaces.push(build_resume_interface(
                interface_id,
                effect_family.clone(),
                step_schema_id,
                step_schema,
                &cases,
                effect_facts,
            )?);
            resume_interface_ids_by_group.insert((step_schema_id, effect_family), interface_id);
            interface_ids.push(interface_id);
        }
        resume_interface_ids_by_step.insert(step_schema_id, interface_ids);
    }

    Ok(StepMaterialization {
        step_types,
        resume_interfaces,
        resume_interface_ids_by_step,
        resume_interface_ids_by_group,
    })
}

pub(crate) fn materialize_dynamic_invoke_entry(
    step_schema: StepSchemaId,
    step_type: &LateLoweredStepType,
    entry_state: StateId,
    complete_state: StateId,
) -> LateLoweredDynamicInvokeEntry {
    LateLoweredDynamicInvokeEntry::new(
        step_type.invoke_args_tuple_ty(),
        step_schema,
        entry_state,
        complete_state,
    )
}

pub(crate) fn materialize_continuation_object(
    inputs: ContinuationObjectMaterializationInputs<'_>,
) -> Result<LateLoweredContinuationObject, EffectLoweringError> {
    let ContinuationObjectMaterializationInputs {
        continuation_object_id,
        owner_version_key,
        step_schema_id,
        step_schema,
        implemented_interfaces,
        resume_interface_ids_by_group,
        captures,
        effect_facts,
    } = inputs;
    let surface_resumes = step_schema
        .cases()
        .iter()
        .map(|case| {
            let continuation_contract =
                build_continuation_contract(step_schema_id, step_schema, case, effect_facts)?;
            Result::<_, EffectLoweringError>::Ok(LateLoweredContinuationSurfaceResume::new(
                case.case_tag(),
                case.concrete_op_key().clone(),
                continuation_contract,
                continuation_resume_body(owner_version_key.impl_plan(), case.case_tag()),
            ))
        })
        .collect::<Result<Vec<_>, _>>()?;

    let methods = step_schema
        .cases()
        .iter()
        .map(|case| {
            let interface_id = *resume_interface_ids_by_group
                .get(&(
                    step_schema_id,
                    case.concrete_op_key().effect_family().clone(),
                ))
                .ok_or_else(|| EffectLoweringError::MissingResumeInterfaceFamily {
                    step_schema: step_schema_id.as_u32(),
                    effect_fqn: case
                        .concrete_op_key()
                        .effect_family()
                        .effect_fqn()
                        .to_string(),
                })?;
            let continuation_contract =
                build_continuation_contract(step_schema_id, step_schema, case, effect_facts)?;
            Result::<_, EffectLoweringError>::Ok(LateLoweredContinuationMethod::new(
                interface_id,
                case.case_tag(),
                case.concrete_op_key().clone(),
                continuation_contract,
                continuation_resume_body(owner_version_key.impl_plan(), case.case_tag()),
            ))
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(LateLoweredContinuationObject::new(
        continuation_object_id,
        owner_version_key,
        step_schema.continuation_obj_ty(),
        implemented_interfaces.to_vec(),
        captures,
        surface_resumes,
        methods,
    ))
}

pub(crate) fn materialize_boundary_map(
    inputs: BoundaryMaterializationInputs<'_>,
) -> Result<BoundaryMaterialization, EffectLoweringError> {
    let BoundaryMaterializationInputs {
        root_fqn,
        body,
        body_facts,
        step_type,
        state_graph,
        frame_schema,
        boundary_map,
        continuation_object,
        step_types,
        types,
    } = inputs;

    let result_locals = collect_result_locals(body);
    let (resume_boundaries, runtime_error_boundaries) = paired_resume_boundaries(boundary_map);
    let mut entries = Vec::with_capacity(boundary_map.entries().len());
    let mut local_runtime_error_targets = Vec::new();
    let mut next_state_raw = state_graph
        .states()
        .iter()
        .map(|state| state.state_id().as_u32())
        .max()
        .unwrap_or(0)
        .saturating_add(1);

    for boundary in boundary_map.entries() {
        let lowering = match boundary.source() {
            LateLoweredBoundarySource::Site {
                site_id,
                kind: BoundarySiteKind::Call,
            } => {
                let facts = clone_call_site_facts(root_fqn, body_facts, site_id)?;
                let input_step = lookup_step_type(root_fqn, step_types, facts.callee_schema())?;
                let result_local = *result_locals.call_results.get(&site_id).ok_or_else(|| {
                    EffectLoweringError::MissingBoundaryResultLocal {
                        root_fqn: root_fqn.to_string(),
                        site_id: site_id.as_u32(),
                        kind: "Call",
                    }
                })?;
                let call_dispatch =
                    build_call_boundary_dispatch_plan(CallBoundaryDispatchInputs {
                        root_fqn,
                        input_step,
                        output_step: step_type,
                        outward_case_tags: facts.resolved_cases().tags(),
                        continuation_object,
                        target_state: boundary.resume_state(),
                        result_local: Some(result_local),
                        types,
                    })?;
                let consumed_runtime_error_case =
                    call_dispatch.consumed_runtime_error_case.map(|pending| {
                        let target_state = StateId::new(next_state_raw);
                        next_state_raw = next_state_raw.saturating_add(1);
                        local_runtime_error_targets.push(LocalRuntimeErrorStateTarget {
                            boundary_id: boundary.boundary_id(),
                            owner_state: boundary.owner_state(),
                            target_state,
                            payload_tuple_ty: pending.payload_tuple_ty,
                            terminal_action: pending.terminal_action,
                        });
                        LateLoweredConsumedRuntimeErrorCase::new(
                            pending.input_case_tag,
                            pending.input_concrete_op_key,
                            pending.payload_tuple_ty,
                            pending.terminal_action,
                            target_state,
                        )
                    });
                LateLoweredBoundaryLowering::Call(LateLoweredCallBoundaryLowering::new(
                    facts,
                    result_local,
                    call_dispatch.dispatch,
                    consumed_runtime_error_case,
                ))
            }
            LateLoweredBoundarySource::Site {
                site_id,
                kind: BoundarySiteKind::Perform,
            } => {
                let facts = clone_perform_site_facts(root_fqn, body_facts, site_id)?;
                let emitted_step = build_current_step_emission(
                    root_fqn,
                    step_type,
                    facts.emitted_case(),
                    continuation_object,
                )?;
                LateLoweredBoundaryLowering::Perform(LateLoweredPerformBoundaryLowering::new(
                    facts,
                    emitted_step,
                ))
            }
            LateLoweredBoundarySource::Site {
                site_id,
                kind: BoundarySiteKind::Resume,
            } => {
                let facts = clone_resume_site_facts(root_fqn, body_facts, site_id)?;
                let input_step = lookup_step_type(root_fqn, step_types, facts.out_step_schema())?;
                let result_local = *result_locals.call_results.get(&site_id).ok_or_else(|| {
                    EffectLoweringError::MissingBoundaryResultLocal {
                        root_fqn: root_fqn.to_string(),
                        site_id: site_id.as_u32(),
                        kind: "Resume",
                    }
                })?;
                let runtime_error_boundary =
                    *runtime_error_boundaries.get(&site_id).ok_or_else(|| {
                        EffectLoweringError::MissingPairedRuntimeErrorBoundary {
                            root_fqn: root_fqn.to_string(),
                            site_id: site_id.as_u32(),
                        }
                    })?;
                let dispatch = build_step_dispatch_plan(
                    root_fqn,
                    input_step,
                    step_type,
                    facts.resolved_cases().tags(),
                    continuation_object,
                    boundary.resume_state(),
                    Some(result_local),
                )?;
                LateLoweredBoundaryLowering::Resume(LateLoweredResumeBoundaryLowering::new(
                    facts,
                    result_local,
                    runtime_error_boundary,
                    dispatch,
                ))
            }
            LateLoweredBoundarySource::RuntimeError { origin_site } => {
                let resume_boundary = *resume_boundaries.get(&origin_site).ok_or_else(|| {
                    EffectLoweringError::MissingPairedResumeBoundary {
                        root_fqn: root_fqn.to_string(),
                        site_id: origin_site.as_u32(),
                    }
                })?;
                let resume_runtime_error_effect =
                    resume_runtime_error_effect_family(root_fqn, body, origin_site, types)?;
                let facts = clone_resume_site_facts(root_fqn, body_facts, origin_site)?;
                let input_step = lookup_step_type(root_fqn, step_types, facts.out_step_schema())?;
                let runtime_case = input_step
                    .cases()
                    .iter()
                    .find(|case| {
                        case.concrete_op_key().effect_family() == &resume_runtime_error_effect
                    })
                    .ok_or_else(
                        || EffectLoweringError::MissingRuntimeErrorCaseInResumeStep {
                            root_fqn: root_fqn.to_string(),
                            site_id: origin_site.as_u32(),
                            step_schema: facts.out_step_schema().as_u32(),
                        },
                    )?;
                let emitted_step = build_emission_from_concrete_op(
                    root_fqn,
                    input_step.step_schema(),
                    step_type,
                    runtime_case.concrete_op_key(),
                    continuation_object,
                )?;
                LateLoweredBoundaryLowering::RuntimeError(
                    LateLoweredRuntimeErrorBoundaryLowering::new(
                        origin_site,
                        resume_boundary,
                        emitted_step,
                    ),
                )
            }
            LateLoweredBoundarySource::Site {
                site_id,
                kind: BoundarySiteKind::Handle,
            } => {
                let facts = clone_handle_site_facts(root_fqn, body_facts, site_id)?;
                let outward_emissions = build_handle_outward_emissions(
                    root_fqn,
                    step_type,
                    &facts,
                    continuation_object,
                )?;
                LateLoweredBoundaryLowering::Handle(LateLoweredHandleBoundaryLowering::new(
                    facts,
                    outward_emissions,
                ))
            }
        };

        entries.push(boundary.clone().with_lowering(lowering));
    }

    let boundary_map = LateLoweredBoundaryMap::new(entries);
    let state_graph =
        attach_local_runtime_error_states(root_fqn, state_graph, &local_runtime_error_targets)?;
    let state_graph = attach_handle_dispatch_contracts(
        root_fqn,
        body,
        body_facts,
        &state_graph,
        frame_schema,
        &boundary_map,
        continuation_object,
    )?;

    Ok(BoundaryMaterialization {
        state_graph,
        boundary_map,
    })
}

fn attach_local_runtime_error_states(
    root_fqn: &str,
    state_graph: &LateLoweredStateGraph,
    targets: &[LocalRuntimeErrorStateTarget],
) -> Result<LateLoweredStateGraph, EffectLoweringError> {
    if targets.is_empty() {
        return Ok(state_graph.clone());
    }

    let mut states = state_graph.states().to_vec();
    let mut local_targets_by_owner = BTreeMap::<StateId, Vec<StateId>>::new();
    for target in targets {
        local_targets_by_owner
            .entry(target.owner_state)
            .or_default()
            .push(target.target_state);
        states.push(LateLoweredState::new(
            target.target_state,
            LateLoweredStateRole::Segment,
            Vec::new(),
            LateLoweredStateTerminator::LocalRuntimeError {
                payload_tuple_ty: target.payload_tuple_ty,
                terminal_action: target.terminal_action,
            },
        ));
    }

    let rewritten_states = states
        .into_iter()
        .map(|state| {
            let Some(local_runtime_error_states) = local_targets_by_owner.get(&state.state_id())
            else {
                return Ok(state);
            };
            let terminator = match state.terminator().clone() {
                LateLoweredStateTerminator::Suspend {
                    boundary_ids,
                    resume_state,
                    local_runtime_error_states: existing_local_runtime_error_states,
                    cleanup_state,
                    drop_state,
                } => {
                    let mut merged_local_runtime_error_states = existing_local_runtime_error_states;
                    merged_local_runtime_error_states
                        .extend(local_runtime_error_states.iter().copied());
                    merged_local_runtime_error_states.sort();
                    merged_local_runtime_error_states.dedup();
                    LateLoweredStateTerminator::Suspend {
                        boundary_ids,
                        resume_state,
                        local_runtime_error_states: merged_local_runtime_error_states,
                        cleanup_state,
                        drop_state,
                    }
                }
                _ => {
                    let boundary_id = targets
                        .iter()
                        .find(|target| target.owner_state == state.state_id())
                        .map(|target| target.boundary_id)
                        .expect(
                            "owner state with local runtime-error target should record boundary",
                        );
                    return Err(EffectLoweringError::InvalidLocalRuntimeErrorOwnerState {
                        root_fqn: root_fqn.to_string(),
                        boundary_id: boundary_id.as_u32(),
                        owner_state: state.state_id().as_u32(),
                    });
                }
            };
            Result::<_, EffectLoweringError>::Ok(LateLoweredState::new(
                state.state_id(),
                state.role(),
                state.source_slices().to_vec(),
                terminator,
            ))
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(LateLoweredStateGraph::new(
        state_graph.entry_state(),
        state_graph.complete_state(),
        state_graph.cleanup_state(),
        state_graph.drop_state(),
        rewritten_states,
    ))
}

fn attach_handle_dispatch_contracts(
    root_fqn: &str,
    body: &Body,
    body_facts: &BodyEffectFacts,
    state_graph: &LateLoweredStateGraph,
    frame_schema: &LateLoweredFrameSchema,
    boundary_map: &LateLoweredBoundaryMap,
    continuation_object: ContinuationObjectId,
) -> Result<LateLoweredStateGraph, EffectLoweringError> {
    let rewritten_states = state_graph
        .states()
        .iter()
        .map(|state| {
            let terminator = match state.terminator().clone() {
                LateLoweredStateTerminator::HandleDispatch {
                    site_id,
                    body_state,
                    arm_states,
                    finally_state,
                    exit_state,
                    boundary_ids,
                    drop_state,
                    ..
                } => {
                    let facts = clone_handle_site_facts(root_fqn, body_facts, site_id)?;
                    let contract = build_handle_dispatch_contract(
                        root_fqn,
                        body,
                        state.state_id(),
                        body_state,
                        site_id,
                        &facts,
                        state_graph,
                        &arm_states,
                        finally_state,
                        exit_state,
                        frame_schema,
                        &boundary_ids,
                        drop_state,
                        boundary_map,
                        continuation_object,
                    )?;
                    LateLoweredStateTerminator::HandleDispatch {
                        site_id,
                        body_state,
                        arm_states,
                        finally_state,
                        exit_state,
                        contract,
                        boundary_ids,
                        drop_state,
                    }
                }
                other => other,
            };
            Result::<_, EffectLoweringError>::Ok(LateLoweredState::new(
                state.state_id(),
                state.role(),
                state.source_slices().to_vec(),
                terminator,
            ))
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(LateLoweredStateGraph::new(
        state_graph.entry_state(),
        state_graph.complete_state(),
        state_graph.cleanup_state(),
        state_graph.drop_state(),
        rewritten_states,
    ))
}

#[allow(clippy::too_many_arguments)]
fn build_handle_dispatch_contract(
    root_fqn: &str,
    body: &Body,
    dispatch_state: StateId,
    body_state: StateId,
    site_id: SiteId,
    facts: &HandleSiteEffectFacts,
    state_graph: &LateLoweredStateGraph,
    arm_states: &[StateId],
    finally_state: Option<StateId>,
    exit_state: StateId,
    frame_schema: &LateLoweredFrameSchema,
    boundary_ids: &[BoundaryId],
    drop_state: Option<StateId>,
    boundary_map: &LateLoweredBoundaryMap,
    continuation_object: ContinuationObjectId,
) -> Result<LateLoweredHandleDispatchContract, EffectLoweringError> {
    if arm_states.len() != facts.arm_facts().len() {
        return Err(invalid_handle_dispatch_contract(
            root_fqn,
            site_id,
            format!(
                "arm state 数量({}) 与 HandleSiteEffectFacts.arm_facts 数量({}) 不一致",
                arm_states.len(),
                facts.arm_facts().len(),
            ),
        ));
    }

    let body_complete_target = finally_state.unwrap_or(exit_state);
    let arm_complete_target = finally_state.unwrap_or(exit_state);
    let finally_complete_target = finally_state.map(|_| exit_state);
    let handle_arms = lookup_handle_arms(root_fqn, body, site_id)?;
    if handle_arms.len() != facts.arm_facts().len() {
        return Err(invalid_handle_dispatch_contract(
            root_fqn,
            site_id,
            format!(
                "canonical MIR handle arm 数量({}) 与 HandleSiteEffectFacts.arm_facts 数量({}) 不一致",
                handle_arms.len(),
                facts.arm_facts().len(),
            ),
        ));
    }
    let handled_arms = facts
        .arm_facts()
        .iter()
        .zip(arm_states.iter().copied())
        .zip(handle_arms.iter().enumerate())
        .map(|((arm_facts, arm_state), (arm_ordinal, arm))| {
            let published_payload_tuple_ty = arm.payload_tuple_ty.ok_or_else(|| {
                invalid_handle_dispatch_contract(
                    root_fqn,
                    site_id,
                    format!(
                        "canonical MIR handle arm #{arm_ordinal} 缺少 payload tuple type，无法发布 authoritative binder contract",
                    ),
                )
            })?;
            if published_payload_tuple_ty != arm_facts.payload_tuple_ty() {
                return Err(invalid_handle_dispatch_contract(
                    root_fqn,
                    site_id,
                    format!(
                        "canonical MIR handle arm #{arm_ordinal} 的 payload tuple ty t{} 与 HandleSiteEffectFacts 发布的 t{} 不一致",
                        published_payload_tuple_ty.as_u32(),
                        arm_facts.payload_tuple_ty().as_u32(),
                    ),
                ));
            }
            if arm.binder_count != arm.binder_locals.len() {
                return Err(invalid_handle_dispatch_contract(
                    root_fqn,
                    site_id,
                    format!(
                        "canonical MIR handle arm #{arm_ordinal} 的 binder_count={} 与 binder_locals.len()={} 不一致",
                        arm.binder_count,
                        arm.binder_locals.len(),
                    ),
                ));
            }
            let payload_binders = arm
                .binder_locals
                .iter()
                .copied()
                .enumerate()
                .map(|(ordinal, local)| {
                    LateLoweredHandlePayloadBinder::new(
                        ordinal as u32,
                        local,
                        frame_schema
                            .slot_for_kind(crate::effect_lowered::ir::LateLoweredFrameSlotKind::HandleBinder {
                                site_id,
                                local,
                                ordinal: ordinal as u32,
                            })
                            .map(|slot| slot.slot_id()),
                    )
                })
                .collect::<Vec<_>>();
            let continuation_binder = arm.continuation_local.map(|local| {
                LateLoweredHandleContinuationBinder::new(
                    local,
                    find_frame_slot_for_local(frame_schema, local),
                    arm_facts.continuation_schema(),
                    continuation_object,
                )
            });
            Ok(LateLoweredHandleArmDispatch::new(
                arm_facts.handled_case(),
                arm_state,
                arm_ordinal as u32,
                arm_facts.payload_tuple_ty(),
                payload_binders,
                continuation_binder,
                arm_facts.arm_outward_cases().tags().to_vec(),
            ))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let body_outward_cases = facts.body_outward_cases().tags().to_vec();
    let finally_outward_cases = facts.finally_outward_cases().tags().to_vec();
    let expected_outward_case_tags = collect_handle_outward_case_tags(facts);
    let handle_boundary_lowering =
        find_handle_boundary_lowering(root_fqn, site_id, boundary_ids, boundary_map)?;
    let outward_emissions = handle_boundary_lowering
        .map(|lowering| lowering.outward_emissions().to_vec())
        .unwrap_or_default();
    let published_outward_case_tags = outward_emissions
        .iter()
        .map(|emission| emission.case_tag())
        .collect::<BTreeSet<_>>();
    if handle_boundary_lowering.is_some()
        && published_outward_case_tags != expected_outward_case_tags
    {
        return Err(invalid_handle_dispatch_contract(
            root_fqn,
            site_id,
            format!(
                "published outward emissions {} 与 HandleSiteEffectFacts 期望的 outward cases {} 不一致",
                format_case_tag_set(&published_outward_case_tags),
                format_case_tag_set(&expected_outward_case_tags),
            ),
        ));
    }

    let mut pending_completions = Vec::new();
    if finally_state.is_some() {
        pending_completions.push(LateLoweredHandlePendingCompletion::ContinueToExit);
        pending_completions.push(LateLoweredHandlePendingCompletion::ReturnFromFunction);
        if handle_boundary_lowering.is_some() {
            let mut pending_outward_cases = facts
                .body_outward_cases()
                .tags()
                .iter()
                .copied()
                .collect::<BTreeSet<_>>();
            for arm in facts.arm_facts() {
                pending_outward_cases.extend(arm.arm_outward_cases().tags().iter().copied());
            }
            for case_tag in pending_outward_cases {
                pending_completions.push(LateLoweredHandlePendingCompletion::PropagateOutward(
                    case_tag,
                ));
            }
        }
    }
    let state_regions = build_handle_state_region_entries(
        root_fqn,
        site_id,
        state_graph,
        dispatch_state,
        body_state,
        &handled_arms,
        finally_state,
        exit_state,
    )?;
    let boundary_routings = build_handle_boundary_routings(
        root_fqn,
        site_id,
        &state_regions,
        &handled_arms,
        &body_outward_cases,
        &finally_outward_cases,
        &outward_emissions,
        &pending_completions,
        boundary_map,
    )?;

    Ok(LateLoweredHandleDispatchContract::new(
        LateLoweredHandleDispatchCarrierContract::new(
            crate::effect_lowered::ir::SystemSlotKind::StateTag,
            crate::effect_lowered::ir::SystemSlotKind::CompletionTag,
            crate::effect_lowered::ir::SystemSlotKind::ResumePayloadCarrier,
        ),
        body_complete_target,
        arm_complete_target,
        finally_complete_target,
        handled_arms,
        body_outward_cases,
        finally_outward_cases,
        outward_emissions,
        pending_completions,
        state_regions,
        boundary_routings,
        drop_state,
    ))
}

#[allow(clippy::too_many_arguments)]
fn build_handle_state_region_entries(
    root_fqn: &str,
    site_id: SiteId,
    state_graph: &LateLoweredStateGraph,
    dispatch_state: StateId,
    body_state: StateId,
    handled_arms: &[LateLoweredHandleArmDispatch],
    finally_state: Option<StateId>,
    exit_state: StateId,
) -> Result<Vec<LateLoweredHandleStateRegionEntry>, EffectLoweringError> {
    let mut memberships = BTreeMap::<StateId, LateLoweredHandleStateRegion>::new();
    insert_handle_state_region(
        root_fqn,
        site_id,
        &mut memberships,
        dispatch_state,
        LateLoweredHandleStateRegion::Dispatch,
    )?;
    insert_handle_state_region(
        root_fqn,
        site_id,
        &mut memberships,
        exit_state,
        LateLoweredHandleStateRegion::Exit,
    )?;

    let mut stop_states = BTreeSet::from([dispatch_state, exit_state]);
    stop_states.extend(
        handled_arms
            .iter()
            .map(LateLoweredHandleArmDispatch::arm_state),
    );
    if let Some(finally_state) = finally_state {
        stop_states.insert(finally_state);
    }

    for state_id in
        collect_handle_region_states(root_fqn, site_id, state_graph, body_state, &stop_states)?
    {
        insert_handle_state_region(
            root_fqn,
            site_id,
            &mut memberships,
            state_id,
            LateLoweredHandleStateRegion::Body,
        )?;
    }

    for arm in handled_arms {
        let mut arm_stops = stop_states.clone();
        arm_stops.remove(&arm.arm_state());
        let region = LateLoweredHandleStateRegion::Arm {
            handled_case: arm.handled_case(),
            arm_ordinal: arm.arm_ordinal(),
        };
        for state_id in collect_handle_region_states(
            root_fqn,
            site_id,
            state_graph,
            arm.arm_state(),
            &arm_stops,
        )? {
            insert_handle_state_region(root_fqn, site_id, &mut memberships, state_id, region)?;
        }
    }

    if let Some(finally_state) = finally_state {
        let mut finally_stops = stop_states;
        finally_stops.remove(&finally_state);
        for state_id in collect_handle_region_states(
            root_fqn,
            site_id,
            state_graph,
            finally_state,
            &finally_stops,
        )? {
            insert_handle_state_region(
                root_fqn,
                site_id,
                &mut memberships,
                state_id,
                LateLoweredHandleStateRegion::Finally,
            )?;
        }
    }

    Ok(memberships
        .into_iter()
        .map(|(state_id, region)| LateLoweredHandleStateRegionEntry::new(state_id, region))
        .collect())
}

#[allow(clippy::too_many_arguments)]
fn build_handle_boundary_routings(
    root_fqn: &str,
    site_id: SiteId,
    state_regions: &[LateLoweredHandleStateRegionEntry],
    handled_arms: &[LateLoweredHandleArmDispatch],
    body_outward_cases: &[CaseTag],
    finally_outward_cases: &[CaseTag],
    outward_emissions: &[LateLoweredStepCaseEmission],
    pending_completions: &[LateLoweredHandlePendingCompletion],
    boundary_map: &LateLoweredBoundaryMap,
) -> Result<Vec<LateLoweredHandleBoundaryRouting>, EffectLoweringError> {
    let mut regions_by_state = BTreeMap::new();
    for entry in state_regions {
        if regions_by_state
            .insert(entry.state_id(), entry.region())
            .is_some()
        {
            return Err(invalid_handle_dispatch_contract(
                root_fqn,
                site_id,
                format!(
                    "state st{} 在 region contract 中重复发布",
                    entry.state_id().as_u32()
                ),
            ));
        }
    }
    let handled_arms_by_case = handled_arms
        .iter()
        .map(|arm| (arm.handled_case(), arm))
        .collect::<BTreeMap<_, _>>();
    let body_outward_cases = body_outward_cases.iter().copied().collect::<BTreeSet<_>>();
    let finally_outward_cases = finally_outward_cases
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let outward_emission_cases = outward_emissions
        .iter()
        .map(LateLoweredStepCaseEmission::case_tag)
        .collect::<BTreeSet<_>>();
    let pending_outward_cases = pending_completions
        .iter()
        .filter_map(|pending| match pending {
            LateLoweredHandlePendingCompletion::PropagateOutward(case_tag) => {
                Some((*case_tag, *pending))
            }
            LateLoweredHandlePendingCompletion::ContinueToExit
            | LateLoweredHandlePendingCompletion::ReturnFromFunction => None,
        })
        .collect::<BTreeMap<_, _>>();
    let mut routes = Vec::new();

    for boundary in boundary_map.entries() {
        let owner_region = regions_by_state
            .get(&boundary.owner_state())
            .copied()
            .unwrap_or(LateLoweredHandleStateRegion::OutsideHandle);
        if matches!(
            owner_region,
            LateLoweredHandleStateRegion::OutsideHandle | LateLoweredHandleStateRegion::Exit
        ) {
            continue;
        }
        if matches!(owner_region, LateLoweredHandleStateRegion::Dispatch)
            && !matches!(
                boundary.source(),
                LateLoweredBoundarySource::Site {
                    site_id: boundary_site,
                    kind: BoundarySiteKind::Handle,
                } if boundary_site == site_id
            )
        {
            return Err(invalid_handle_dispatch_contract(
                root_fqn,
                site_id,
                format!(
                    "dispatch state st{} 上的 boundary bd{} 不是当前 handle site 的 published Handle boundary：source={:?}",
                    boundary.owner_state().as_u32(),
                    boundary.boundary_id().as_u32(),
                    boundary.source(),
                ),
            ));
        }

        let case_tags = collect_handle_boundary_case_tags(root_fqn, site_id, boundary)?;
        let case_routings = case_tags
            .into_iter()
            .map(|case_tag| {
                route_handle_boundary_case(
                    root_fqn,
                    site_id,
                    boundary,
                    owner_region,
                    case_tag,
                    &handled_arms_by_case,
                    &body_outward_cases,
                    &finally_outward_cases,
                    &outward_emission_cases,
                    &pending_outward_cases,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        routes.push(LateLoweredHandleBoundaryRouting::new(
            boundary.boundary_id(),
            boundary.owner_state(),
            owner_region,
            boundary.resume_state(),
            case_routings,
        ));
    }

    Ok(routes)
}

fn collect_handle_region_states(
    root_fqn: &str,
    site_id: SiteId,
    state_graph: &LateLoweredStateGraph,
    entry_state: StateId,
    stop_states: &BTreeSet<StateId>,
) -> Result<BTreeSet<StateId>, EffectLoweringError> {
    if state_graph.state(entry_state).is_none() {
        return Err(invalid_handle_dispatch_contract(
            root_fqn,
            site_id,
            format!(
                "HandleDispatch region root st{} 不存在于 state graph 中",
                entry_state.as_u32()
            ),
        ));
    }

    let mut visited = BTreeSet::new();
    let mut worklist = vec![entry_state];
    while let Some(state_id) = worklist.pop() {
        if stop_states.contains(&state_id) || !visited.insert(state_id) {
            continue;
        }
        let state = state_graph.state(state_id).ok_or_else(|| {
            invalid_handle_dispatch_contract(
                root_fqn,
                site_id,
                format!(
                    "HandleDispatch region 遍历命中了不存在的 state st{}",
                    state_id.as_u32()
                ),
            )
        })?;
        worklist.extend(state.successors().iter().rev().copied());
    }

    Ok(visited)
}

fn insert_handle_state_region(
    root_fqn: &str,
    site_id: SiteId,
    memberships: &mut BTreeMap<StateId, LateLoweredHandleStateRegion>,
    state_id: StateId,
    region: LateLoweredHandleStateRegion,
) -> Result<(), EffectLoweringError> {
    match memberships.insert(state_id, region) {
        Some(existing) if existing != region => Err(invalid_handle_dispatch_contract(
            root_fqn,
            site_id,
            format!(
                "state st{} 在 HandleDispatch region contract 中同时归属于 {:?} 和 {:?}",
                state_id.as_u32(),
                existing,
                region,
            ),
        )),
        Some(_) | None => Ok(()),
    }
}

fn collect_handle_boundary_case_tags(
    root_fqn: &str,
    site_id: SiteId,
    boundary: &crate::effect_lowered::ir::LateLoweredBoundary,
) -> Result<Vec<CaseTag>, EffectLoweringError> {
    let mut tags = BTreeSet::new();
    let lowering = boundary.lowering().ok_or_else(|| {
        invalid_handle_dispatch_contract(
            root_fqn,
            site_id,
            format!(
                "boundary bd{} 缺少 lowering，无法发布 handle boundary routing contract",
                boundary.boundary_id().as_u32()
            ),
        )
    })?;
    let case_iter: Vec<CaseTag> = match lowering {
        LateLoweredBoundaryLowering::Call(lowering) => lowering
            .dispatch()
            .outward_cases()
            .iter()
            .map(|forwarding| forwarding.emission().case_tag())
            .collect(),
        LateLoweredBoundaryLowering::Perform(lowering) => vec![lowering.emitted_step().case_tag()],
        LateLoweredBoundaryLowering::Resume(lowering) => lowering
            .dispatch()
            .outward_cases()
            .iter()
            .map(|forwarding| forwarding.emission().case_tag())
            .collect(),
        LateLoweredBoundaryLowering::RuntimeError(lowering) => {
            vec![lowering.emitted_step().case_tag()]
        }
        LateLoweredBoundaryLowering::Handle(lowering) => lowering
            .outward_emissions()
            .iter()
            .map(LateLoweredStepCaseEmission::case_tag)
            .collect(),
    };
    for case_tag in case_iter {
        if !tags.insert(case_tag) {
            return Err(invalid_handle_dispatch_contract(
                root_fqn,
                site_id,
                format!(
                    "boundary bd{} 重复发布 outward case c{}，无法生成稳定 routing contract",
                    boundary.boundary_id().as_u32(),
                    case_tag.as_u32(),
                ),
            ));
        }
    }
    Ok(tags.into_iter().collect())
}

#[allow(clippy::too_many_arguments)]
fn route_handle_boundary_case(
    root_fqn: &str,
    site_id: SiteId,
    boundary: &crate::effect_lowered::ir::LateLoweredBoundary,
    owner_region: LateLoweredHandleStateRegion,
    case_tag: CaseTag,
    handled_arms_by_case: &BTreeMap<CaseTag, &LateLoweredHandleArmDispatch>,
    body_outward_cases: &BTreeSet<CaseTag>,
    finally_outward_cases: &BTreeSet<CaseTag>,
    outward_emission_cases: &BTreeSet<CaseTag>,
    pending_outward_cases: &BTreeMap<CaseTag, LateLoweredHandlePendingCompletion>,
) -> Result<LateLoweredHandleBoundaryCaseRouting, EffectLoweringError> {
    let action = match owner_region {
        LateLoweredHandleStateRegion::Body => {
            if let Some(arm) = handled_arms_by_case.get(&case_tag) {
                LateLoweredHandleBoundaryCaseRoutingAction::ConsumeToArm {
                    arm_state: arm.arm_state(),
                    arm_ordinal: arm.arm_ordinal(),
                    continuation_resume_state: boundary.resume_state(),
                }
            } else if body_outward_cases.contains(&case_tag) {
                pending_outward_cases.get(&case_tag).copied().map_or(
                    LateLoweredHandleBoundaryCaseRoutingAction::EmitOutward,
                    |completion| LateLoweredHandleBoundaryCaseRoutingAction::PendingCompletion {
                        completion,
                    },
                )
            } else if finally_outward_cases.contains(&case_tag) {
                LateLoweredHandleBoundaryCaseRoutingAction::EmitOutward
            } else {
                return Err(invalid_handle_dispatch_contract(
                    root_fqn,
                    site_id,
                    format!(
                        "body region 的 boundary bd{} 发布了未声明的 outward case c{}",
                        boundary.boundary_id().as_u32(),
                        case_tag.as_u32(),
                    ),
                ));
            }
        }
        LateLoweredHandleStateRegion::Arm {
            handled_case,
            arm_ordinal,
        } => {
            let arm = handled_arms_by_case.get(&handled_case).ok_or_else(|| {
                invalid_handle_dispatch_contract(
                    root_fqn,
                    site_id,
                    format!(
                        "arm region ordinal {} handled case c{} 缺少 published handled-arm contract",
                        arm_ordinal,
                        handled_case.as_u32(),
                    ),
                )
            })?;
            if arm.arm_ordinal() != arm_ordinal {
                return Err(invalid_handle_dispatch_contract(
                    root_fqn,
                    site_id,
                    format!(
                        "arm region ordinal {} 与 handled case c{} 的 published arm ordinal {} 不一致",
                        arm_ordinal,
                        handled_case.as_u32(),
                        arm.arm_ordinal(),
                    ),
                ));
            }
            if !arm.arm_outward_cases().contains(&case_tag) {
                return Err(invalid_handle_dispatch_contract(
                    root_fqn,
                    site_id,
                    format!(
                        "arm region(c{}, ordinal={}) 的 boundary bd{} 发布了未声明的 outward case c{}",
                        handled_case.as_u32(),
                        arm_ordinal,
                        boundary.boundary_id().as_u32(),
                        case_tag.as_u32(),
                    ),
                ));
            }
            pending_outward_cases.get(&case_tag).copied().map_or(
                LateLoweredHandleBoundaryCaseRoutingAction::EmitOutward,
                |completion| LateLoweredHandleBoundaryCaseRoutingAction::PendingCompletion {
                    completion,
                },
            )
        }
        LateLoweredHandleStateRegion::Finally => {
            if !finally_outward_cases.contains(&case_tag) {
                return Err(invalid_handle_dispatch_contract(
                    root_fqn,
                    site_id,
                    format!(
                        "finally region 的 boundary bd{} 发布了未声明的 outward case c{}",
                        boundary.boundary_id().as_u32(),
                        case_tag.as_u32(),
                    ),
                ));
            }
            LateLoweredHandleBoundaryCaseRoutingAction::EmitOutward
        }
        LateLoweredHandleStateRegion::Dispatch => {
            if !outward_emission_cases.contains(&case_tag) {
                return Err(invalid_handle_dispatch_contract(
                    root_fqn,
                    site_id,
                    format!(
                        "dispatch region 的 handle boundary bd{} 发布了未声明的 outward emission case c{}",
                        boundary.boundary_id().as_u32(),
                        case_tag.as_u32(),
                    ),
                ));
            }
            LateLoweredHandleBoundaryCaseRoutingAction::EmitOutward
        }
        LateLoweredHandleStateRegion::Exit | LateLoweredHandleStateRegion::OutsideHandle => {
            return Err(invalid_handle_dispatch_contract(
                root_fqn,
                site_id,
                format!(
                    "boundary bd{} 的 owner state st{} 不在当前 HandleDispatch published region 内，却尝试生成 routing",
                    boundary.boundary_id().as_u32(),
                    boundary.owner_state().as_u32(),
                ),
            ));
        }
    };

    Ok(LateLoweredHandleBoundaryCaseRouting::new(case_tag, action))
}

fn lookup_handle_arms<'a>(
    root_fqn: &str,
    body: &'a Body,
    site_id: SiteId,
) -> Result<&'a [crate::mir::HandlerArm], EffectLoweringError> {
    let mut found = None;
    for block in &body.blocks {
        let crate::mir::TerminatorKind::Handle {
            site_id: handle_site,
            arms,
            ..
        } = &block.terminator.kind
        else {
            continue;
        };
        if *handle_site != site_id {
            continue;
        }
        if found.replace(arms.as_slice()).is_some() {
            return Err(invalid_handle_dispatch_contract(
                root_fqn,
                site_id,
                "canonical MIR 中同一 handle site 重复发布多个 Handle terminator".to_string(),
            ));
        }
    }
    found.ok_or_else(|| {
        invalid_handle_dispatch_contract(
            root_fqn,
            site_id,
            "缺少对应的 canonical MIR Handle terminator，无法发布 arm binder contract".to_string(),
        )
    })
}

fn find_frame_slot_for_local(
    frame_schema: &LateLoweredFrameSchema,
    local: LocalId,
) -> Option<crate::effect_lowered::ir::FrameSlotId> {
    frame_schema.slots().iter().find_map(|slot| {
        let slot_local = match slot.kind() {
            crate::effect_lowered::ir::LateLoweredFrameSlotKind::SourceLocal(slot_local)
            | crate::effect_lowered::ir::LateLoweredFrameSlotKind::CompilerTemporary(slot_local)
            | crate::effect_lowered::ir::LateLoweredFrameSlotKind::HandleBinder {
                local: slot_local,
                ..
            }
            | crate::effect_lowered::ir::LateLoweredFrameSlotKind::BoundaryResult {
                local: slot_local,
                ..
            }
            | crate::effect_lowered::ir::LateLoweredFrameSlotKind::JoinValue {
                local: slot_local,
                ..
            } => Some(slot_local),
            crate::effect_lowered::ir::LateLoweredFrameSlotKind::ResumePayload { .. }
            | crate::effect_lowered::ir::LateLoweredFrameSlotKind::System(_) => None,
        };
        (slot_local == Some(local)).then_some(slot.slot_id())
    })
}

fn find_handle_boundary_lowering<'a>(
    root_fqn: &str,
    site_id: SiteId,
    boundary_ids: &[BoundaryId],
    boundary_map: &'a LateLoweredBoundaryMap,
) -> Result<Option<&'a LateLoweredHandleBoundaryLowering>, EffectLoweringError> {
    let mut lowering = None;
    for boundary_id in boundary_ids {
        let Some(boundary) = boundary_map.boundary(*boundary_id) else {
            return Err(invalid_handle_dispatch_contract(
                root_fqn,
                site_id,
                format!("boundary_ids 中引用了不存在的 bd{}", boundary_id.as_u32()),
            ));
        };
        let (boundary_site, handle_lowering) = match (boundary.source(), boundary.lowering()) {
            (
                LateLoweredBoundarySource::Site {
                    site_id: boundary_site,
                    kind: BoundarySiteKind::Handle,
                },
                Some(LateLoweredBoundaryLowering::Handle(lowering)),
            ) => (boundary_site, lowering),
            (source, lowering) => {
                return Err(invalid_handle_dispatch_contract(
                    root_fqn,
                    site_id,
                    format!(
                        "boundary bd{} 不是当前 handle site 的 published Handle lowering：source={source:?} lowering={lowering:?}",
                        boundary_id.as_u32(),
                    ),
                ));
            }
        };
        if boundary_site != site_id {
            return Err(invalid_handle_dispatch_contract(
                root_fqn,
                site_id,
                format!(
                    "boundary bd{} 属于 site{}，但当前 HandleDispatch 属于 site{}",
                    boundary_id.as_u32(),
                    boundary_site.as_u32(),
                    site_id.as_u32(),
                ),
            ));
        }
        if lowering.replace(handle_lowering).is_some() {
            return Err(invalid_handle_dispatch_contract(
                root_fqn,
                site_id,
                "同一 HandleDispatch 绑定了多个 handle boundary lowering".to_string(),
            ));
        }
    }
    Ok(lowering)
}

fn collect_handle_outward_case_tags(facts: &HandleSiteEffectFacts) -> BTreeSet<CaseTag> {
    let mut tags = facts
        .body_outward_cases()
        .tags()
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    for arm in facts.arm_facts() {
        tags.extend(arm.arm_outward_cases().tags().iter().copied());
    }
    tags.extend(facts.finally_outward_cases().tags().iter().copied());
    tags
}

fn invalid_handle_dispatch_contract(
    root_fqn: &str,
    site_id: SiteId,
    detail: String,
) -> EffectLoweringError {
    EffectLoweringError::InvalidHandleDispatchContract {
        root_fqn: root_fqn.to_string(),
        site_id: site_id.as_u32(),
        detail,
    }
}

fn format_case_tag_set(tags: &BTreeSet<CaseTag>) -> String {
    if tags.is_empty() {
        return "[]".to_string();
    }
    format!(
        "[{}]",
        tags.iter()
            .map(|case_tag| format!("c{}", case_tag.as_u32()))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn build_step_type(
    step_schema_id: StepSchemaId,
    step_schema: &StepSchema,
    effect_facts: &MaterializedEffectFacts,
) -> Result<LateLoweredStepType, EffectLoweringError> {
    Ok(LateLoweredStepType::new(
        step_schema_id,
        step_schema.invoke_args_tuple_ty(),
        step_schema.complete_ty(),
        step_schema.continuation_obj_ty(),
        step_schema
            .cases()
            .iter()
            .map(|case| {
                let continuation_contract =
                    build_continuation_contract(step_schema_id, step_schema, case, effect_facts)?;
                Result::<_, EffectLoweringError>::Ok(LateLoweredStepCase::new(
                    case.case_tag(),
                    case.concrete_op_key().clone(),
                    case.payload_tuple_ty(),
                    continuation_contract,
                ))
            })
            .collect::<Result<Vec<_>, _>>()?,
    ))
}

fn build_resume_interface(
    interface_id: ResumeInterfaceId,
    effect_family: EffectFamilyKey,
    step_schema_id: StepSchemaId,
    step_schema: &StepSchema,
    cases: &[&StepCaseFact],
    effect_facts: &MaterializedEffectFacts,
) -> Result<LateLoweredResumeInterface, EffectLoweringError> {
    let mut methods = Vec::with_capacity(cases.len());
    let mut return_step_schema = None;
    for case in cases {
        let continuation_contract =
            build_continuation_contract(step_schema_id, step_schema, case, effect_facts)?;
        return_step_schema.get_or_insert(continuation_contract.out_step_schema());
        methods.push(LateLoweredResumeMethod::new(
            case.case_tag(),
            case.concrete_op_key().clone(),
            continuation_contract,
        ));
    }
    Ok(LateLoweredResumeInterface::new(
        interface_id,
        effect_family,
        return_step_schema.unwrap_or(step_schema_id),
        methods,
    ))
}

fn build_continuation_contract(
    step_schema_id: StepSchemaId,
    step_schema: &StepSchema,
    case: &StepCaseFact,
    effect_facts: &MaterializedEffectFacts,
) -> Result<LateLoweredContinuationContract, EffectLoweringError> {
    let continuation_schema = effect_facts
        .continuation_schemas()
        .get(&case.continuation_schema())
        .ok_or_else(|| EffectLoweringError::MissingContinuationSchema {
            step_schema: step_schema_id.as_u32(),
            continuation_schema: case.continuation_schema().as_u32(),
            case_tag: case.case_tag().as_u32(),
        })?;

    if continuation_schema.out_step_schema() != step_schema_id {
        return Err(EffectLoweringError::ContinuationOutStepSchemaMismatch {
            step_schema: step_schema_id.as_u32(),
            continuation_schema: case.continuation_schema().as_u32(),
            case_tag: case.case_tag().as_u32(),
            out_step_schema: continuation_schema.out_step_schema().as_u32(),
        });
    }

    if continuation_schema.answer_ty() != step_schema.complete_ty() {
        return Err(EffectLoweringError::ContinuationAnswerTyMismatch {
            step_schema: step_schema_id.as_u32(),
            continuation_schema: case.continuation_schema().as_u32(),
            case_tag: case.case_tag().as_u32(),
            answer_ty: continuation_schema.answer_ty().as_u32(),
            complete_ty: step_schema.complete_ty().as_u32(),
        });
    }

    Ok(LateLoweredContinuationContract::new(
        case.continuation_schema(),
        continuation_schema.resume_tuple_ty(),
        continuation_schema.answer_ty(),
        continuation_schema.out_step_schema(),
        continuation_schema.surface_ty(),
    ))
}

fn group_cases_by_effect_family(
    step_schema: &StepSchema,
) -> BTreeMap<EffectFamilyKey, Vec<&StepCaseFact>> {
    let mut grouped = BTreeMap::<EffectFamilyKey, Vec<&StepCaseFact>>::new();
    for case in step_schema.cases() {
        grouped
            .entry(case.concrete_op_key().effect_family().clone())
            .or_default()
            .push(case);
    }
    grouped
}

fn continuation_resume_body(
    impl_plan: ImplPlan,
    case_tag: crate::effect_facts::CaseTag,
) -> LateLoweredContinuationResumeBody {
    match impl_plan {
        ImplPlan::NoOutward => LateLoweredContinuationResumeBody::Unreachable,
        ImplPlan::SingleCase(selected) if selected == case_tag => {
            LateLoweredContinuationResumeBody::ResumeCapturedState {
                repeated_resume: LateLoweredOneShotPolicy::OrdinaryRuntimeErrorOutward,
            }
        }
        ImplPlan::SingleCase(_) => LateLoweredContinuationResumeBody::Unreachable,
        ImplPlan::CanonicalFull => LateLoweredContinuationResumeBody::ResumeCapturedState {
            repeated_resume: LateLoweredOneShotPolicy::OrdinaryRuntimeErrorOutward,
        },
    }
}

#[derive(Default)]
struct BoundaryResultLocals {
    call_results: HashMap<SiteId, LocalId>,
}

fn collect_result_locals(body: &Body) -> BoundaryResultLocals {
    let mut call_results = HashMap::new();
    for block in &body.blocks {
        for stmt in &block.stmts {
            let StatementKind::Assign {
                target,
                value: Rvalue::Call { site_id, .. },
            } = &stmt.kind
            else {
                continue;
            };
            call_results.insert(*site_id, *target);
        }
    }
    BoundaryResultLocals { call_results }
}

fn paired_resume_boundaries(
    boundary_map: &LateLoweredBoundaryMap,
) -> (HashMap<SiteId, BoundaryId>, HashMap<SiteId, BoundaryId>) {
    let mut resume_boundaries = HashMap::new();
    let mut runtime_error_boundaries = HashMap::new();
    for boundary in boundary_map.entries() {
        match boundary.source() {
            LateLoweredBoundarySource::Site {
                site_id,
                kind: BoundarySiteKind::Resume,
            } => {
                resume_boundaries.insert(site_id, boundary.boundary_id());
            }
            LateLoweredBoundarySource::RuntimeError { origin_site } => {
                runtime_error_boundaries.insert(origin_site, boundary.boundary_id());
            }
            LateLoweredBoundarySource::Site { .. } => {}
        }
    }
    (resume_boundaries, runtime_error_boundaries)
}

fn clone_call_site_facts(
    root_fqn: &str,
    body_facts: &BodyEffectFacts,
    site_id: SiteId,
) -> Result<CallSiteEffectFacts, EffectLoweringError> {
    let site = body_facts
        .site(site_id)
        .ok_or_else(|| EffectLoweringError::MissingSiteFacts {
            root_fqn: root_fqn.to_string(),
            site_id: site_id.as_u32(),
        })?;
    match site {
        SiteEffectFacts::Call(facts) => Ok(facts.clone()),
        other => Err(EffectLoweringError::UnexpectedSiteFactsKind {
            root_fqn: root_fqn.to_string(),
            site_id: site_id.as_u32(),
            expected: "Call",
            actual: site_facts_kind(other),
        }),
    }
}

fn clone_perform_site_facts(
    root_fqn: &str,
    body_facts: &BodyEffectFacts,
    site_id: SiteId,
) -> Result<PerformSiteEffectFacts, EffectLoweringError> {
    let site = body_facts
        .site(site_id)
        .ok_or_else(|| EffectLoweringError::MissingSiteFacts {
            root_fqn: root_fqn.to_string(),
            site_id: site_id.as_u32(),
        })?;
    match site {
        SiteEffectFacts::Perform(facts) => Ok(facts.clone()),
        other => Err(EffectLoweringError::UnexpectedSiteFactsKind {
            root_fqn: root_fqn.to_string(),
            site_id: site_id.as_u32(),
            expected: "Perform",
            actual: site_facts_kind(other),
        }),
    }
}

fn clone_resume_site_facts(
    root_fqn: &str,
    body_facts: &BodyEffectFacts,
    site_id: SiteId,
) -> Result<ResumeSiteEffectFacts, EffectLoweringError> {
    let site = body_facts
        .site(site_id)
        .ok_or_else(|| EffectLoweringError::MissingSiteFacts {
            root_fqn: root_fqn.to_string(),
            site_id: site_id.as_u32(),
        })?;
    match site {
        SiteEffectFacts::Resume(facts) => Ok(facts.clone()),
        other => Err(EffectLoweringError::UnexpectedSiteFactsKind {
            root_fqn: root_fqn.to_string(),
            site_id: site_id.as_u32(),
            expected: "Resume",
            actual: site_facts_kind(other),
        }),
    }
}

fn clone_handle_site_facts(
    root_fqn: &str,
    body_facts: &BodyEffectFacts,
    site_id: SiteId,
) -> Result<HandleSiteEffectFacts, EffectLoweringError> {
    let site = body_facts
        .site(site_id)
        .ok_or_else(|| EffectLoweringError::MissingSiteFacts {
            root_fqn: root_fqn.to_string(),
            site_id: site_id.as_u32(),
        })?;
    match site {
        SiteEffectFacts::Handle(facts) => Ok(facts.clone()),
        other => Err(EffectLoweringError::UnexpectedSiteFactsKind {
            root_fqn: root_fqn.to_string(),
            site_id: site_id.as_u32(),
            expected: "Handle",
            actual: site_facts_kind(other),
        }),
    }
}

fn site_facts_kind(site: &SiteEffectFacts) -> &'static str {
    match site {
        SiteEffectFacts::Call(_) => "Call",
        SiteEffectFacts::Perform(_) => "Perform",
        SiteEffectFacts::Resume(_) => "Resume",
        SiteEffectFacts::Handle(_) => "Handle",
    }
}

fn lookup_step_type<'a>(
    root_fqn: &str,
    step_types: &'a [LateLoweredStepType],
    step_schema: StepSchemaId,
) -> Result<&'a LateLoweredStepType, EffectLoweringError> {
    step_types
        .iter()
        .find(|step_type| step_type.step_schema() == step_schema)
        .ok_or_else(|| EffectLoweringError::MissingStepSchema {
            root_fqn: root_fqn.to_string(),
            step_schema: step_schema.as_u32(),
        })
}

fn build_step_dispatch_plan(
    root_fqn: &str,
    input_step: &LateLoweredStepType,
    output_step: &LateLoweredStepType,
    outward_case_tags: &[crate::effect_facts::CaseTag],
    continuation_object: ContinuationObjectId,
    target_state: StateId,
    result_local: Option<LocalId>,
) -> Result<LateLoweredStepDispatchPlan, EffectLoweringError> {
    let complete =
        LateLoweredCompleteStepDispatch::new(input_step.complete_ty(), target_state, result_local);
    let outward_cases = outward_case_tags
        .iter()
        .map(|case_tag| {
            let input_case = input_step.case(*case_tag).ok_or_else(|| {
                EffectLoweringError::MissingInputStepCase {
                    root_fqn: root_fqn.to_string(),
                    step_schema: input_step.step_schema().as_u32(),
                    case_tag: case_tag.as_u32(),
                }
            })?;
            let emission = build_emission_from_concrete_op(
                root_fqn,
                input_step.step_schema(),
                output_step,
                input_case.concrete_op_key(),
                continuation_object,
            )?;
            Result::<_, EffectLoweringError>::Ok(LateLoweredStepCaseForwarding::new(
                input_case.case_tag(),
                input_case.concrete_op_key().clone(),
                emission,
            ))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(LateLoweredStepDispatchPlan::new(
        input_step.step_schema(),
        complete,
        outward_cases,
    ))
}

fn build_call_boundary_dispatch_plan(
    inputs: CallBoundaryDispatchInputs<'_>,
) -> Result<CallBoundaryDispatchMaterialization, EffectLoweringError> {
    let CallBoundaryDispatchInputs {
        root_fqn,
        input_step,
        output_step,
        outward_case_tags,
        continuation_object,
        target_state,
        result_local,
        types,
    } = inputs;
    let complete =
        LateLoweredCompleteStepDispatch::new(input_step.complete_ty(), target_state, result_local);
    let mut outward_cases = Vec::with_capacity(outward_case_tags.len());
    let mut consumed_runtime_error_case = None;

    for case_tag in outward_case_tags {
        let input_case = input_step.case(*case_tag).ok_or_else(|| {
            EffectLoweringError::MissingInputStepCase {
                root_fqn: root_fqn.to_string(),
                step_schema: input_step.step_schema().as_u32(),
                case_tag: case_tag.as_u32(),
            }
        })?;
        let projected_case = output_step
            .cases()
            .iter()
            .find(|case| case.concrete_op_key() == input_case.concrete_op_key());
        if let Some(projected_case) = projected_case {
            outward_cases.push(LateLoweredStepCaseForwarding::new(
                input_case.case_tag(),
                input_case.concrete_op_key().clone(),
                LateLoweredStepCaseEmission::new(
                    projected_case.case_tag(),
                    projected_case.concrete_op_key().clone(),
                    projected_case.payload_tuple_ty(),
                    projected_case.continuation_contract(),
                    continuation_object,
                ),
            ));
            continue;
        }

        // Pure caller 仍需保留 call boundary，但 compiler-generated RuntimeError case
        // 由 boundary 本地消费，不应被强行投影回 caller outward StepSchema。
        if is_runtime_error_raise_case(input_case, types) {
            consumed_runtime_error_case.get_or_insert_with(|| PendingConsumedRuntimeErrorCase {
                input_case_tag: input_case.case_tag(),
                input_concrete_op_key: input_case.concrete_op_key().clone(),
                payload_tuple_ty: input_case.payload_tuple_ty(),
                terminal_action: local_runtime_error_terminal_action(),
            });
            continue;
        }

        return Err(EffectLoweringError::MissingProjectedStepCase {
            root_fqn: root_fqn.to_string(),
            input_step_schema: input_step.step_schema().as_u32(),
            output_step_schema: output_step.step_schema().as_u32(),
            concrete_op: input_case
                .concrete_op_key()
                .instance_key()
                .template
                .fqn
                .clone(),
        });
    }

    Ok(CallBoundaryDispatchMaterialization {
        dispatch: LateLoweredStepDispatchPlan::new(
            input_step.step_schema(),
            complete,
            outward_cases,
        ),
        consumed_runtime_error_case,
    })
}

fn local_runtime_error_terminal_action() -> LateLoweredLocalRuntimeErrorTerminalAction {
    LateLoweredLocalRuntimeErrorTerminalAction::RuntimeFatal {
        runtime_entry: LateLoweredPublishedRuntimeEntry::RuntimeErrorFatal,
    }
}

fn build_current_step_emission(
    root_fqn: &str,
    step_type: &LateLoweredStepType,
    case_tag: crate::effect_facts::CaseTag,
    continuation_object: ContinuationObjectId,
) -> Result<LateLoweredStepCaseEmission, EffectLoweringError> {
    let case =
        step_type
            .case(case_tag)
            .ok_or_else(|| EffectLoweringError::MissingInputStepCase {
                root_fqn: root_fqn.to_string(),
                step_schema: step_type.step_schema().as_u32(),
                case_tag: case_tag.as_u32(),
            })?;
    Ok(LateLoweredStepCaseEmission::new(
        case.case_tag(),
        case.concrete_op_key().clone(),
        case.payload_tuple_ty(),
        case.continuation_contract(),
        continuation_object,
    ))
}

fn build_emission_from_concrete_op(
    root_fqn: &str,
    input_step_schema: StepSchemaId,
    output_step: &LateLoweredStepType,
    concrete_op_key: &ConcreteOpKey,
    continuation_object: ContinuationObjectId,
) -> Result<LateLoweredStepCaseEmission, EffectLoweringError> {
    let case = output_step
        .cases()
        .iter()
        .find(|case| case.concrete_op_key() == concrete_op_key)
        .ok_or_else(|| EffectLoweringError::MissingProjectedStepCase {
            root_fqn: root_fqn.to_string(),
            input_step_schema: input_step_schema.as_u32(),
            output_step_schema: output_step.step_schema().as_u32(),
            concrete_op: concrete_op_key.instance_key().template.fqn.clone(),
        })?;
    Ok(LateLoweredStepCaseEmission::new(
        case.case_tag(),
        case.concrete_op_key().clone(),
        case.payload_tuple_ty(),
        case.continuation_contract(),
        continuation_object,
    ))
}

fn build_handle_outward_emissions(
    root_fqn: &str,
    step_type: &LateLoweredStepType,
    facts: &HandleSiteEffectFacts,
    continuation_object: ContinuationObjectId,
) -> Result<Vec<LateLoweredStepCaseEmission>, EffectLoweringError> {
    let mut tags = BTreeSet::new();
    tags.extend(facts.body_outward_cases().tags().iter().copied());
    tags.extend(facts.finally_outward_cases().tags().iter().copied());
    for arm in facts.arm_facts() {
        tags.extend(arm.arm_outward_cases().tags().iter().copied());
    }
    tags.into_iter()
        .map(|case_tag| {
            build_current_step_emission(root_fqn, step_type, case_tag, continuation_object)
        })
        .collect()
}

fn resume_runtime_error_effect_family(
    root_fqn: &str,
    body: &Body,
    site_id: SiteId,
    types: &TypeStore,
) -> Result<EffectFamilyKey, EffectLoweringError> {
    let resume = find_resume_metadata(body, site_id).ok_or_else(|| {
        EffectLoweringError::MissingResumeSiteMetadata {
            root_fqn: root_fqn.to_string(),
            site_id: site_id.as_u32(),
        }
    })?;
    let runtime_error_ty = resume.runtime_error_effect_ty.ok_or_else(|| {
        EffectLoweringError::MissingResumeRuntimeErrorEffect {
            root_fqn: root_fqn.to_string(),
            site_id: site_id.as_u32(),
        }
    })?;
    effect_family_for_effect_ty(runtime_error_ty, types).ok_or_else(|| {
        EffectLoweringError::UnsupportedEffectFamilyType {
            root_fqn: root_fqn.to_string(),
            site_id: site_id.as_u32(),
            ty: runtime_error_ty.as_u32(),
        }
    })
}

fn find_resume_metadata(body: &Body, site_id: SiteId) -> Option<&ResumeMetadata> {
    for block in &body.blocks {
        for stmt in &block.stmts {
            let StatementKind::Assign {
                value:
                    Rvalue::Call {
                        site_id: stmt_site,
                        kind: CallKind::Resume { resume, .. },
                        ..
                    },
                ..
            } = &stmt.kind
            else {
                continue;
            };
            if *stmt_site == site_id {
                return Some(resume);
            }
        }
    }
    None
}

fn effect_family_for_effect_ty(
    effect_ty: crate::ty::TypeId,
    types: &TypeStore,
) -> Option<EffectFamilyKey> {
    match types.kind(effect_ty) {
        TypeKind::Ref(RefTypeKind::Nominal(NominalType { fqn, args, .. })) => {
            Some(EffectFamilyKey::new(fqn.clone(), args.clone()))
        }
        _ => None,
    }
}

fn is_runtime_error_raise_case(case: &LateLoweredStepCase, types: &TypeStore) -> bool {
    if case.concrete_op_key().instance_key().template.fqn != "scoop.core.Raise.raise" {
        return false;
    }

    types.display(case.payload_tuple_ty()).to_string() == "scoop.core.RuntimeError"
        || case
            .concrete_op_key()
            .effect_family()
            .type_args()
            .iter()
            .any(|&ty| types.display(ty).to_string() == "scoop.core.RuntimeError")
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::path::PathBuf;

    use crate::effect_facts::{
        CallTargetMode, ImplPlan, NestedHandleClassification, SiteEffectFacts,
    };
    use crate::effect_lowered::LateLoweredProgramBuilder;
    use crate::effect_lowered::ir::{
        BoundarySiteKind, LateLoweredBoundaryLowering, LateLoweredContinuationMethodReachability,
        LateLoweredContinuationResumeBody, LateLoweredHandleBoundaryCaseRoutingAction,
        LateLoweredHandlePendingCompletion, LateLoweredHandleStateRegion, LateLoweredOneShotPolicy,
        LateLoweredStateTerminator, SystemSlotKind,
    };
    use crate::effect_refactor_pipeline::load_effect_facts_stage_output_for_dump;
    use crate::mir::SiteId;
    use crate::session::{EffectPipelineMode, Session, SessionOptions};
    use crate::source::SourceFile;

    fn refactor_session() -> Session {
        Session::with_options(SessionOptions::new(EffectPipelineMode::Refactor)).unwrap()
    }

    fn load_fixture(phase: &str, name: &str) -> SourceFile {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures")
            .join(phase)
            .join(name);
        SourceFile::load(&path).expect("fixture 应可加载")
    }

    struct RawMaterializedOutput {
        effect_facts_stage_output: crate::effect_refactor_pipeline::RefactorEffectFactsStageOutput,
        program: crate::effect_lowered::LateLoweredProgram,
    }

    impl RawMaterializedOutput {
        fn program(&self) -> &crate::effect_lowered::LateLoweredProgram {
            &self.program
        }

        fn types(&self) -> &crate::ty::TypeStore {
            self.effect_facts_stage_output.types()
        }
    }

    fn load_output(source: &SourceFile) -> RawMaterializedOutput {
        let session = refactor_session();
        let effect_facts_stage_output = load_effect_facts_stage_output_for_dump(&session, source)
            .expect("fixture 应可通过 refactor effect-facts stage");
        let program = LateLoweredProgramBuilder::from_canonical_inputs(
            effect_facts_stage_output.materialized_pass_view(),
            effect_facts_stage_output.effect_facts(),
            effect_facts_stage_output.types(),
        )
        .build()
        .expect("fixture 应可通过 raw late-lowering builder");
        RawMaterializedOutput {
            effect_facts_stage_output,
            program,
        }
    }

    fn callable<'a>(
        output: &'a RawMaterializedOutput,
        fqn: &str,
    ) -> &'a crate::effect_lowered::LateLoweredCallable {
        output
            .program()
            .callable(fqn)
            .unwrap_or_else(|| panic!("late-lowered program 应发布 {fqn}"))
    }

    fn site_boundary(
        callable: &crate::effect_lowered::LateLoweredCallable,
        kind: BoundarySiteKind,
    ) -> &crate::effect_lowered::ir::LateLoweredBoundary {
        callable
            .boundary_map()
            .entries()
            .iter()
            .find(|boundary| {
                matches!(
                    boundary.source(),
                    crate::effect_lowered::ir::LateLoweredBoundarySource::Site { kind: boundary_kind, .. }
                        if boundary_kind == kind
                )
            })
            .expect("应找到指定 kind 的 boundary")
    }

    fn handle_dispatch_state(
        callable: &crate::effect_lowered::LateLoweredCallable,
        site_id: SiteId,
    ) -> &crate::effect_lowered::ir::LateLoweredState {
        callable
            .state_graph()
            .states()
            .iter()
            .find(|state| {
                matches!(
                    state.terminator(),
                    LateLoweredStateTerminator::HandleDispatch { site_id: state_site, .. }
                        if *state_site == site_id
                )
            })
            .expect("应找到指定 site 的 HandleDispatch state")
    }

    fn handle_site_facts<'a>(
        output: &'a RawMaterializedOutput,
        callable: &crate::effect_lowered::LateLoweredCallable,
        site_id: SiteId,
    ) -> &'a crate::effect_facts::HandleSiteEffectFacts {
        let body_facts = output
            .effect_facts_stage_output
            .effect_facts()
            .body(callable.instance_key())
            .expect("callable 应发布 body effect facts");
        match body_facts.site(site_id) {
            Some(SiteEffectFacts::Handle(facts)) => facts,
            other => panic!("应找到指定 site 的 Handle facts，而不是 {other:?}"),
        }
    }

    #[test]
    fn refactor_step_materialization_keeps_canonical_cases_and_dynamic_entry_states() {
        let output = load_output(&load_fixture("effect_facts", "single_case_impl_plan.scoop"));
        let leaf = callable(&output, "sample.leaf");
        let step_type = output
            .program()
            .step_type(leaf.step_schema())
            .expect("callable 应能回查 canonical Step shell");
        let case_fqns = step_type
            .cases()
            .iter()
            .map(|case| case.concrete_op_key().instance_key().template.fqn.clone())
            .collect::<BTreeSet<_>>();

        assert_eq!(
            case_fqns,
            [
                "sample.Ping.hit".to_string(),
                "scoop.core.Raise.raise".to_string()
            ]
            .into_iter()
            .collect()
        );
        assert_eq!(
            leaf.dynamic_invoke_entry().step_schema(),
            leaf.step_schema()
        );
        assert_eq!(
            leaf.dynamic_invoke_entry().entry_state(),
            leaf.state_graph().entry_state()
        );
        assert_eq!(
            leaf.dynamic_invoke_entry().complete_state(),
            leaf.state_graph().complete_state()
        );
    }

    #[test]
    fn refactor_resume_interface_completeness_groups_methods_by_effect_family() {
        let output = load_output(&load_fixture("effect_facts", "single_case_impl_plan.scoop"));
        let leaf = callable(&output, "sample.leaf");
        let interfaces = leaf
            .resume_interfaces()
            .iter()
            .map(|interface_id| {
                output
                    .program()
                    .resume_interface(*interface_id)
                    .expect("callable 应能回查 resume interface")
            })
            .collect::<Vec<_>>();

        assert_eq!(interfaces.len(), 2);
        assert_eq!(
            interfaces
                .iter()
                .map(|interface| interface.effect_family().effect_fqn().to_string())
                .collect::<BTreeSet<_>>(),
            ["sample.Ping".to_string(), "scoop.core.Raise".to_string()]
                .into_iter()
                .collect()
        );
        assert!(
            interfaces
                .iter()
                .all(|interface| interface.return_step_schema() == leaf.step_schema())
        );
        assert_eq!(
            interfaces
                .iter()
                .map(|interface| interface.methods().len())
                .sum::<usize>(),
            output
                .program()
                .step_type(leaf.step_schema())
                .expect("callable 应能回查 step shell")
                .cases()
                .len()
        );
    }

    #[test]
    fn refactor_continuation_object_materializes_surface_resume_and_one_shot_contracts() {
        let output = load_output(&load_fixture("effect_facts", "single_case_impl_plan.scoop"));
        let leaf = callable(&output, "sample.leaf");
        let object = output
            .program()
            .continuation_object(leaf.continuation_object())
            .expect("callable 应能回查 continuation object");

        assert_eq!(object.surface_resumes().len(), 2);
        assert_eq!(object.methods().len(), 2);
        assert_eq!(
            object
                .methods()
                .iter()
                .filter(|method| {
                    method.reachability() == LateLoweredContinuationMethodReachability::Reachable
                })
                .count(),
            1
        );
        assert!(object.surface_resumes().iter().any(|surface| {
            output.types().display(surface.surface_ty()).to_string()
                == "scoop.core.Continuation<Unit, Unit, eff sample.Ping>"
                && matches!(
                    surface.body(),
                    LateLoweredContinuationResumeBody::ResumeCapturedState {
                        repeated_resume: LateLoweredOneShotPolicy::OrdinaryRuntimeErrorOutward
                    }
                )
        }));
        assert!(object.surface_resumes().iter().any(|surface| {
            surface.concrete_op_key().instance_key().template.fqn == "scoop.core.Raise.raise"
                && surface.reachability() == LateLoweredContinuationMethodReachability::Unreachable
        }));
    }

    #[test]
    fn refactor_boundary_lowering_materializes_effectful_call_dispatch_contract() {
        let output = load_output(&load_fixture(
            "effect_facts",
            "dynamic_fallback_widening.scoop",
        ));
        let call_value = callable(&output, "sample.callValue");
        let boundary = site_boundary(call_value, BoundarySiteKind::Call);
        let LateLoweredBoundaryLowering::Call(lowering) = boundary
            .lowering()
            .expect("call boundary 应发布 lowering contract")
        else {
            panic!("call boundary 应物化成 Call lowering")
        };

        assert_eq!(
            lowering.facts().target_mode(),
            CallTargetMode::DynamicFallback
        );
        assert_eq!(
            lowering.dispatch().input_step_schema(),
            lowering.facts().callee_schema()
        );
        assert_eq!(
            lowering.dispatch().complete().target_state(),
            boundary.resume_state()
        );
        assert_eq!(lowering.dispatch().outward_cases().len(), 2);
        assert!(lowering.consumed_runtime_error_case().is_none());
        assert_eq!(
            lowering
                .dispatch()
                .outward_cases()
                .iter()
                .map(|forwarding| {
                    forwarding
                        .emission()
                        .concrete_op_key()
                        .instance_key()
                        .template
                        .fqn
                        .clone()
                })
                .collect::<BTreeSet<_>>(),
            ["sample.Alpha.go".to_string(), "sample.Beta.go".to_string()]
                .into_iter()
                .collect()
        );
    }

    #[test]
    fn refactor_boundary_lowering_keeps_local_runtime_error_contract_for_pure_caller_calls() {
        let output = load_output(&load_fixture(
            "run-pass",
            "effect_resume_if_else_branch_single_perform.scoop",
        ));
        let main = callable(&output, "main");
        let step_type = output
            .program()
            .step_type(main.step_schema())
            .expect("main 应能回查 canonical Step shell");
        let call_boundaries = main
            .boundary_map()
            .entries()
            .iter()
            .filter_map(|boundary| match boundary.lowering() {
                Some(LateLoweredBoundaryLowering::Call(lowering)) => Some((boundary, lowering)),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert!(main.resolved_outward_cases().is_empty());
        assert!(step_type.cases().is_empty());
        assert_eq!(call_boundaries.len(), 2);
        assert!(
            call_boundaries
                .iter()
                .all(|(_, lowering)| lowering.dispatch().outward_cases().is_empty())
        );
        for (boundary, lowering) in call_boundaries {
            let runtime_error_case = lowering
                .consumed_runtime_error_case()
                .expect("pure caller 的 call boundary 应显式发布本地 runtime-error contract");
            assert_eq!(runtime_error_case.input_case_tag().as_u32(), 1);
            assert_eq!(
                runtime_error_case
                    .input_concrete_op_key()
                    .instance_key()
                    .template
                    .fqn,
                "scoop.core.Raise.raise"
            );
            assert_eq!(
                output
                    .types()
                    .display(runtime_error_case.payload_tuple_ty())
                    .to_string(),
                "scoop.core.RuntimeError"
            );
            assert_eq!(
                runtime_error_case.terminal_action(),
                crate::effect_lowered::ir::LateLoweredLocalRuntimeErrorTerminalAction::RuntimeFatal {
                    runtime_entry:
                        crate::effect_lowered::ir::LateLoweredPublishedRuntimeEntry::RuntimeErrorFatal,
                }
            );
            let target_state = main
                .state_graph()
                .states()
                .iter()
                .find(|state| state.state_id() == runtime_error_case.target_state())
                .expect("本地 runtime-error contract 应发布 dedicated target state");
            assert!(main.state_graph().states().iter().any(|state| {
                state.state_id() == boundary.owner_state()
                    && matches!(
                        state.terminator(),
                        crate::effect_lowered::ir::LateLoweredStateTerminator::Suspend {
                            local_runtime_error_states,
                            ..
                        } if local_runtime_error_states.contains(&runtime_error_case.target_state())
                    )
            }));
            assert!(matches!(
                target_state.terminator(),
                crate::effect_lowered::ir::LateLoweredStateTerminator::LocalRuntimeError {
                    payload_tuple_ty,
                    terminal_action,
                } if *payload_tuple_ty == runtime_error_case.payload_tuple_ty()
                    && *terminal_action == runtime_error_case.terminal_action()
            ));
        }
    }

    #[test]
    fn refactor_boundary_lowering_materializes_resume_and_runtime_error_contracts() {
        let output = load_output(&load_fixture(
            "mir_refactor",
            "dispatch_and_resume_call.scoop",
        ));
        let callable = callable(&output, "fixtures.mir.resumeBoom");
        let resume_boundary = site_boundary(callable, BoundarySiteKind::Resume);
        let runtime_error_boundary = callable
            .boundary_map()
            .entries()
            .iter()
            .find(|boundary| {
                matches!(
                    boundary.source(),
                    crate::effect_lowered::ir::LateLoweredBoundarySource::RuntimeError { .. }
                )
            })
            .expect("resume callable 应发布 runtime-error boundary");

        let LateLoweredBoundaryLowering::Resume(resume_lowering) = resume_boundary
            .lowering()
            .expect("resume boundary 应发布 lowering contract")
        else {
            panic!("resume boundary 应物化成 Resume lowering")
        };
        let LateLoweredBoundaryLowering::RuntimeError(runtime_error_lowering) =
            runtime_error_boundary
                .lowering()
                .expect("runtime-error boundary 应发布 lowering contract")
        else {
            panic!("runtime-error boundary 应物化成 RuntimeError lowering")
        };

        assert_eq!(
            resume_lowering.runtime_error_boundary(),
            runtime_error_boundary.boundary_id()
        );
        assert_eq!(
            runtime_error_lowering.resume_boundary(),
            resume_boundary.boundary_id()
        );
        assert_eq!(
            resume_lowering.dispatch().input_step_schema(),
            resume_lowering.facts().out_step_schema()
        );
        assert_eq!(resume_lowering.dispatch().outward_cases().len(), 2);
        assert_eq!(
            runtime_error_lowering
                .emitted_step()
                .concrete_op_key()
                .instance_key()
                .template
                .fqn,
            "scoop.core.Raise.raise"
        );
    }

    #[test]
    fn refactor_boundary_lowering_materializes_perform_and_handle_contracts() {
        let perform_output = load_output(&load_fixture("effect_facts", "handle_perform.scoop"));
        let handled_main = callable(&perform_output, "a.main");
        let perform_boundary = site_boundary(handled_main, BoundarySiteKind::Perform);
        let LateLoweredBoundaryLowering::Perform(perform_lowering) = perform_boundary
            .lowering()
            .expect("perform boundary 应发布 lowering contract")
        else {
            panic!("perform boundary 应物化成 Perform lowering")
        };
        assert_eq!(
            perform_lowering.facts().emitted_case(),
            perform_lowering.emitted_step().case_tag()
        );
        assert_eq!(
            perform_lowering
                .emitted_step()
                .concrete_op_key()
                .instance_key()
                .template
                .fqn,
            "scoop.core.Raise.raise"
        );

        let handle_output = load_output(&load_fixture(
            "effect_facts",
            "nested_handle_self_contained_vs_outward.scoop",
        ));
        let outward = callable(&handle_output, "sample.nested_may_suspend_outward");
        let handle_boundary = site_boundary(outward, BoundarySiteKind::Handle);
        let LateLoweredBoundaryLowering::Handle(handle_lowering) = handle_boundary
            .lowering()
            .expect("handle boundary 应发布 lowering contract")
        else {
            panic!("handle boundary 应物化成 Handle lowering")
        };
        assert_eq!(
            handle_lowering.facts().nested_handle_classification(),
            NestedHandleClassification::MaySuspendOutward
        );
        assert_eq!(handle_lowering.outward_emissions().len(), 1);
        assert_eq!(
            handle_lowering.outward_emissions()[0]
                .concrete_op_key()
                .instance_key()
                .template
                .fqn,
            "sample.Outer.again"
        );
    }

    #[test]
    fn refactor_handle_dispatch_contract_publishes_body_arm_finally_and_outward_routes() {
        let output = load_output(&load_fixture(
            "effect_facts",
            "nested_handle_self_contained_vs_outward.scoop",
        ));
        let callable = callable(&output, "sample.nested_may_suspend_outward");
        let handle_state = handle_dispatch_state(callable, SiteId::from_raw(1));
        let LateLoweredStateTerminator::HandleDispatch {
            arm_states,
            finally_state,
            exit_state,
            contract,
            ..
        } = handle_state.terminator()
        else {
            panic!("指定 state 应保持 HandleDispatch terminator");
        };

        assert_eq!(
            contract.carrier().state_tag_slot(),
            SystemSlotKind::StateTag
        );
        assert_eq!(
            contract.carrier().completion_tag_slot(),
            SystemSlotKind::CompletionTag
        );
        assert_eq!(
            contract.carrier().payload_carrier_slot(),
            SystemSlotKind::ResumePayloadCarrier
        );
        assert_eq!(
            contract.body_complete_target(),
            finally_state.expect("fixture 应保留 finally state")
        );
        assert_eq!(
            contract.arm_complete_target(),
            finally_state.expect("fixture 应保留 finally state")
        );
        assert_eq!(contract.finally_complete_target(), Some(*exit_state));
        assert_eq!(
            contract.abandon_target(),
            callable.state_graph().drop_state()
        );
        assert_eq!(contract.handled_arms().len(), 1);
        assert_eq!(contract.handled_arms()[0].handled_case().as_u32(), 0);
        assert_eq!(contract.handled_arms()[0].arm_state(), arm_states[0]);
        assert!(contract.handled_arms()[0].arm_outward_cases().is_empty());
        assert!(contract.body_outward_cases().is_empty());
        assert_eq!(
            contract.finally_outward_cases(),
            &[crate::effect_facts::CaseTag::new(1)]
        );
        assert!(
            contract
                .outward_emission(crate::effect_facts::CaseTag::new(1))
                .is_some(),
            "finally outward case 应能回查 published outward emission"
        );
        assert!(
            contract
                .pending_completions()
                .contains(&LateLoweredHandlePendingCompletion::ContinueToExit)
        );
        assert!(
            contract
                .pending_completions()
                .contains(&LateLoweredHandlePendingCompletion::ReturnFromFunction)
        );
        assert!(
            !contract.pending_completions().contains(
                &LateLoweredHandlePendingCompletion::PropagateOutward(
                    crate::effect_facts::CaseTag::new(1)
                )
            ),
            "仅 finally outward 的 case 不应被误发布成 pending completion tag"
        );
    }

    #[test]
    fn refactor_handle_dispatch_region_contract_publishes_body_routing_for_handled_perform() {
        let output = load_output(&load_fixture(
            "run-pass",
            "effect_resume_if_else_branch_single_perform.scoop",
        ));
        let callable = callable(&output, "run");
        let (_site_id, contract) = callable
            .state_graph()
            .states()
            .iter()
            .find_map(|state| match state.terminator() {
                LateLoweredStateTerminator::HandleDispatch {
                    site_id, contract, ..
                } => Some((*site_id, contract)),
                _ => None,
            })
            .expect("run 应发布 HandleDispatch contract");
        let handled_arm = contract
            .handled_arms()
            .first()
            .expect("single-perform fixture 应发布唯一 handled arm");
        let body_route = contract
            .boundary_routings()
            .iter()
            .find(|routing| {
                matches!(routing.owner_region(), LateLoweredHandleStateRegion::Body)
                    && callable
                        .boundary_map()
                        .boundary(routing.boundary_id())
                        .is_some_and(|boundary| {
                            matches!(
                                boundary.source(),
                                crate::effect_lowered::ir::LateLoweredBoundarySource::Site {
                                    kind: BoundarySiteKind::Perform,
                                    ..
                                }
                            )
                        })
            })
            .expect("handle body 内的 perform boundary 应发布 body-region routing");
        let route = body_route
            .case_routing(handled_arm.handled_case())
            .expect("handled perform case 应发布 consume-to-arm routing");

        assert_eq!(
            contract.state_region(body_route.owner_state()),
            LateLoweredHandleStateRegion::Body
        );
        assert_eq!(
            contract.state_region(body_route.resume_state()),
            LateLoweredHandleStateRegion::Body
        );
        assert!(matches!(
            route.action(),
            LateLoweredHandleBoundaryCaseRoutingAction::ConsumeToArm {
                arm_state,
                arm_ordinal,
                continuation_resume_state,
            } if arm_state == handled_arm.arm_state()
                && arm_ordinal == handled_arm.arm_ordinal()
                && continuation_resume_state == body_route.resume_state()
        ));
    }

    #[test]
    fn refactor_handle_dispatch_region_contract_tracks_multi_resume_routes_and_arm_regions() {
        let output = load_output(&load_fixture(
            "run-pass",
            "effect_multi_escape_indirect_direct_while.scoop",
        ));
        let callable = callable(&output, "main");
        let (_site_id, contract) = callable
            .state_graph()
            .states()
            .iter()
            .find_map(|state| match state.terminator() {
                LateLoweredStateTerminator::HandleDispatch {
                    site_id, contract, ..
                } => Some((*site_id, contract)),
                _ => None,
            })
            .expect("main 应发布 HandleDispatch contract");
        let ask_arm = contract
            .handled_arms()
            .iter()
            .find(|arm| arm.continuation_binder().is_some())
            .expect("Ask arm 应发布 escape continuation binder");
        let consume_routes = contract
            .boundary_routings()
            .iter()
            .filter_map(|routing| {
                routing
                    .case_routing(ask_arm.handled_case())
                    .map(|route| (routing, route))
            })
            .collect::<Vec<_>>();
        let resume_states = consume_routes
            .iter()
            .map(|(routing, route)| match route.action() {
                LateLoweredHandleBoundaryCaseRoutingAction::ConsumeToArm {
                    arm_state,
                    arm_ordinal,
                    continuation_resume_state,
                } => {
                    assert_eq!(arm_state, ask_arm.arm_state());
                    assert_eq!(arm_ordinal, ask_arm.arm_ordinal());
                    assert_eq!(continuation_resume_state, routing.resume_state());
                    continuation_resume_state
                }
                other => panic!("Ask handled case 应走 consume-to-arm，而不是 {other:?}"),
            })
            .collect::<BTreeSet<_>>();

        assert!(
            consume_routes.len() >= 2,
            "indirect/direct mixed fixture 应至少发布两个 Ask consume route"
        );
        assert!(
            resume_states.len() >= 2,
            "不同 body boundary 的 continuation resume_state 应被稳定区分"
        );
        assert!(
            resume_states
                .iter()
                .all(|state_id| contract.state_region(*state_id)
                    == LateLoweredHandleStateRegion::Body)
        );
        assert!(contract.state_regions().iter().any(|entry| matches!(
            entry.region(),
            LateLoweredHandleStateRegion::Arm { arm_ordinal: 0, .. }
        )));
        assert!(contract.state_regions().iter().any(|entry| matches!(
            entry.region(),
            LateLoweredHandleStateRegion::Arm { arm_ordinal: 1, .. }
        )));
    }

    #[test]
    fn refactor_handle_dispatch_region_contract_tracks_pending_and_finally_routing() {
        let pending_output = load_output(&SourceFile::new_virtual(
            "<mem>/late_lowered_handle_region_pending.scoop",
            r#"
package sample

effect Inner {
    fun go(): Int
}

effect Outer {
    fun again(): Unit
}

fun cleanup() {}

fun propagate_before_finally(): Int {
    return handle {
        val nested: Int = handle {
            Outer.again()
            0
        } with {
            Inner.go() -> 1
        } finally {
            cleanup()
        }
        nested + 10
    } with {
        Outer.again() -> 99
    }
}
"#,
        ));
        let pending_callable = callable(&pending_output, "sample.propagate_before_finally");
        let pending_contract =
            match handle_dispatch_state(pending_callable, SiteId::from_raw(1)).terminator() {
                LateLoweredStateTerminator::HandleDispatch { contract, .. } => contract,
                other => panic!("期望 HandleDispatch terminator，而不是 {other:?}"),
            };
        let pending_case = pending_contract.body_outward_cases()[0];
        let pending_route = pending_contract
            .boundary_routings()
            .iter()
            .find(|routing| {
                matches!(routing.owner_region(), LateLoweredHandleStateRegion::Body)
                    && routing.case_routing(pending_case).is_some()
            })
            .expect("body outward case 应发布 pending routing");
        assert!(matches!(
            pending_route
                .case_routing(pending_case)
                .expect("pending case 应可回查")
                .action(),
            LateLoweredHandleBoundaryCaseRoutingAction::PendingCompletion {
                completion: LateLoweredHandlePendingCompletion::PropagateOutward(case_tag),
            } if case_tag == pending_case
        ));

        let finally_output = load_output(&SourceFile::new_virtual(
            "<mem>/late_lowered_handle_region_finally_outward.scoop",
            r#"
package sample

effect Inner {
    fun go(): Int
}

effect Outer {
    fun again(): Unit
}

fun finally_outward(): Int / (Outer) {
    return handle {
        Inner.go()
        0
    } with {
        Inner.go() -> 1
    } finally {
        Outer.again()
    }
}
"#,
        ));
        let finally_callable = callable(&finally_output, "sample.finally_outward");
        let (_site_id, finally_contract) = finally_callable
            .state_graph()
            .states()
            .iter()
            .find_map(|state| match state.terminator() {
                LateLoweredStateTerminator::HandleDispatch {
                    site_id, contract, ..
                } => Some((*site_id, contract)),
                _ => None,
            })
            .expect("finally_outward 应发布 HandleDispatch contract");
        let finally_case = finally_contract.finally_outward_cases()[0];
        let finally_route = finally_contract
            .boundary_routings()
            .iter()
            .find(|routing| {
                matches!(
                    routing.owner_region(),
                    LateLoweredHandleStateRegion::Finally
                ) && routing.case_routing(finally_case).is_some()
            })
            .expect("finally outward case 应发布 finally-region routing");
        assert!(matches!(
            finally_route
                .case_routing(finally_case)
                .expect("finally case 应可回查")
                .action(),
            LateLoweredHandleBoundaryCaseRoutingAction::EmitOutward
        ));
    }

    #[test]
    fn refactor_handle_arm_binding_contract_publishes_payload_and_escape_continuation_binding() {
        let output = load_output(&SourceFile::new_virtual(
            "<mem>/late_lowered_handle_arm_binding_single.scoop",
            r#"
package sample

import scoop.core.*

effect Edge {
    fun visit(from: String, to: Int): Int
}

fun run(): Int {
    return handle {
        Edge.visit("alpha", 1)
    } with {
        Edge.visit(from, to), k -> {
            k.resume(to + 1)
        }
    }
}

fun main(): Int {
    return 0
}
"#,
        ));
        let callable = callable(&output, "sample.run");
        let (site_id, contract) = callable
            .state_graph()
            .states()
            .iter()
            .find_map(|state| match state.terminator() {
                LateLoweredStateTerminator::HandleDispatch {
                    site_id, contract, ..
                } => Some((*site_id, contract)),
                _ => None,
            })
            .expect("run 应发布 HandleDispatch contract");
        let arm = contract
            .handled_arms()
            .first()
            .expect("单 arm fixture 应发布唯一 handled arm");
        let facts = handle_site_facts(&output, callable, site_id);
        let expected = &facts.arm_facts()[0];

        assert_eq!(arm.arm_ordinal(), 0);
        assert_eq!(arm.payload_tuple_ty(), expected.payload_tuple_ty());
        assert_eq!(arm.payload_binders().len(), 2);
        assert_eq!(arm.payload_binders()[0].ordinal(), 0);
        assert_eq!(arm.payload_binders()[1].ordinal(), 1);
        assert_ne!(
            arm.payload_binders()[0].local(),
            arm.payload_binders()[1].local(),
            "不同 payload binder 必须稳定绑定到不同 local"
        );
        let continuation_binder = arm
            .continuation_binder()
            .expect("escape continuation arm 必须发布 continuation binder contract");
        assert_eq!(
            continuation_binder.continuation_schema(),
            expected.continuation_schema()
        );
        assert_eq!(
            continuation_binder.continuation_object(),
            callable.continuation_object()
        );
    }

    #[test]
    fn refactor_handle_arm_binding_contract_publishes_mixed_multi_arm_bindings_without_ambiguity() {
        let output = load_output(&load_fixture(
            "run-pass",
            "effect_multi_escape_indirect_direct_while.scoop",
        ));
        let callable = callable(&output, "main");
        let (site_id, contract) = callable
            .state_graph()
            .states()
            .iter()
            .find_map(|state| match state.terminator() {
                LateLoweredStateTerminator::HandleDispatch {
                    site_id, contract, ..
                } => Some((*site_id, contract)),
                _ => None,
            })
            .expect("main 应发布 HandleDispatch contract");
        let facts = handle_site_facts(&output, callable, site_id);

        assert_eq!(contract.handled_arms().len(), 2);
        let mut arm_ordinals = contract
            .handled_arms()
            .iter()
            .map(|arm| arm.arm_ordinal())
            .collect::<Vec<_>>();
        arm_ordinals.sort();
        assert_eq!(arm_ordinals, vec![0, 1]);

        let escape_arm = contract
            .handled_arms()
            .iter()
            .find(|arm| arm.continuation_binder().is_some())
            .expect("mixed fixture 应发布带 continuation binder 的 arm");
        let payload_only_arm = contract
            .handled_arms()
            .iter()
            .find(|arm| arm.continuation_binder().is_none())
            .expect("mixed fixture 应发布纯 payload arm");
        assert_eq!(escape_arm.payload_binders().len(), 1);
        assert_eq!(payload_only_arm.payload_binders().len(), 1);

        let expected_by_case = facts
            .arm_facts()
            .iter()
            .map(|arm| (arm.handled_case(), arm.continuation_schema()))
            .collect::<std::collections::BTreeMap<_, _>>();
        assert_eq!(
            escape_arm
                .continuation_binder()
                .expect("escape arm 应带 continuation binder")
                .continuation_schema(),
            *expected_by_case
                .get(&escape_arm.handled_case())
                .expect("handled case 应能回查 arm facts continuation schema")
        );
        assert_eq!(
            payload_only_arm.payload_tuple_ty(),
            facts
                .arm_facts()
                .iter()
                .find(|arm| arm.handled_case() == payload_only_arm.handled_case())
                .expect("payload-only arm handled case 应能回查 facts")
                .payload_tuple_ty()
        );
    }

    #[test]
    fn refactor_completion_state_contract_tracks_body_outward_cases_across_finally() {
        let output = load_output(&SourceFile::new_virtual(
            "<mem>/late_lowered_handle_body_outward_finally.scoop",
            r#"
package sample

effect Inner {
    fun go(): Int
}

effect Outer {
    fun again(): Unit
}

fun cleanup() {}

fun propagate_before_finally(): Int {
    return handle {
        val nested: Int = handle {
            Outer.again()
            0
        } with {
            Inner.go() -> 1
        } finally {
            cleanup()
        }
        nested + 10
    } with {
        Outer.again() -> 99
    }
}
"#,
        ));
        let callable = callable(&output, "sample.propagate_before_finally");
        let handle_state = handle_dispatch_state(callable, SiteId::from_raw(1));
        let LateLoweredStateTerminator::HandleDispatch { contract, .. } = handle_state.terminator()
        else {
            panic!("指定 state 应保持 HandleDispatch terminator");
        };

        assert_eq!(contract.body_outward_cases().len(), 1);
        let outward_case = contract.body_outward_cases()[0];
        assert!(contract.finally_outward_cases().is_empty());
        assert!(contract.pending_completions().contains(
            &LateLoweredHandlePendingCompletion::PropagateOutward(outward_case,)
        ));
        assert!(contract.outward_emission(outward_case).is_some());
    }

    #[test]
    fn refactor_handle_dispatch_contract_dump_exposes_published_completion_state() {
        let output = load_output(&SourceFile::new_virtual(
            "<mem>/late_lowered_handle_contract_dump.scoop",
            r#"
package sample

effect Inner {
    fun go(): Int
}

effect Outer {
    fun again(): Unit
}

fun cleanup() {}

fun propagate_before_finally(): Int {
    return handle {
        val nested: Int = handle {
            Outer.again()
            0
        } with {
            Inner.go() -> 1
        } finally {
            cleanup()
        }
        nested + 10
    } with {
        Outer.again() -> 99
    }
}
"#,
        ));
        let dump = output.program().stable_dump();

        assert!(dump.contains("handle_contract:"));
        assert!(dump.contains("pending_completions:"));
        assert!(dump.contains("state_regions:"));
        assert!(dump.contains("boundary_routings:"));
        assert!(dump.contains("case_routings:"));
        assert!(dump.contains("PropagateOutward("));
        assert!(dump.contains("outward_emissions:"));
    }

    #[test]
    fn refactor_handle_arm_binding_contract_dump_exposes_payload_and_continuation_binders() {
        let output = load_output(&load_fixture(
            "run-pass",
            "effect_multi_escape_indirect_direct_while.scoop",
        ));
        let dump = output.program().stable_dump();

        assert!(dump.contains("payload_binders:"));
        assert!(dump.contains("continuation_binder:"));
        assert!(dump.contains("continuation_schema="));
    }

    #[test]
    fn refactor_impl_plan_lowering_keeps_no_outward_single_case_and_canonical_full_distinct() {
        let no_outward_output = load_output(&SourceFile::new_virtual(
            "<mem>/late_lowered_no_outward.scoop",
            "package sample\nfun helper() {}\nfun main() { helper() }\n",
        ));
        let no_outward = callable(&no_outward_output, "sample.main");
        assert_eq!(no_outward.impl_plan(), ImplPlan::NoOutward);
        assert!(no_outward.boundary_map().entries().is_empty());

        let single_case_output =
            load_output(&load_fixture("effect_facts", "single_case_impl_plan.scoop"));
        let single_case = callable(&single_case_output, "sample.leaf");
        let single_case_object = single_case_output
            .program()
            .continuation_object(single_case.continuation_object())
            .expect("single-case callable 应能回查 continuation object");
        assert!(matches!(single_case.impl_plan(), ImplPlan::SingleCase(_)));
        assert_eq!(
            single_case_object
                .methods()
                .iter()
                .filter(|method| {
                    method.reachability() == LateLoweredContinuationMethodReachability::Reachable
                })
                .count(),
            1
        );

        let canonical_output = load_output(&load_fixture(
            "effect_facts",
            "dynamic_fallback_widening.scoop",
        ));
        let canonical = callable(&canonical_output, "sample.callValue");
        let canonical_boundary = site_boundary(canonical, BoundarySiteKind::Call);
        let LateLoweredBoundaryLowering::Call(canonical_lowering) = canonical_boundary
            .lowering()
            .expect("canonical-full boundary 应发布 lowering contract")
        else {
            panic!("canonical-full boundary 应物化成 Call lowering")
        };
        assert_eq!(canonical.impl_plan(), ImplPlan::CanonicalFull);
        assert_eq!(canonical_lowering.dispatch().outward_cases().len(), 2);
    }
}
