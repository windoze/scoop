//! Step / dynamic-invoke / continuation-object / boundary-map / handle-dispatch contract materialization entry points.

#![allow(dead_code)]

use super::*;

pub(crate) fn materialize_step_and_resume_interfaces(
    effect_facts: &MaterializedEffectFacts,
) -> Result<StepMaterialization, EffectLoweringError> {
    let mut step_types = Vec::with_capacity(effect_facts.step_schemas().len());
    let mut resume_packings = Vec::new();
    let mut resume_packing_ids_by_step = BTreeMap::new();
    let mut resume_packing_ids_by_group = BTreeMap::new();
    let mut next_interface_raw = 0u32;

    for (&step_schema_id, step_schema) in effect_facts.step_schemas() {
        step_types.push(build_step_type(step_schema_id, step_schema, effect_facts)?);

        let grouped_cases = group_cases_by_effect_family(step_schema);
        let mut interface_ids = Vec::with_capacity(grouped_cases.len());
        for (effect_family, cases) in grouped_cases {
            let interface_id = ResumeInterfaceId::new(next_interface_raw);
            next_interface_raw = next_interface_raw.saturating_add(1);
            resume_packings.push(build_resume_interface(
                interface_id,
                effect_family.clone(),
                step_schema_id,
                step_schema,
                &cases,
                effect_facts,
            )?);
            resume_packing_ids_by_group.insert((step_schema_id, effect_family), interface_id);
            interface_ids.push(interface_id);
        }
        resume_packing_ids_by_step.insert(step_schema_id, interface_ids);
    }

    Ok(StepMaterialization {
        step_types,
        resume_packings,
        resume_packing_ids_by_step,
        resume_packing_ids_by_group,
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
        implemented_packings,
        resume_packing_ids_by_group,
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
            let interface_id = *resume_packing_ids_by_group
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
        implemented_packings.to_vec(),
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
        owner_version_key,
        body,
        body_facts,
        step_type,
        state_graph,
        frame_schema,
        boundary_map,
        continuation_object,
        step_types,
        types,
        nominal_direct_supertypes,
        cross_callable_continuation_provenance,
    } = inputs;

    let result_locals = collect_result_locals(body);
    let continuation_provenance = PublishedContinuationProvenance::build(
        root_fqn,
        body,
        body_facts,
        owner_version_key,
        continuation_object,
        cross_callable_continuation_provenance,
    )?;
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
                let result_frame_slot =
                    published_boundary_result_slot(frame_schema, boundary.boundary_id()).and_then(
                        |(slot_local, slot_id)| (slot_local == result_local).then_some(slot_id),
                    );
                let call_dispatch =
                    build_call_boundary_dispatch_plan(CallBoundaryDispatchInputs {
                        root_fqn,
                        boundary_id: boundary.boundary_id(),
                        input_step,
                        output_step: step_type,
                        outward_case_tags: facts.resolved_cases().tags(),
                        continuation_object,
                        target_state: boundary.resume_state(),
                        result_local: Some(result_local),
                        result_frame_slot,
                        types,
                    })?;
                let operand_contract = build_call_boundary_operand_contract(
                    root_fqn,
                    body,
                    state_graph,
                    boundary,
                    &facts,
                    result_local,
                    types,
                    nominal_direct_supertypes,
                )?;
                let metadata = materialized_call_site_metadata(root_fqn, body, site_id)?;
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
                    metadata,
                    operand_contract,
                    call_dispatch.dispatch,
                    call_dispatch.continuation_compositions,
                    consumed_runtime_error_case,
                ))
            }
            LateLoweredBoundarySource::Site {
                site_id,
                kind: BoundarySiteKind::ClassCtor,
            } => {
                let facts = clone_class_ctor_site_facts(root_fqn, body_facts, site_id)?;
                let result_local = *result_locals.call_results.get(&site_id).ok_or_else(|| {
                    EffectLoweringError::MissingBoundaryResultLocal {
                        root_fqn: root_fqn.to_string(),
                        site_id: site_id.as_u32(),
                        kind: "ClassCtor",
                    }
                })?;
                let (class_fqn, source_consumption) = build_class_ctor_boundary_source_contract(
                    root_fqn,
                    body,
                    state_graph,
                    boundary,
                    result_local,
                )?;
                let emitted_steps = facts
                    .emitted_cases()
                    .tags()
                    .iter()
                    .map(|case_tag| {
                        build_current_step_emission(
                            root_fqn,
                            step_type,
                            *case_tag,
                            continuation_object,
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                LateLoweredBoundaryLowering::ClassCtor(LateLoweredClassCtorBoundaryLowering::new(
                    facts,
                    result_local,
                    class_fqn,
                    source_consumption,
                    emitted_steps,
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
                let operand_payload_ty = if is_any_type(types, facts.payload_tuple_ty()) {
                    emitted_step.payload_tuple_ty()
                } else {
                    facts.payload_tuple_ty()
                };
                let operand_contract = build_perform_boundary_operand_contract(
                    root_fqn,
                    body,
                    state_graph,
                    boundary,
                    operand_payload_ty,
                    types,
                    nominal_direct_supertypes,
                )?;
                LateLoweredBoundaryLowering::Perform(LateLoweredPerformBoundaryLowering::new(
                    facts,
                    operand_contract,
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
                let result_frame_slot =
                    published_boundary_result_slot(frame_schema, boundary.boundary_id()).and_then(
                        |(slot_local, slot_id)| (slot_local == result_local).then_some(slot_id),
                    );
                let continuation_compositions = build_boundary_continuation_compositions(
                    root_fqn,
                    boundary.boundary_id(),
                    input_step,
                    &dispatch,
                    boundary.resume_state(),
                    result_local,
                    result_frame_slot,
                )?;
                let operand_contract = build_resume_boundary_operand_contract(
                    root_fqn,
                    owner_version_key,
                    body,
                    state_graph,
                    boundary,
                    &facts,
                    result_local,
                    &continuation_provenance,
                    continuation_object,
                    types,
                    nominal_direct_supertypes,
                )?;
                LateLoweredBoundaryLowering::Resume(LateLoweredResumeBoundaryLowering::new(
                    facts,
                    result_local,
                    runtime_error_boundary,
                    operand_contract,
                    dispatch,
                    continuation_compositions,
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
        types,
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

pub(crate) fn attach_local_runtime_error_states(
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

#[allow(clippy::too_many_arguments)]
pub(crate) fn attach_handle_dispatch_contracts(
    root_fqn: &str,
    body: &Body,
    body_facts: &BodyEffectFacts,
    types: &TypeStore,
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
                        types,
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
pub(crate) fn build_handle_dispatch_contract(
    root_fqn: &str,
    body: &Body,
    dispatch_state: StateId,
    body_state: StateId,
    site_id: SiteId,
    facts: &HandleSiteEffectFacts,
    types: &TypeStore,
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
            let completion_payload_source = handle_arm_completion_payload_source(
                root_fqn,
                site_id,
                body,
                types,
                state_graph,
                arm_state,
                arm_states,
                finally_state,
                exit_state,
                arm.body_ty,
            )?;
            Ok(LateLoweredHandleArmDispatch::new(
                arm_facts.handled_case(),
                arm_state,
                arm_ordinal as u32,
                arm_facts.payload_tuple_ty(),
                completion_payload_source,
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
    let body_completion_payload_source = handle_body_completion_payload_source(
        root_fqn,
        site_id,
        body,
        types,
        state_graph,
        &state_regions,
        body_complete_target,
        facts.result_ty(),
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
    let pending_payload_transports = build_handle_pending_payload_transports(
        root_fqn,
        site_id,
        &pending_completions,
        &outward_emissions,
        frame_schema,
    )?;
    let pending_completion_origins = build_handle_pending_completion_origins(
        root_fqn,
        site_id,
        &pending_completions,
        &boundary_routings,
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
        Some(body_completion_payload_source),
        handled_arms,
        body_outward_cases,
        finally_outward_cases,
        outward_emissions,
        pending_completions,
        pending_completion_origins,
        pending_payload_transports,
        state_regions,
        boundary_routings,
        drop_state,
    ))
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn handle_arm_completion_payload_source(
    root_fqn: &str,
    site_id: SiteId,
    body: &Body,
    types: &TypeStore,
    state_graph: &LateLoweredStateGraph,
    arm_state: StateId,
    arm_states: &[StateId],
    finally_state: Option<StateId>,
    exit_state: StateId,
    body_ty: TypeId,
) -> Result<LateLoweredCompletionPayloadSource, EffectLoweringError> {
    let arm_complete_target = finally_state.unwrap_or(exit_state);
    if matches!(
        types.kind(body_ty),
        TypeKind::Value(ValueTypeKind::Unit | ValueTypeKind::Nothing)
    ) {
        return Ok(LateLoweredCompletionPayloadSource::unit(body_ty));
    }

    let mut stop_states = BTreeSet::from([exit_state]);
    stop_states.extend(
        arm_states
            .iter()
            .copied()
            .filter(|state| *state != arm_state),
    );
    if let Some(finally_state) = finally_state {
        stop_states.insert(finally_state);
    }

    let mut published = None;
    for state_id in
        collect_handle_region_states(root_fqn, site_id, state_graph, arm_state, &stop_states)?
    {
        let state = state_graph.state(state_id).ok_or_else(|| {
            invalid_handle_dispatch_contract(
                root_fqn,
                site_id,
                format!(
                    "handle arm completion payload source 引用了不存在的 arm state st{}",
                    state_id.as_u32()
                ),
            )
        })?;
        if !matches!(
            state.terminator(),
            LateLoweredStateTerminator::Goto { target } if *target == arm_complete_target
        ) {
            continue;
        }
        let candidate = handle_completion_payload_source_from_state(
            root_fqn,
            site_id,
            body,
            types,
            state_graph,
            state.state_id(),
            body_ty,
            "handle arm completion payload source",
        )?;
        if let Some(existing) = &published {
            if !same_completion_payload_source_ignoring_span(existing, &candidate) {
                return Err(invalid_handle_dispatch_contract(
                    root_fqn,
                    site_id,
                    format!(
                        "handle arm completion payload source 歧义：已发布 {:?}，又发现 {:?}",
                        existing, candidate
                    ),
                ));
            }
            continue;
        }
        published = Some(candidate);
    }

    published.ok_or_else(|| {
        invalid_handle_dispatch_contract(
            root_fqn,
            site_id,
            format!(
                "non-Unit handle arm completion payload source 从 st{} 到 st{} 缺少 completion payload source",
                arm_state.as_u32(),
                arm_complete_target.as_u32()
            ),
        )
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn handle_body_completion_payload_source(
    root_fqn: &str,
    site_id: SiteId,
    body: &Body,
    types: &TypeStore,
    state_graph: &LateLoweredStateGraph,
    state_regions: &[LateLoweredHandleStateRegionEntry],
    body_complete_target: StateId,
    result_ty: TypeId,
) -> Result<LateLoweredCompletionPayloadSource, EffectLoweringError> {
    if matches!(
        types.kind(result_ty),
        TypeKind::Value(ValueTypeKind::Unit | ValueTypeKind::Nothing)
    ) {
        return Ok(LateLoweredCompletionPayloadSource::unit(result_ty));
    }

    let mut published = None;
    let mut return_fallback = None;
    for entry in state_regions {
        if entry.region() != LateLoweredHandleStateRegion::Body {
            continue;
        }
        let state = state_graph.state(entry.state_id()).ok_or_else(|| {
            invalid_handle_dispatch_contract(
                root_fqn,
                site_id,
                format!(
                    "handle body completion payload source 引用了不存在的 body state st{}",
                    entry.state_id().as_u32()
                ),
            )
        })?;
        let candidate = match state.terminator() {
            LateLoweredStateTerminator::Goto { target } if *target == body_complete_target => {
                handle_completion_payload_source_from_state(
                    root_fqn,
                    site_id,
                    body,
                    types,
                    state_graph,
                    state.state_id(),
                    result_ty,
                    "handle body completion payload source",
                )?
            }
            LateLoweredStateTerminator::Return { payload_source, .. } => {
                let candidate = payload_source.clone();
                if let Some(existing) = &return_fallback {
                    if !same_completion_payload_source_ignoring_span(existing, &candidate) {
                        return Err(invalid_handle_dispatch_contract(
                            root_fqn,
                            site_id,
                            format!(
                                "handle body return fallback payload source 歧义：已发布 {:?}，又发现 {:?}",
                                existing, candidate
                            ),
                        ));
                    }
                    continue;
                }
                return_fallback = Some(candidate);
                continue;
            }
            _ => continue,
        };
        if let Some(existing) = &published {
            if !same_completion_payload_source_ignoring_span(existing, &candidate) {
                return Err(invalid_handle_dispatch_contract(
                    root_fqn,
                    site_id,
                    format!(
                        "handle body completion payload source 歧义：已发布 {:?}，又发现 {:?}",
                        existing, candidate
                    ),
                ));
            }
            continue;
        }
        published = Some(candidate);
    }

    if let Some(source) = published.or(return_fallback) {
        return Ok(source);
    }

    Err(invalid_handle_dispatch_contract(
        root_fqn,
        site_id,
        format!(
            "non-Unit handle body 缺少指向 st{} 的 completion payload source",
            body_complete_target.as_u32()
        ),
    ))
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn handle_completion_payload_source_from_state(
    root_fqn: &str,
    site_id: SiteId,
    body: &Body,
    types: &TypeStore,
    state_graph: &LateLoweredStateGraph,
    state_id: StateId,
    complete_ty: TypeId,
    context: &str,
) -> Result<LateLoweredCompletionPayloadSource, EffectLoweringError> {
    if matches!(
        types.kind(complete_ty),
        TypeKind::Value(ValueTypeKind::Unit | ValueTypeKind::Nothing)
    ) {
        return Ok(LateLoweredCompletionPayloadSource::unit(complete_ty));
    }
    let state = state_graph.state(state_id).ok_or_else(|| {
        invalid_handle_dispatch_contract(
            root_fqn,
            site_id,
            format!("{context} 引用了不存在的 state st{}", state_id.as_u32()),
        )
    })?;
    let mut skipped_type_mismatches = Vec::new();

    for slice in state.source_slices().iter().rev() {
        if slice.end_statement_index() == slice.start_statement_index() {
            continue;
        }
        let block = body
            .blocks
            .get(slice.block_id().as_u32() as usize)
            .ok_or_else(|| {
                invalid_handle_dispatch_contract(
                    root_fqn,
                    site_id,
                    format!(
                        "handle arm completion payload source 引用了不存在的 block bb{}",
                        slice.block_id().as_u32()
                    ),
                )
            })?;
        for stmt_index in (slice.start_statement_index()..slice.end_statement_index()).rev() {
            let stmt = block.stmts.get(stmt_index as usize).ok_or_else(|| {
                invalid_handle_dispatch_contract(
                    root_fqn,
                    site_id,
                    format!(
                        "{context} 引用了不存在的 bb{} stmt{}",
                        slice.block_id().as_u32(),
                        stmt_index
                    ),
                )
            })?;
            let StatementKind::Assign { target, .. } = &stmt.kind else {
                continue;
            };
            let local = body.locals.get(target.as_u32() as usize).ok_or_else(|| {
                invalid_handle_dispatch_contract(
                    root_fqn,
                    site_id,
                    format!("{context} 引用了不存在的 local{}", target.as_u32()),
                )
            })?;
            if local.ty != complete_ty && !is_any_type(types, complete_ty) {
                skipped_type_mismatches.push(format!(
                    "local{}:t{}",
                    target.as_u32(),
                    local.ty.as_u32()
                ));
                continue;
            }
            return Ok(LateLoweredCompletionPayloadSource::operand(
                LateLoweredOperandSource::new_local(*target, complete_ty, Some(stmt.span)),
            ));
        }
    }

    let skipped = if skipped_type_mismatches.is_empty() {
        String::new()
    } else {
        format!(
            "；已跳过非 completion 类型赋值 [{}]，目标 complete_ty=t{}",
            skipped_type_mismatches.join(", "),
            complete_ty.as_u32()
        )
    };
    Err(invalid_handle_dispatch_contract(
        root_fqn,
        site_id,
        format!(
            "non-Unit {context} state st{} 缺少 completion payload source{}",
            state_id.as_u32(),
            skipped
        ),
    ))
}

pub(crate) fn same_completion_payload_source_ignoring_span(
    left: &LateLoweredCompletionPayloadSource,
    right: &LateLoweredCompletionPayloadSource,
) -> bool {
    match (left, right) {
        (
            LateLoweredCompletionPayloadSource::Unit {
                complete_ty: left_ty,
            },
            LateLoweredCompletionPayloadSource::Unit {
                complete_ty: right_ty,
            },
        ) => left_ty == right_ty,
        (
            LateLoweredCompletionPayloadSource::Operand(left),
            LateLoweredCompletionPayloadSource::Operand(right),
        ) => left.source_ty() == right.source_ty() && left.value() == right.value(),
        _ => false,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn build_handle_state_region_entries(
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
pub(crate) fn build_handle_boundary_routings(
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

pub(crate) fn build_handle_pending_payload_transports(
    root_fqn: &str,
    site_id: SiteId,
    pending_completions: &[LateLoweredHandlePendingCompletion],
    outward_emissions: &[LateLoweredStepCaseEmission],
    frame_schema: &LateLoweredFrameSchema,
) -> Result<Vec<LateLoweredHandlePendingPayloadTransport>, EffectLoweringError> {
    let mut transports = Vec::new();
    let mut seen = BTreeSet::new();

    for completion in pending_completions {
        let LateLoweredHandlePendingCompletion::PropagateOutward(case_tag) = completion else {
            continue;
        };
        if !seen.insert(*completion) {
            return Err(invalid_handle_dispatch_contract(
                root_fqn,
                site_id,
                format!(
                    "重复发布 pending payload transport {:?}，无法保持 cleanup/finally payload contract 唯一",
                    completion
                ),
            ));
        }
        let emission = outward_emissions
            .iter()
            .find(|emission| emission.case_tag() == *case_tag)
            .ok_or_else(|| {
                invalid_handle_dispatch_contract(
                    root_fqn,
                    site_id,
                    format!(
                        "pending completion c{} 缺少 outward emission，无法发布 cleanup/finally payload transport",
                        case_tag.as_u32()
                    ),
                )
            })?;
        let slot = frame_schema
            .slot_for_kind(crate::effect_lowered::ir::LateLoweredFrameSlotKind::HandlePendingPayload {
                site_id,
                case_tag: *case_tag,
            })
            .ok_or_else(|| {
                invalid_handle_dispatch_contract(
                    root_fqn,
                    site_id,
                    format!(
                        "pending completion c{} 缺少 HandlePendingPayload frame slot，无法发布 cleanup/finally payload transport",
                        case_tag.as_u32()
                    ),
                )
            })?;
        if slot.ty() != emission.payload_tuple_ty() {
            return Err(invalid_handle_dispatch_contract(
                root_fqn,
                site_id,
                format!(
                    "pending completion c{} 的 payload transport frame slot fs{} 类型漂移：slot=t{}，outward emission=t{}",
                    case_tag.as_u32(),
                    slot.slot_id().as_u32(),
                    slot.ty().as_u32(),
                    emission.payload_tuple_ty().as_u32(),
                ),
            ));
        }
        transports.push(LateLoweredHandlePendingPayloadTransport::new(
            *completion,
            emission.payload_tuple_ty(),
            slot.slot_id(),
        ));
    }

    Ok(transports)
}

pub(crate) fn build_handle_pending_completion_origins(
    root_fqn: &str,
    site_id: SiteId,
    pending_completions: &[LateLoweredHandlePendingCompletion],
    boundary_routings: &[LateLoweredHandleBoundaryRouting],
) -> Result<Vec<LateLoweredHandlePendingCompletionOrigin>, EffectLoweringError> {
    let published_completions = pending_completions.iter().copied().collect::<BTreeSet<_>>();
    let mut origins = Vec::new();
    let mut seen = BTreeSet::new();

    for routing in boundary_routings {
        for case in routing.case_routings() {
            let LateLoweredHandleBoundaryCaseRoutingAction::PendingCompletion { completion } =
                case.action()
            else {
                continue;
            };
            if !published_completions.contains(&completion) {
                return Err(invalid_handle_dispatch_contract(
                    root_fqn,
                    site_id,
                    format!(
                        "boundary bd{} case c{} 引用未发布的 pending completion {:?}",
                        routing.boundary_id().as_u32(),
                        case.case_tag().as_u32(),
                        completion,
                    ),
                ));
            }
            if !matches!(
                completion,
                LateLoweredHandlePendingCompletion::PropagateOutward(_)
            ) {
                return Err(invalid_handle_dispatch_contract(
                    root_fqn,
                    site_id,
                    format!(
                        "boundary bd{} case c{} 的 pending completion {:?} 不是 outward propagation",
                        routing.boundary_id().as_u32(),
                        case.case_tag().as_u32(),
                        completion,
                    ),
                ));
            }
            let origin = LateLoweredHandlePendingCompletionOrigin::new(
                completion,
                routing.boundary_id(),
                routing.owner_state(),
                routing.resume_state(),
            );
            if seen.insert(origin) {
                origins.push(origin);
            }
        }
    }

    Ok(origins)
}

pub(crate) fn collect_handle_region_states(
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

pub(crate) fn insert_handle_state_region(
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

pub(crate) fn collect_handle_boundary_case_tags(
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
        LateLoweredBoundaryLowering::ClassCtor(lowering) => lowering
            .emitted_steps()
            .iter()
            .map(LateLoweredStepCaseEmission::case_tag)
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
pub(crate) fn route_handle_boundary_case(
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

pub(crate) fn lookup_handle_arms<'a>(
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

pub(crate) fn materialize_resume_payload_bindings(
    root_fqn: &str,
    frame_schema: &LateLoweredFrameSchema,
    boundary_map: &LateLoweredBoundaryMap,
) -> Result<Vec<LateLoweredResumePayloadBinding>, EffectLoweringError> {
    let (resume_boundaries, _) = paired_resume_boundaries(boundary_map);
    let mut bindings_by_boundary = BTreeMap::<BoundaryId, LateLoweredResumePayloadBinding>::new();
    let mut bindings_by_state = BTreeMap::<StateId, LateLoweredResumePayloadBinding>::new();

    for boundary in boundary_map.entries() {
        let Some(binding) = (match (boundary.source(), boundary.lowering()) {
            (
                LateLoweredBoundarySource::Site {
                    kind: BoundarySiteKind::Call,
                    ..
                },
                Some(LateLoweredBoundaryLowering::Call(lowering)),
            ) => Some(build_resume_payload_binding_from_result_local(
                root_fqn,
                frame_schema,
                boundary,
                "Call",
                lowering.result_local(),
            )?),
            (
                LateLoweredBoundarySource::Site {
                    kind: BoundarySiteKind::Perform,
                    ..
                },
                Some(LateLoweredBoundaryLowering::Perform(_)),
            ) => Some(build_resume_payload_binding_from_boundary_result_slot(
                root_fqn,
                frame_schema,
                boundary,
                "Perform",
            )?),
            (
                LateLoweredBoundarySource::Site {
                    kind: BoundarySiteKind::Resume,
                    ..
                },
                Some(LateLoweredBoundaryLowering::Resume(lowering)),
            ) => Some(build_resume_payload_binding_from_result_local(
                root_fqn,
                frame_schema,
                boundary,
                "Resume",
                lowering.result_local(),
            )?),
            (
                LateLoweredBoundarySource::Site {
                    kind: BoundarySiteKind::Handle,
                    ..
                },
                Some(LateLoweredBoundaryLowering::Handle(_)),
            )
            | (
                LateLoweredBoundarySource::RuntimeError { .. },
                Some(LateLoweredBoundaryLowering::RuntimeError(_)),
            ) => None,
            _ => None,
        }) else {
            continue;
        };

        insert_resume_payload_binding(
            root_fqn,
            &mut bindings_by_boundary,
            &mut bindings_by_state,
            binding,
        )?;
    }

    for boundary in boundary_map.entries() {
        let origin_site = match (boundary.source(), boundary.lowering()) {
            (
                LateLoweredBoundarySource::RuntimeError { origin_site },
                Some(LateLoweredBoundaryLowering::RuntimeError(_)),
            ) => origin_site,
            _ => continue,
        };
        let paired_resume_boundary = resume_boundaries.get(&origin_site).ok_or_else(|| {
            invalid_resume_payload_binding_contract(
                root_fqn,
                boundary.boundary_id(),
                format!(
                    "runtime-error route origin=site{} 缺少配对的 resume boundary，无法继承 resumed local/home binding",
                    origin_site.as_u32(),
                ),
            )
        })?;
        let paired_binding = bindings_by_boundary.get(paired_resume_boundary).copied().ok_or_else(
            || {
                invalid_resume_payload_binding_contract(
                    root_fqn,
                    boundary.boundary_id(),
                    format!(
                        "paired resume boundary bd{} 缺少 resumed local/home binding，无法为 runtime-error route 继承 authoritative consumer",
                        paired_resume_boundary.as_u32(),
                    ),
                )
            },
        )?;
        let binding = LateLoweredResumePayloadBinding::new(
            boundary.boundary_id(),
            boundary.resume_state(),
            paired_binding.consumer_local(),
            paired_binding.consumer_frame_slot(),
        );
        insert_resume_payload_binding(
            root_fqn,
            &mut bindings_by_boundary,
            &mut bindings_by_state,
            binding,
        )?;
    }

    Ok(boundary_map
        .entries()
        .iter()
        .filter_map(|boundary| bindings_by_boundary.get(&boundary.boundary_id()).copied())
        .collect())
}

pub(crate) fn materialize_completion_payload_bindings(
    root_fqn: &str,
    step_type: &LateLoweredStepType,
    state_graph: &LateLoweredStateGraph,
    frame_schema: &LateLoweredFrameSchema,
    types: &TypeStore,
) -> Result<Vec<LateLoweredCompletionPayloadBinding>, EffectLoweringError> {
    let mut bindings = BTreeMap::<StateId, LateLoweredCompletionPayloadBinding>::new();
    for state in state_graph.states() {
        let LateLoweredStateTerminator::Return {
            payload_source,
            complete_state,
        } = state.terminator()
        else {
            continue;
        };
        if *complete_state != state_graph.complete_state() {
            return Err(invalid_completion_payload_contract(
                root_fqn,
                format!(
                    "return state st{} 指向 st{}，但 callable complete_state 是 st{}",
                    state.state_id().as_u32(),
                    complete_state.as_u32(),
                    state_graph.complete_state().as_u32(),
                ),
            ));
        }
        validate_completion_payload_source(root_fqn, step_type, payload_source, types)?;
        let payload_frame_slot =
            completion_payload_frame_slot(root_fqn, frame_schema, payload_source)?;
        let binding = LateLoweredCompletionPayloadBinding::new(
            state.state_id(),
            *complete_state,
            payload_source.clone(),
            payload_frame_slot,
        );
        if bindings.insert(state.state_id(), binding).is_some() {
            return Err(invalid_completion_payload_contract(
                root_fqn,
                format!(
                    "return state st{} 重复发布 completion payload source",
                    state.state_id().as_u32(),
                ),
            ));
        }
    }
    Ok(bindings.into_values().collect())
}

pub(crate) fn validate_completion_payload_source(
    root_fqn: &str,
    step_type: &LateLoweredStepType,
    payload_source: &LateLoweredCompletionPayloadSource,
    types: &TypeStore,
) -> Result<(), EffectLoweringError> {
    if payload_source.source_ty() != step_type.complete_ty() {
        return Err(invalid_completion_payload_contract(
            root_fqn,
            format!(
                "payload source type t{} 与 StepSchema s{} complete_ty t{} 不一致",
                payload_source.source_ty().as_u32(),
                step_type.step_schema().as_u32(),
                step_type.complete_ty().as_u32(),
            ),
        ));
    }
    if payload_source.is_unit() && !is_unit_type(types, step_type.complete_ty()) {
        return Err(invalid_completion_payload_contract(
            root_fqn,
            format!(
                "non-Unit complete_ty t{} 不能发布 Unit completion payload source",
                step_type.complete_ty().as_u32(),
            ),
        ));
    }
    Ok(())
}

pub(crate) fn completion_payload_frame_slot(
    root_fqn: &str,
    frame_schema: &LateLoweredFrameSchema,
    payload_source: &LateLoweredCompletionPayloadSource,
) -> Result<Option<crate::effect_lowered::ir::FrameSlotId>, EffectLoweringError> {
    let Some(source) = payload_source.operand_source() else {
        return Ok(None);
    };
    let crate::effect_lowered::ir::LateLoweredOperandValueSource::Local(local) = source.value()
    else {
        return Ok(None);
    };
    let Some(slot_id) = find_frame_slot_for_local(frame_schema, *local) else {
        return Ok(None);
    };
    let slot = frame_schema
        .slots()
        .iter()
        .find(|slot| slot.slot_id() == slot_id)
        .expect("frame slot id returned by find_frame_slot_for_local should exist");
    if slot.ty() != source.source_ty() {
        return Err(invalid_completion_payload_contract(
            root_fqn,
            format!(
                "completion payload local{} 的 home slot{} 类型为 t{}，但 payload source type 为 t{}",
                local.as_u32(),
                slot.slot_id().as_u32(),
                slot.ty().as_u32(),
                source.source_ty().as_u32(),
            ),
        ));
    }
    Ok(Some(slot_id))
}

pub(crate) fn invalid_completion_payload_contract(
    root_fqn: &str,
    detail: String,
) -> EffectLoweringError {
    EffectLoweringError::InvalidCompletionPayloadContract {
        root_fqn: root_fqn.to_string(),
        detail,
    }
}
