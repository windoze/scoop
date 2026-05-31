//! Resume payload bindings + step-type / resume-interface / continuation-contract construction + source compatibility helpers.

#![allow(dead_code)]

use super::*;

pub(crate) fn is_unit_type(types: &TypeStore, ty: TypeId) -> bool {
    matches!(types.kind(ty), TypeKind::Value(ValueTypeKind::Unit))
}

pub(crate) fn is_any_type(types: &TypeStore, ty: TypeId) -> bool {
    matches!(types.kind(ty), TypeKind::Ref(RefTypeKind::Any))
}

pub(crate) fn build_resume_payload_binding_from_result_local(
    root_fqn: &str,
    frame_schema: &LateLoweredFrameSchema,
    boundary: &LateLoweredBoundary,
    kind: &'static str,
    result_local: LocalId,
) -> Result<LateLoweredResumePayloadBinding, EffectLoweringError> {
    let boundary_result_slot = published_boundary_result_slot(frame_schema, boundary.boundary_id());
    if let Some((slot_local, _)) = boundary_result_slot
        && slot_local != result_local
    {
        return Err(invalid_resume_payload_binding_contract(
            root_fqn,
            boundary.boundary_id(),
            format!(
                "{kind} boundary 的 BoundaryResult slot 绑定到了 local{}，但 published result local 为 local{}",
                slot_local.as_u32(),
                result_local.as_u32(),
            ),
        ));
    }
    let consumer_frame_slot = boundary_result_slot
        .map(|(_, slot_id)| slot_id)
        .or_else(|| find_frame_slot_for_local(frame_schema, result_local));
    Ok(LateLoweredResumePayloadBinding::new(
        boundary.boundary_id(),
        boundary.resume_state(),
        result_local,
        consumer_frame_slot,
    ))
}

pub(crate) fn build_resume_payload_binding_from_boundary_result_slot(
    root_fqn: &str,
    frame_schema: &LateLoweredFrameSchema,
    boundary: &LateLoweredBoundary,
    kind: &'static str,
) -> Result<LateLoweredResumePayloadBinding, EffectLoweringError> {
    let Some((consumer_local, consumer_frame_slot)) =
        published_boundary_result_slot(frame_schema, boundary.boundary_id())
    else {
        return Err(invalid_resume_payload_binding_contract(
            root_fqn,
            boundary.boundary_id(),
            format!(
                "{kind} boundary 缺少 BoundaryResult frame slot，无法 authoritative 发布 resumed local/home",
            ),
        ));
    };
    Ok(LateLoweredResumePayloadBinding::new(
        boundary.boundary_id(),
        boundary.resume_state(),
        consumer_local,
        Some(consumer_frame_slot),
    ))
}

pub(crate) fn insert_resume_payload_binding(
    root_fqn: &str,
    bindings_by_boundary: &mut BTreeMap<BoundaryId, LateLoweredResumePayloadBinding>,
    bindings_by_state: &mut BTreeMap<StateId, LateLoweredResumePayloadBinding>,
    binding: LateLoweredResumePayloadBinding,
) -> Result<(), EffectLoweringError> {
    if bindings_by_boundary
        .insert(binding.boundary_id(), binding)
        .is_some()
    {
        return Err(invalid_resume_payload_binding_contract(
            root_fqn,
            binding.boundary_id(),
            "重复发布多个 resumed local/home binding".to_string(),
        ));
    }
    match bindings_by_state.get(&binding.resume_state()) {
        Some(existing)
            if existing.consumer_local() == binding.consumer_local()
                && existing.consumer_frame_slot() == binding.consumer_frame_slot() => {}
        Some(existing) => {
            return Err(invalid_resume_payload_binding_contract(
                root_fqn,
                binding.boundary_id(),
                format!(
                    "resume state st{} 同时映射到不兼容的 resumed local/home：已发布 {}，当前尝试发布 {}",
                    binding.resume_state().as_u32(),
                    render_resume_payload_binding_target(
                        existing.consumer_local(),
                        existing.consumer_frame_slot(),
                    ),
                    render_resume_payload_binding_target(
                        binding.consumer_local(),
                        binding.consumer_frame_slot(),
                    ),
                ),
            ));
        }
        None => {
            bindings_by_state.insert(binding.resume_state(), binding);
        }
    }
    Ok(())
}

