//! Dispatch plan / boundary continuation compositions / step emissions / runtime-error helpers.

#![allow(dead_code)]

use super::*;

pub(crate) fn collect_result_locals(body: &Body) -> BoundaryResultLocals {
    let mut call_results = HashMap::new();
    for block in &body.blocks {
        for stmt in &block.stmts {
            if let StatementKind::Assign { target, value } = &stmt.kind {
                match value {
                    Rvalue::Call { site_id, .. } | Rvalue::ClassCtor { site_id, .. } => {
                        call_results.insert(*site_id, *target);
                    }
                    Rvalue::TopLevelRef(top_level)
                        if top_level.site_id.is_some() && !top_level.hidden_effects.is_pure() =>
                    {
                        call_results.insert(top_level.site_id.expect("checked above"), *target);
                    }
                    Rvalue::MemberAccess {
                        site_id: Some(site_id),
                        member,
                        ..
                    } if !member.hidden_effects.is_pure() => {
                        call_results.insert(*site_id, *target);
                    }
                    _ => {}
                }
            }
        }
    }
    BoundaryResultLocals { call_results }
}

pub(crate) fn paired_resume_boundaries(
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

pub(crate) fn clone_call_site_facts(
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

pub(crate) fn clone_class_ctor_site_facts(
    root_fqn: &str,
    body_facts: &BodyEffectFacts,
    site_id: SiteId,
) -> Result<ClassCtorSiteEffectFacts, EffectLoweringError> {
    let site = body_facts
        .site(site_id)
        .ok_or_else(|| EffectLoweringError::MissingSiteFacts {
            root_fqn: root_fqn.to_string(),
            site_id: site_id.as_u32(),
        })?;
    match site {
        SiteEffectFacts::ClassCtor(facts) => Ok(facts.clone()),
        other => Err(EffectLoweringError::UnexpectedSiteFactsKind {
            root_fqn: root_fqn.to_string(),
            site_id: site_id.as_u32(),
            expected: "ClassCtor",
            actual: site_facts_kind(other),
        }),
    }
}

pub(crate) fn clone_perform_site_facts(
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

pub(crate) fn clone_resume_site_facts(
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

pub(crate) fn clone_handle_site_facts(
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

pub(crate) fn site_facts_kind(site: &SiteEffectFacts) -> &'static str {
    match site {
        SiteEffectFacts::Call(_) => "Call",
        SiteEffectFacts::ClassCtor(_) => "ClassCtor",
        SiteEffectFacts::Perform(_) => "Perform",
        SiteEffectFacts::Resume(_) => "Resume",
        SiteEffectFacts::Handle(_) => "Handle",
    }
}

pub(crate) fn lookup_step_type<'a>(
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

pub(crate) fn build_step_dispatch_plan(
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

pub(crate) fn build_call_boundary_dispatch_plan(
    inputs: CallBoundaryDispatchInputs<'_>,
) -> Result<CallBoundaryDispatchMaterialization, EffectLoweringError> {
    let CallBoundaryDispatchInputs {
        root_fqn,
        boundary_id,
        input_step,
        output_step,
        outward_case_tags,
        continuation_object,
        target_state,
        result_local,
        result_frame_slot,
        types,
    } = inputs;
    let complete =
        LateLoweredCompleteStepDispatch::new(input_step.complete_ty(), target_state, result_local);
    let mut outward_cases = Vec::with_capacity(outward_case_tags.len());
    let mut continuation_compositions = Vec::with_capacity(outward_case_tags.len());
    let mut consumed_runtime_error_case = None;
    let caller_result_local =
        result_local.ok_or_else(
            || EffectLoweringError::InvalidResumePayloadBindingContract {
                root_fqn: root_fqn.to_string(),
                boundary_id: boundary_id.as_u32(),
                detail: "call-boundary continuation composition 缺少 caller result local"
                    .to_string(),
            },
        )?;

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
            let forwarding = LateLoweredStepCaseForwarding::new(
                input_case.case_tag(),
                input_case.concrete_op_key().clone(),
                LateLoweredStepCaseEmission::new(
                    projected_case.case_tag(),
                    projected_case.concrete_op_key().clone(),
                    projected_case.payload_tuple_ty(),
                    projected_case.continuation_contract(),
                    continuation_object,
                ),
            );
            continuation_compositions.push(LateLoweredCallBoundaryContinuationComposition::new(
                boundary_id,
                input_step.step_schema(),
                input_case.case_tag(),
                projected_case.case_tag(),
                input_case.continuation_contract(),
                projected_case.continuation_contract(),
                target_state,
                caller_result_local,
                result_frame_slot,
                input_step.complete_ty(),
            ));
            outward_cases.push(forwarding);
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
        continuation_compositions,
        consumed_runtime_error_case,
    })
}

pub(crate) fn build_boundary_continuation_compositions(
    root_fqn: &str,
    boundary_id: BoundaryId,
    input_step: &LateLoweredStepType,
    dispatch: &LateLoweredStepDispatchPlan,
    target_state: StateId,
    caller_result_local: LocalId,
    caller_result_frame_slot: Option<crate::effect_lowered::ir::FrameSlotId>,
) -> Result<Vec<LateLoweredCallBoundaryContinuationComposition>, EffectLoweringError> {
    dispatch
        .outward_cases()
        .iter()
        .map(|forwarding| {
            let input_case = input_step
                .case(forwarding.input_case_tag())
                .ok_or_else(|| EffectLoweringError::MissingInputStepCase {
                    root_fqn: root_fqn.to_string(),
                    step_schema: input_step.step_schema().as_u32(),
                    case_tag: forwarding.input_case_tag().as_u32(),
                })?;
            Ok(LateLoweredCallBoundaryContinuationComposition::new(
                boundary_id,
                input_step.step_schema(),
                input_case.case_tag(),
                forwarding.emission().case_tag(),
                input_case.continuation_contract(),
                forwarding.emission().continuation_contract(),
                target_state,
                caller_result_local,
                caller_result_frame_slot,
                input_step.complete_ty(),
            ))
        })
        .collect()
}

pub(crate) fn local_runtime_error_terminal_action() -> LateLoweredLocalRuntimeErrorTerminalAction {
    LateLoweredLocalRuntimeErrorTerminalAction::RuntimeFatal {
        runtime_entry: LateLoweredPublishedRuntimeEntry::RuntimeErrorFatal,
    }
}

pub(crate) fn build_current_step_emission(
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

pub(crate) fn build_emission_from_concrete_op(
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

pub(crate) fn build_handle_outward_emissions(
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

pub(crate) fn resume_runtime_error_effect_family(
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

pub(crate) fn find_resume_metadata(body: &Body, site_id: SiteId) -> Option<&ResumeMetadata> {
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

pub(crate) fn effect_family_for_effect_ty(
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

pub(crate) fn is_runtime_error_raise_case(case: &LateLoweredStepCase, types: &TypeStore) -> bool {
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
