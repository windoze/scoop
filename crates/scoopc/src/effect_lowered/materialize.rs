use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::effect_facts::{
    BodyEffectFacts, CallSiteEffectFacts, ConcreteOpKey, EffectFamilyKey, HandleSiteEffectFacts,
    ImplPlan, MaterializedEffectFacts, PerformSiteEffectFacts, ResumeSiteEffectFacts,
    SiteEffectFacts, StepCaseFact, StepSchema, StepSchemaId,
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
    LateLoweredDynamicInvokeEntry, LateLoweredHandleBoundaryLowering, LateLoweredOneShotPolicy,
    LateLoweredPerformBoundaryLowering, LateLoweredResumeBoundaryLowering,
    LateLoweredResumeInterface, LateLoweredResumeMethod, LateLoweredRuntimeErrorBoundaryLowering,
    LateLoweredState, LateLoweredStateGraph, LateLoweredStateRole, LateLoweredStateTerminator,
    LateLoweredStepCase, LateLoweredStepCaseEmission, LateLoweredStepCaseForwarding,
    LateLoweredStepDispatchPlan, LateLoweredStepType, ResumeInterfaceId, StateId,
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
}

struct LocalRuntimeErrorStateTarget {
    boundary_id: BoundaryId,
    owner_state: StateId,
    target_state: StateId,
    payload_tuple_ty: crate::ty::TypeId,
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
                        });
                        LateLoweredConsumedRuntimeErrorCase::new(
                            pending.input_case_tag,
                            pending.input_concrete_op_key,
                            pending.payload_tuple_ty,
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

    Ok(BoundaryMaterialization {
        state_graph: attach_local_runtime_error_states(
            root_fqn,
            state_graph,
            &local_runtime_error_targets,
        )?,
        boundary_map: LateLoweredBoundaryMap::new(entries),
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
            Ok(LateLoweredState::new(
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

    use crate::effect_facts::{CallTargetMode, ImplPlan, NestedHandleClassification};
    use crate::effect_lowered::LateLoweredProgramBuilder;
    use crate::effect_lowered::ir::{
        BoundarySiteKind, LateLoweredBoundaryLowering, LateLoweredContinuationMethodReachability,
        LateLoweredContinuationResumeBody, LateLoweredOneShotPolicy,
    };
    use crate::effect_refactor_pipeline::load_effect_facts_stage_output_for_dump;
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
                } if *payload_tuple_ty == runtime_error_case.payload_tuple_ty()
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