pub(crate) fn published_boundary_result_slot(
    frame_schema: &LateLoweredFrameSchema,
    boundary_id: BoundaryId,
) -> Option<(LocalId, crate::effect_lowered::ir::FrameSlotId)> {
    frame_schema
        .slots()
        .iter()
        .find_map(|slot| match slot.kind() {
            LateLoweredFrameSlotKind::BoundaryResult { boundary, local }
                if boundary == boundary_id =>
            {
                Some((local, slot.slot_id()))
            }
            _ => None,
        })
}

pub(crate) fn render_resume_payload_binding_target(
    consumer_local: LocalId,
    consumer_frame_slot: Option<crate::effect_lowered::ir::FrameSlotId>,
) -> String {
    match consumer_frame_slot {
        Some(slot_id) => format!(
            "local{} / slot{}",
            consumer_local.as_u32(),
            slot_id.as_u32()
        ),
        None => format!("local{} / <no-frame-slot>", consumer_local.as_u32()),
    }
}

pub(crate) fn invalid_resume_payload_binding_contract(
    root_fqn: &str,
    boundary_id: BoundaryId,
    detail: String,
) -> EffectLoweringError {
    EffectLoweringError::InvalidResumePayloadBindingContract {
        root_fqn: root_fqn.to_string(),
        boundary_id: boundary_id.as_u32(),
        detail,
    }
}

pub(crate) fn find_frame_slot_for_local(
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
            crate::effect_lowered::ir::LateLoweredFrameSlotKind::HandleSavedEffectCtx {
                ..
            }
            | crate::effect_lowered::ir::LateLoweredFrameSlotKind::HandleArmEffectCtx { .. }
            | crate::effect_lowered::ir::LateLoweredFrameSlotKind::HandlePendingPayload {
                ..
            }
            | crate::effect_lowered::ir::LateLoweredFrameSlotKind::ResumePayload { .. }
            | crate::effect_lowered::ir::LateLoweredFrameSlotKind::System(_) => None,
        };
        (slot_local == Some(local)).then_some(slot.slot_id())
    })
}

pub(crate) fn find_handle_boundary_lowering<'a>(
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

pub(crate) fn collect_handle_outward_case_tags(facts: &HandleSiteEffectFacts) -> BTreeSet<CaseTag> {
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

pub(crate) fn invalid_handle_dispatch_contract(
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

pub(crate) fn format_case_tag_set(tags: &BTreeSet<CaseTag>) -> String {
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

pub(crate) fn build_step_type(
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

pub(crate) fn build_resume_interface(
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

pub(crate) fn build_continuation_contract(
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

pub(crate) fn group_cases_by_effect_family(
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

pub(crate) fn continuation_resume_body(
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
pub(crate) struct BoundaryResultLocals {
    pub(crate) call_results: HashMap<SiteId, LocalId>,
}

pub(crate) fn invalid_boundary_operand_contract(
    root_fqn: &str,
    site_id: SiteId,
    kind: &'static str,
    detail: impl Into<String>,
) -> EffectLoweringError {
    EffectLoweringError::InvalidBoundaryOperandContract {
        root_fqn: root_fqn.to_string(),
        site_id: site_id.as_u32(),
        kind,
        detail: detail.into(),
    }
}

pub(crate) fn expected_source_types_for_carrier(
    types: &TypeStore,
    carrier_ty: crate::ty::TypeId,
    source_count: usize,
) -> Result<Vec<crate::ty::TypeId>, String> {
    match source_count {
        0 => match types.kind(carrier_ty) {
            TypeKind::Value(ValueTypeKind::Unit) => Ok(Vec::new()),
            _ => Err(format!(
                "只有 Unit carrier 才允许 0 个 source，但 published carrier 为 t{}",
                carrier_ty.as_u32(),
            )),
        },
        1 => Ok(vec![carrier_ty]),
        _ => match types.kind(carrier_ty) {
            TypeKind::Value(ValueTypeKind::Tuple(elements)) if elements.len() == source_count => {
                Ok(elements.clone())
            }
            TypeKind::Value(ValueTypeKind::Tuple(elements)) => Err(format!(
                "published tuple carrier t{} 期望 {} 个 source，实际为 {source_count}",
                carrier_ty.as_u32(),
                elements.len(),
            )),
            _ => Err(format!(
                "published carrier t{} 期望单一 source，实际数量为 {source_count}",
                carrier_ty.as_u32(),
            )),
        },
    }
}

pub(crate) fn call_kind_matches_facts(kind: &CallKind, facts: &CallSiteEffectFacts) -> bool {
    matches!(
        (kind, facts.kind()),
        (
            CallKind::Direct { .. },
            crate::effect_facts::CallSiteKind::Direct
        ) | (
            CallKind::Closure { .. },
            crate::effect_facts::CallSiteKind::Closure
        ) | (
            CallKind::FunValue { .. },
            crate::effect_facts::CallSiteKind::FunValue
        ) | (
            CallKind::FunPtr { .. },
            crate::effect_facts::CallSiteKind::FunPtr
        ) | (
            CallKind::Virtual { .. },
            crate::effect_facts::CallSiteKind::Virtual
        ) | (
            CallKind::Interface { .. },
            crate::effect_facts::CallSiteKind::Interface
        )
    )
}

pub(crate) fn local_decl_ty(
    root_fqn: &str,
    site_id: SiteId,
    kind: &'static str,
    body: &Body,
    local: LocalId,
) -> Result<crate::ty::TypeId, EffectLoweringError> {
    body.locals
        .get(local.as_u32() as usize)
        .map(|decl| decl.ty)
        .ok_or_else(|| {
            invalid_boundary_operand_contract(
                root_fqn,
                site_id,
                kind,
                format!("operand 引用了缺失的 local{}", local.as_u32()),
            )
        })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn operand_source_with_expected_ty(
    root_fqn: &str,
    site_id: SiteId,
    kind: &'static str,
    body: &Body,
    types: &TypeStore,
    nominal_direct_supertypes: &NominalDirectSupertypeIndex,
    operand: &Operand,
    expected_ty: crate::ty::TypeId,
    span: Option<crate::span::Span>,
) -> Result<LateLoweredOperandSource, EffectLoweringError> {
    match operand {
        Operand::Local(local) => {
            let local_ty = local_decl_ty(root_fqn, site_id, kind, body, *local)?;
            if local_ty != expected_ty
                && !local_defines_static_member_value_of_type(body, types, *local, expected_ty)
                && !function_value_source_type_compatible(types, local_ty, expected_ty)
                && !nominal_source_type_compatible(
                    types,
                    local_ty,
                    expected_ty,
                    nominal_direct_supertypes,
                )
            {
                return Err(invalid_boundary_operand_contract(
                    root_fqn,
                    site_id,
                    kind,
                    format!(
                        "local{} 的类型为 t{}({})，但 published operand contract 期望 t{}({})",
                        local.as_u32(),
                        local_ty.as_u32(),
                        types.display(local_ty),
                        expected_ty.as_u32(),
                        types.display(expected_ty),
                    ),
                ));
            }
            Ok(LateLoweredOperandSource::new_local(
                *local,
                expected_ty,
                span,
            ))
        }
        Operand::Const(value) => Ok(LateLoweredOperandSource::new_const(
            value.clone(),
            expected_ty,
            span,
        )),
    }
}

pub(crate) fn nominal_source_type_compatible(
    types: &TypeStore,
    local_ty: TypeId,
    expected_ty: TypeId,
    nominal_direct_supertypes: &NominalDirectSupertypeIndex,
) -> bool {
    if builtin_string_source_type_compatible(types, local_ty, expected_ty) {
        return true;
    }
    if matches!(types.kind(expected_ty), TypeKind::Ref(RefTypeKind::Any)) {
        return matches!(types.kind(local_ty), TypeKind::Ref(_));
    }
    let (local_nominal, expected_nominal) = match (types.kind(local_ty), types.kind(expected_ty)) {
        (
            TypeKind::Ref(RefTypeKind::Nominal(local_nominal)),
            TypeKind::Ref(RefTypeKind::Nominal(expected_nominal)),
        )
        | (
            TypeKind::Value(ValueTypeKind::Nominal(local_nominal)),
            TypeKind::Value(ValueTypeKind::Nominal(expected_nominal)),
        ) => (local_nominal, expected_nominal),
        _ => return false,
    };
    if local_nominal == expected_nominal {
        return true;
    }
    if !local_nominal.args.is_empty()
        || local_nominal.eff.is_some()
        || !expected_nominal.args.is_empty()
        || expected_nominal.eff.is_some()
    {
        return false;
    }

    let mut stack = vec![local_nominal.fqn.clone()];
    let mut seen = HashSet::new();
    while let Some(current) = stack.pop() {
        if !seen.insert(current.clone()) {
            continue;
        }
        if current == expected_nominal.fqn {
            return true;
        }
        if let Some(supers) = nominal_direct_supertypes.get(&current) {
            stack.extend(supers.iter().cloned());
        }
    }
    false
}

pub(crate) fn builtin_string_source_type_compatible(
    types: &TypeStore,
    local_ty: TypeId,
    expected_ty: TypeId,
) -> bool {
    pub(crate) fn is_string(types: &TypeStore, ty: TypeId) -> bool {
        matches!(types.kind(ty), TypeKind::Ref(RefTypeKind::String))
            || matches!(
                types.kind(ty),
                TypeKind::Ref(RefTypeKind::Nominal(nominal)) if nominal.fqn == "scoop.core.String"
            )
    }

    is_string(types, local_ty) && is_string(types, expected_ty)
}

pub(crate) fn function_value_source_type_compatible(
    types: &TypeStore,
    local_ty: TypeId,
    expected_ty: TypeId,
) -> bool {
    let (
        TypeKind::Ref(RefTypeKind::Function(local_fun)),
        TypeKind::Ref(RefTypeKind::Function(expected_fun)),
    ) = (types.kind(local_ty), types.kind(expected_ty))
    else {
        return false;
    };
    local_fun.receiver == expected_fun.receiver
        && local_fun.params == expected_fun.params
        && local_fun.effects == expected_fun.effects
        && local_fun.effects_closed == expected_fun.effects_closed
        && (local_fun.return_ty == expected_fun.return_ty
            || matches!(
                types.kind(local_fun.return_ty),
                TypeKind::Value(ValueTypeKind::Nothing)
            ))
}

pub(crate) fn local_defines_static_member_value_of_type(
    body: &Body,
    types: &TypeStore,
    local: LocalId,
    expected_ty: TypeId,
) -> bool {
    let expected_fqn = match types.kind(expected_ty) {
        TypeKind::Ref(RefTypeKind::Nominal(nominal))
        | TypeKind::Value(ValueTypeKind::Nominal(nominal)) => nominal.fqn.as_str(),
        _ => return false,
    };
    body.blocks
        .iter()
        .flat_map(|block| &block.stmts)
        .any(|stmt| {
            let StatementKind::Assign {
                target,
                value: Rvalue::MemberAccess { member, .. },
            } = &stmt.kind
            else {
                return false;
            };
            if *target != local {
                return false;
            }
            let Some(MemberTarget::Value { fqn }) = member.resolved.as_ref() else {
                return false;
            };
            fqn.strip_prefix(expected_fqn)
                .is_some_and(|suffix| suffix.starts_with('.'))
        })
}

pub(crate) fn operand_source_with_inferred_ty(
    root_fqn: &str,
    site_id: SiteId,
    kind: &'static str,
    body: &Body,
    operand: &Operand,
    span: Option<crate::span::Span>,
) -> Result<LateLoweredOperandSource, EffectLoweringError> {
    match operand {
        Operand::Local(local) => Ok(LateLoweredOperandSource::new_local(
            *local,
            local_decl_ty(root_fqn, site_id, kind, body, *local)?,
            span,
        )),
        Operand::Const(_) => Err(invalid_boundary_operand_contract(
            root_fqn,
            site_id,
            kind,
            "当前 boundary contract 无法为 carrier/continuation 常量来源恢复稳定 source_ty",
        )),
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn build_ordered_call_arg_sources(
    root_fqn: &str,
    site_id: SiteId,
    kind: &'static str,
    body: &Body,
    args: &[CallArg],
    expected_tuple_ty: crate::ty::TypeId,
    types: &TypeStore,
    nominal_direct_supertypes: &NominalDirectSupertypeIndex,
) -> Result<Vec<LateLoweredOperandSource>, EffectLoweringError> {
    let expected_components =
        expected_source_types_for_carrier(types, expected_tuple_ty, args.len())
            .map_err(|detail| invalid_boundary_operand_contract(root_fqn, site_id, kind, detail))?;
    if args.len() != expected_components.len() {
        return Err(invalid_boundary_operand_contract(
            root_fqn,
            site_id,
            kind,
            format!(
                "ordered args 数量({}) 与 published carrier t{} 的 component 数量({}) 不一致",
                args.len(),
                expected_tuple_ty.as_u32(),
                expected_components.len(),
            ),
        ));
    }
    args.iter()
        .zip(expected_components)
        .map(|(arg, expected_ty)| {
            operand_source_with_expected_ty(
                root_fqn,
                site_id,
                kind,
                body,
                types,
                nominal_direct_supertypes,
                &arg.value,
                expected_ty,
                Some(arg.span),
            )
        })
        .collect()
}

pub(crate) fn expected_source_components_for_carrier(
    types: &TypeStore,
    carrier_ty: TypeId,
) -> Vec<TypeId> {
    match types.kind(carrier_ty) {
        TypeKind::Value(ValueTypeKind::Unit) => Vec::new(),
        TypeKind::Value(ValueTypeKind::Tuple(elements)) => elements.clone(),
        _ => vec![carrier_ty],
    }
}

pub(crate) fn local_assignment(body: &Body, local: LocalId) -> Option<&Rvalue> {
    body.blocks
        .iter()
        .flat_map(|block| block.stmts.iter())
        .find_map(|stmt| match &stmt.kind {
            StatementKind::Assign { target, value } if *target == local => Some(value),
            _ => None,
        })
}

pub(crate) fn resolve_closure_env_operand<'a>(
    body: &'a Body,
    callee: &Operand,
) -> Option<&'a Operand> {
    let &Operand::Local(mut current) = callee else {
        return None;
    };
    for _ in 0..32 {
        match local_assignment(body, current)? {
            Rvalue::MakeClosure { env, .. } => return Some(env),
            Rvalue::Use(Operand::Local(next)) => current = *next,
            Rvalue::Transport {
                value: Operand::Local(next),
                ..
            } => current = *next,
            _ => return None,
        }
    }
    None
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn build_known_instance_closure_call_arg_sources(
    root_fqn: &str,
    site_id: SiteId,
    kind: &'static str,
    body: &Body,
    types: &TypeStore,
    nominal_direct_supertypes: &NominalDirectSupertypeIndex,
    callee: &Operand,
    args: &[CallArg],
    expected_tuple_ty: TypeId,
) -> Result<Option<Vec<LateLoweredOperandSource>>, EffectLoweringError> {
    let expected_components = expected_source_components_for_carrier(types, expected_tuple_ty);
    let Some(env_operand) = resolve_closure_env_operand(body, callee) else {
        return Ok(None);
    };
    if args.is_empty()
        && let Ok(source) = operand_source_with_expected_ty(
            root_fqn,
            site_id,
            kind,
            body,
            types,
            nominal_direct_supertypes,
            env_operand,
            expected_tuple_ty,
            None,
        )
    {
        return Ok(Some(vec![source]));
    }
    let decompose_env_tuple = matches!(
        types.kind(expected_tuple_ty),
        TypeKind::Value(ValueTypeKind::Tuple(_))
    );
    let env_sources = match env_operand {
        Operand::Local(local) => match local_assignment(body, *local) {
            Some(Rvalue::MakeTuple { elements, .. }) if decompose_env_tuple => {
                if elements.len() > expected_components.len() {
                    return Err(invalid_boundary_operand_contract(
                        root_fqn,
                        site_id,
                        kind,
                        format!(
                            "closure env component 数量({}) 超过 published invoke carrier t{} 的 component 数量({})",
                            elements.len(),
                            expected_tuple_ty.as_u32(),
                            expected_components.len(),
                        ),
                    ));
                }
                elements
                    .iter()
                    .zip(expected_components.iter().copied())
                    .map(|(element, expected_ty)| {
                        operand_source_with_expected_ty(
                            root_fqn,
                            site_id,
                            kind,
                            body,
                            types,
                            nominal_direct_supertypes,
                            element,
                            expected_ty,
                            None,
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?
            }
            Some(Rvalue::Use(source)) if expected_components.len() == 1 => {
                vec![operand_source_with_expected_ty(
                    root_fqn,
                    site_id,
                    kind,
                    body,
                    types,
                    nominal_direct_supertypes,
                    source,
                    expected_components[0],
                    None,
                )?]
            }
            _ if expected_components.len() == 1 => vec![operand_source_with_expected_ty(
                root_fqn,
                site_id,
                kind,
                body,
                types,
                nominal_direct_supertypes,
                env_operand,
                expected_components[0],
                None,
            )?],
            _ if expected_components.is_empty() => Vec::new(),
            _ => return Ok(None),
        },
        Operand::Const(_) if expected_components.is_empty() => Vec::new(),
        Operand::Const(_) => return Ok(None),
    };
    let explicit_components = &expected_components[env_sources.len()..];
    if args.len() != explicit_components.len() {
        return Err(invalid_boundary_operand_contract(
            root_fqn,
            site_id,
            kind,
            format!(
                "closure env component 数量({}) + ordered args 数量({}) 与 published carrier t{} 的 component 数量({}) 不一致",
                env_sources.len(),
                args.len(),
                expected_tuple_ty.as_u32(),
                expected_components.len(),
            ),
        ));
    }
    let mut sources = env_sources;
    for (arg, expected_ty) in args.iter().zip(explicit_components.iter().copied()) {
        sources.push(operand_source_with_expected_ty(
            root_fqn,
            site_id,
            kind,
            body,
            types,
            nominal_direct_supertypes,
            &arg.value,
            expected_ty,
            Some(arg.span),
        )?);
    }
    Ok(Some(sources))
}

pub(crate) fn build_ordered_perform_payload_sources(
    root_fqn: &str,
    site_id: SiteId,
    body: &Body,
    args: &[PerformArg],
    payload_tuple_ty: crate::ty::TypeId,
    types: &TypeStore,
    nominal_direct_supertypes: &NominalDirectSupertypeIndex,
) -> Result<Vec<LateLoweredOperandSource>, EffectLoweringError> {
    let expected_components =
        expected_source_types_for_carrier(types, payload_tuple_ty, args.len()).map_err(
            |detail| invalid_boundary_operand_contract(root_fqn, site_id, "Perform", detail),
        )?;
    if args.len() != expected_components.len() {
        return Err(invalid_boundary_operand_contract(
            root_fqn,
            site_id,
            "Perform",
            format!(
                "payload source 数量({}) 与 published payload tuple t{} 的 component 数量({}) 不一致",
                args.len(),
                payload_tuple_ty.as_u32(),
                expected_components.len(),
            ),
        ));
    }
    args.iter()
        .zip(expected_components)
        .map(|(arg, expected_ty)| {
            operand_source_with_expected_ty(
                root_fqn,
                site_id,
                "Perform",
                body,
                types,
                nominal_direct_supertypes,
                &arg.value,
                expected_ty,
                Some(arg.span),
            )
        })
        .collect()
}

pub(crate) fn validate_source_slice_bounds(
    root_fqn: &str,
    site_id: SiteId,
    kind: &'static str,
    body: &Body,
    source_slice: LateLoweredStateSlice,
) -> Result<(), EffectLoweringError> {
    let block = body
        .blocks
        .get(source_slice.block_id().as_u32() as usize)
        .ok_or_else(|| {
            invalid_boundary_operand_contract(
                root_fqn,
                site_id,
                kind,
                format!(
                    "source slice 指向缺失的 canonical MIR block bb{}",
                    source_slice.block_id().as_u32(),
                ),
            )
        })?;
    let start = source_slice.start_statement_index() as usize;
    let end = source_slice.end_statement_index() as usize;
    if start > end || end > block.stmts.len() {
        return Err(invalid_boundary_operand_contract(
            root_fqn,
            site_id,
            kind,
            format!(
                "source slice [{}..{}) 越界于 canonical MIR block bb{}（stmt_count={}）",
                source_slice.start_statement_index(),
                source_slice.end_statement_index(),
                source_slice.block_id().as_u32(),
                block.stmts.len(),
            ),
        ));
    }
    Ok(())
}
