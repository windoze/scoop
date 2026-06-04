//! LIR statement classification (resume payload / completion / boundary result injection).

#![allow(dead_code)]

use super::*;

pub(crate) fn materialize_source_statement_classifications(
    root_fqn: &str,
    body: &Body,
    state_graph: &LateLoweredStateGraph,
    frame_schema: &LateLoweredFrameSchema,
    boundary_map: &LateLoweredBoundaryMap,
) -> Result<Vec<LateLoweredSourceStatementClassification>, EffectLoweringError> {
    let boundary_statement_anchors = collect_boundary_statement_anchors(root_fqn, boundary_map)?;
    let handle_binder_locals = collect_handle_binder_locals(state_graph);
    let mut classifications = Vec::new();
    let mut seen_statements = BTreeSet::<LirBodyAnchor>::new();
    let mut matched_boundary_statement_anchors = BTreeSet::<BoundaryId>::new();

    for state in state_graph.states() {
        for (stmt_index, stmt) in state.statements().iter().enumerate() {
            let statement_index = LirStatementIndex::new(stmt_index as u32);
            let anchor = LirBodyAnchor::statement(state.state_id(), statement_index);
            if !seen_statements.insert(anchor) {
                return Err(invalid_source_slice_classification_contract(
                    root_fqn,
                    format!(
                        "state st{} statement{} 被重复分类，classification contract 不再唯一",
                        state.state_id().as_u32(),
                        stmt_index,
                    ),
                ));
            }
            let kind = classify_source_statement(
                state.state_id(),
                body,
                anchor,
                stmt,
                frame_schema,
                &boundary_statement_anchors,
                &handle_binder_locals,
                &mut matched_boundary_statement_anchors,
            );
            if let LateLoweredSourceStatementClassificationKind::Unsupported { reason } = kind {
                return Err(invalid_source_slice_classification_contract(
                    root_fqn,
                    format!(
                        "state st{} statement{} has unsupported LIR classification: {reason}",
                        state.state_id().as_u32(),
                        stmt_index,
                    ),
                ));
            }
            classifications.push(LateLoweredSourceStatementClassification::new(anchor, kind));
        }
    }

    for (anchor, boundary_id) in &boundary_statement_anchors {
        if !matched_boundary_statement_anchors.contains(boundary_id) {
            return Err(invalid_source_slice_classification_contract(
                root_fqn,
                format!(
                    "boundary bd{} 的 statement anchor {:?} 未落入任何 LIR statement classification",
                    boundary_id.as_u32(),
                    anchor,
                ),
            ));
        }
    }
    Ok(classifications)
}

pub(crate) fn collect_boundary_statement_anchors(
    root_fqn: &str,
    boundary_map: &LateLoweredBoundaryMap,
) -> Result<BTreeMap<LirBodyAnchor, BoundaryId>, EffectLoweringError> {
    let mut anchors = BTreeMap::new();
    for boundary in boundary_map.entries() {
        let Some(LateLoweredBoundarySourceConsumption::Statement {
            source_slice,
            statement_index,
            ..
        }) = boundary_source_consumption(boundary)
        else {
            continue;
        };
        let anchor = LirBodyAnchor::statement(
            StateId::new(source_slice.block_id().as_u32()),
            LirStatementIndex::new(
                statement_index.saturating_sub(source_slice.start_statement_index()),
            ),
        );
        if let Some(existing) = anchors.insert(anchor, boundary.boundary_id()) {
            return Err(invalid_source_slice_classification_contract(
                root_fqn,
                format!(
                    "LIR anchor {:?} 同时被 boundary bd{} 与 bd{} 声明为 consumed anchor",
                    anchor,
                    existing.as_u32(),
                    boundary.boundary_id().as_u32(),
                ),
            ));
        }
    }
    Ok(anchors)
}

pub(crate) fn boundary_source_consumption(
    boundary: &LateLoweredBoundary,
) -> Option<LateLoweredBoundarySourceConsumption> {
    match boundary.lowering()? {
        LateLoweredBoundaryLowering::Call(lowering) => {
            Some(lowering.operand_contract().source_consumption())
        }
        LateLoweredBoundaryLowering::ClassCtor(lowering) => Some(lowering.source_consumption()),
        LateLoweredBoundaryLowering::Perform(lowering) => {
            Some(lowering.operand_contract().source_consumption())
        }
        LateLoweredBoundaryLowering::Resume(lowering) => {
            Some(lowering.operand_contract().source_consumption())
        }
        LateLoweredBoundaryLowering::RuntimeError(_) | LateLoweredBoundaryLowering::Handle(_) => {
            None
        }
    }
}

pub(crate) fn collect_handle_binder_locals(
    state_graph: &LateLoweredStateGraph,
) -> BTreeMap<(StateId, LocalId), SiteId> {
    let mut locals = BTreeMap::new();
    for state in state_graph.states() {
        let LateLoweredStateTerminator::HandleDispatch {
            site_id, contract, ..
        } = state.terminator()
        else {
            continue;
        };
        for arm in contract.handled_arms() {
            for binder in arm.payload_binders() {
                locals.insert((arm.arm_state(), binder.local()), *site_id);
            }
            if let Some(binder) = arm.continuation_binder() {
                locals.insert((arm.arm_state(), binder.local()), *site_id);
            }
        }
    }
    locals
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn classify_source_statement(
    state_id: StateId,
    body: &Body,
    anchor: LirBodyAnchor,
    stmt: &LirStatement,
    frame_schema: &LateLoweredFrameSchema,
    boundary_statement_anchors: &BTreeMap<LirBodyAnchor, BoundaryId>,
    handle_binder_locals: &BTreeMap<(StateId, LocalId), SiteId>,
    matched_boundary_statement_anchors: &mut BTreeSet<BoundaryId>,
) -> LateLoweredSourceStatementClassificationKind {
    if let Some(boundary_id) = boundary_statement_anchors.get(&anchor).copied() {
        matched_boundary_statement_anchors.insert(boundary_id);
        return LateLoweredSourceStatementClassificationKind::BoundaryConsumedAnchor {
            boundary_id,
        };
    }

    if let Some(binding) = resume_payload_injection_binding(frame_schema, stmt) {
        return LateLoweredSourceStatementClassificationKind::ResumePayloadInjection {
            boundary_id: binding.boundary_id(),
            resume_state: binding.resume_state(),
            consumer_local: binding.consumer_local(),
        };
    }

    if let Some(site_id) = handle_binder_statement(handle_binder_locals, state_id, stmt) {
        return LateLoweredSourceStatementClassificationKind::HandleSyntheticCarrierBinder {
            site_id,
            state_id,
        };
    }

    if let Some(binding) = boundary_result_injection_binding(frame_schema, state_id, stmt) {
        return LateLoweredSourceStatementClassificationKind::BoundaryResultInjection {
            boundary_id: binding.boundary_id(),
            resume_state: binding.resume_state(),
            result_local: binding.consumer_local(),
        };
    }

    if let Some(binding) = completion_payload_injection_binding(frame_schema, state_id, stmt) {
        return LateLoweredSourceStatementClassificationKind::CompletionPayloadInjection {
            return_state: binding.return_state(),
            complete_state: binding.complete_state(),
        };
    }

    classify_effect_neutral_source_statement(body, stmt)
}

pub(crate) fn resume_payload_injection_binding(
    frame_schema: &LateLoweredFrameSchema,
    stmt: &LirStatement,
) -> Option<LateLoweredResumePayloadBinding> {
    let LirStatementKind::Assign {
        target,
        value: LirRvalue::PerformResult { .. },
    } = &stmt.kind
    else {
        return None;
    };
    let mut matches = frame_schema
        .resume_payload_bindings()
        .iter()
        .copied()
        .filter(|binding| binding.consumer_local() == *target);
    let binding = matches.next()?;
    matches.next().is_none().then_some(binding)
}

pub(crate) fn boundary_result_injection_binding(
    frame_schema: &LateLoweredFrameSchema,
    state_id: StateId,
    stmt: &LirStatement,
) -> Option<LateLoweredResumePayloadBinding> {
    let LirStatementKind::Assign { target, .. } = &stmt.kind else {
        return None;
    };
    frame_schema
        .resume_payload_bindings()
        .iter()
        .copied()
        .find(|binding| binding.resume_state() == state_id && binding.consumer_local() == *target)
}

pub(crate) fn completion_payload_injection_binding<'a>(
    frame_schema: &'a LateLoweredFrameSchema,
    state_id: StateId,
    stmt: &LirStatement,
) -> Option<&'a LateLoweredCompletionPayloadBinding> {
    let binding = frame_schema.completion_payload_binding_for_state(state_id)?;
    let crate::effect_lowered::ir::LateLoweredCompletionPayloadSource::Operand(source) =
        binding.payload_source()
    else {
        return None;
    };
    let crate::effect_lowered::ir::LateLoweredOperandValueSource::Local(local) = source.value()
    else {
        return None;
    };
    matches!(
        &stmt.kind,
        LirStatementKind::Assign { target, .. } if *target == *local
    )
    .then_some(binding)
}

pub(crate) fn handle_binder_statement(
    handle_binder_locals: &BTreeMap<(StateId, LocalId), SiteId>,
    state_id: StateId,
    stmt: &LirStatement,
) -> Option<SiteId> {
    let LirStatementKind::Assign { target, .. } = &stmt.kind else {
        return None;
    };
    handle_binder_locals.get(&(state_id, *target)).copied()
}

pub(crate) fn classify_effect_neutral_source_statement(
    body: &Body,
    stmt: &LirStatement,
) -> LateLoweredSourceStatementClassificationKind {
    match &stmt.kind {
        LirStatementKind::Nop => LateLoweredSourceStatementClassificationKind::ElidedUnreachable,
        LirStatementKind::StoreMember { .. } | LirStatementKind::StoreGlobal { .. } => {
            LateLoweredSourceStatementClassificationKind::EffectNeutralValue
        }
        LirStatementKind::Assign { value, .. } => classify_effect_neutral_rvalue(body, value),
    }
}

pub(crate) fn classify_effect_neutral_rvalue(
    body: &Body,
    value: &LirRvalue,
) -> LateLoweredSourceStatementClassificationKind {
    match value {
        LirRvalue::Use(_)
        | LirRvalue::Transport { .. }
        | LirRvalue::TopLevelRef(_)
        | LirRvalue::TypeCheck { .. }
        | LirRvalue::Cast { .. }
        | LirRvalue::SizeOf { .. }
        | LirRvalue::KindOf { .. }
        | LirRvalue::AlignOf { .. }
        | LirRvalue::DescOf { .. }
        | LirRvalue::TypeMetadataLiteral(_)
        | LirRvalue::MemberAccess { .. }
        | LirRvalue::EnumVariant { .. }
        | LirRvalue::ClassCtor { .. }
        | LirRvalue::MakeClosure { .. }
        | LirRvalue::MakeTuple { .. }
        | LirRvalue::StructLit { .. }
        | LirRvalue::InterpolatedString { .. }
        | LirRvalue::TupleGet { .. }
        | LirRvalue::PatternMatch { .. }
        | LirRvalue::PatternExtract { .. } => {
            LateLoweredSourceStatementClassificationKind::EffectNeutralValue
        }
        LirRvalue::Call {
            site_id,
            kind,
            args,
            ..
        } if is_dynamic_call_kind(kind) => {
            LateLoweredSourceStatementClassificationKind::DynamicInvokeCall {
                site_id: *site_id,
                metadata: call_site_materialized_metadata(body, kind, args.len()),
            }
        }
        LirRvalue::Call { .. } => LateLoweredSourceStatementClassificationKind::EffectNeutralValue,
        LirRvalue::PerformResult { .. } => {
            LateLoweredSourceStatementClassificationKind::Unsupported {
                reason: "perform result requires published resume payload injection".to_string(),
            }
        }
    }
}

pub(crate) fn local_is_only_value_member_namespace_receiver(body: &Body, local: LocalId) -> bool {
    let mut saw_value_member = false;
    for block in &body.blocks {
        for stmt in &block.stmts {
            let StatementKind::Assign { target, value } = &stmt.kind else {
                continue;
            };
            if *target == local {
                continue;
            }
            if let Rvalue::MemberAccess {
                receiver: Operand::Local(receiver),
                member,
                ..
            } = value
                && *receiver == local
                && matches!(member.resolved, Some(MemberTarget::Value { .. }))
            {
                saw_value_member = true;
                continue;
            }
            if rvalue_mentions_local(value, local) {
                return false;
            }
        }
    }
    saw_value_member
}

fn call_site_materialized_metadata(
    body: &Body,
    kind: &LirCallKind,
    arg_count: usize,
) -> LateLoweredCallSiteMaterializedMetadata {
    LateLoweredCallSiteMaterializedMetadata::new(
        call_site_materialized_kind(kind),
        arg_count,
        call_carrier_source_ty(body, kind),
    )
}

fn is_dynamic_call_kind(kind: &LirCallKind) -> bool {
    matches!(
        kind,
        LirCallKind::Closure { .. }
            | LirCallKind::FunValue { .. }
            | LirCallKind::FunPtr { .. }
            | LirCallKind::Virtual { .. }
            | LirCallKind::Interface { .. }
    )
}

fn call_site_materialized_kind(kind: &LirCallKind) -> LateLoweredCallSiteMaterializedKind {
    match kind {
        LirCallKind::Direct { .. } => LateLoweredCallSiteMaterializedKind::Direct,
        LirCallKind::Closure { .. } => LateLoweredCallSiteMaterializedKind::Closure,
        LirCallKind::FunValue { .. } => LateLoweredCallSiteMaterializedKind::FunValue,
        LirCallKind::FunPtr { .. } => LateLoweredCallSiteMaterializedKind::FunPtr,
        LirCallKind::Virtual { dispatch, .. } => LateLoweredCallSiteMaterializedKind::Virtual {
            owner_fqn: dispatch.owner.as_str().to_string(),
            member_name: dispatch.member_name.clone(),
            member_fqn: dispatch.member.as_str().to_string(),
            receiver_ty: dispatch.receiver_ty,
        },
        LirCallKind::Interface { dispatch, .. } => LateLoweredCallSiteMaterializedKind::Interface {
            owner_fqn: dispatch.owner.as_str().to_string(),
            member_name: dispatch.member_name.clone(),
            member_fqn: dispatch.member.as_str().to_string(),
            receiver_ty: dispatch.receiver_ty,
        },
        LirCallKind::Resume { .. } => LateLoweredCallSiteMaterializedKind::Resume,
    }
}

fn call_carrier_source_ty(body: &Body, kind: &LirCallKind) -> Option<TypeId> {
    match kind {
        LirCallKind::Closure { callee, .. }
        | LirCallKind::FunValue { callee }
        | LirCallKind::FunPtr { callee } => operand_source_ty(body, callee),
        LirCallKind::Virtual { receiver, dispatch }
        | LirCallKind::Interface { receiver, dispatch } => {
            operand_source_ty(body, receiver).or(Some(dispatch.receiver_ty))
        }
        LirCallKind::Resume { continuation, .. } => operand_source_ty(body, continuation),
        LirCallKind::Direct { .. } => None,
    }
}

fn operand_source_ty(body: &Body, operand: &LirOperand) -> Option<TypeId> {
    match operand {
        LirOperand::Local(local) => body.locals.get(local.as_u32() as usize).map(|decl| decl.ty),
        LirOperand::Const(_) => None,
    }
}

pub(crate) fn operand_mentions_local(operand: &Operand, local: LocalId) -> bool {
    matches!(operand, Operand::Local(found) if *found == local)
}

pub(crate) fn call_args_mention_local(args: &[CallArg], local: LocalId) -> bool {
    args.iter()
        .any(|arg| operand_mentions_local(&arg.value, local))
}

pub(crate) fn call_kind_mentions_local(kind: &CallKind, local: LocalId) -> bool {
    match kind {
        CallKind::Direct { .. } => false,
        CallKind::Closure { callee, .. }
        | CallKind::FunValue { callee }
        | CallKind::FunPtr { callee } => operand_mentions_local(callee, local),
        CallKind::Virtual { receiver, .. } | CallKind::Interface { receiver, .. } => {
            operand_mentions_local(receiver, local)
        }
        CallKind::Resume { continuation, .. } => operand_mentions_local(continuation, local),
    }
}

pub(crate) fn rvalue_mentions_local(value: &Rvalue, local: LocalId) -> bool {
    match value {
        Rvalue::Use(operand)
        | Rvalue::Transport { value: operand, .. }
        | Rvalue::TypeCheck { value: operand, .. }
        | Rvalue::Cast { value: operand, .. }
        | Rvalue::TupleGet { tuple: operand, .. }
        | Rvalue::PatternMatch {
            subject: operand, ..
        }
        | Rvalue::PatternExtract {
            subject: operand, ..
        }
        | Rvalue::MakeClosure { env: operand, .. } => operand_mentions_local(operand, local),
        Rvalue::MemberAccess { receiver, .. } => operand_mentions_local(receiver, local),
        Rvalue::EnumVariant { args, .. } | Rvalue::ClassCtor { args, .. } => {
            call_args_mention_local(args, local)
        }
        Rvalue::Call { kind, args, .. } => {
            call_kind_mentions_local(kind, local) || call_args_mention_local(args, local)
        }
        Rvalue::MakeTuple { elements, .. } => elements
            .iter()
            .any(|operand| operand_mentions_local(operand, local)),
        Rvalue::StructLit { fields, .. } => fields
            .iter()
            .any(|field| operand_mentions_local(&field.value, local)),
        Rvalue::InterpolatedString { parts, .. } => parts.iter().any(|part| match part {
            crate::mir::InterpolatedStringPart::Text { .. } => false,
            crate::mir::InterpolatedStringPart::Expr { value, .. } => {
                operand_mentions_local(value, local)
            }
        }),
        Rvalue::TopLevelRef(_)
        | Rvalue::UnresolvedName { .. }
        | Rvalue::SizeOf { .. }
        | Rvalue::KindOf { .. }
        | Rvalue::AlignOf { .. }
        | Rvalue::DescOf { .. }
        | Rvalue::TypeMetadataLiteral(_)
        | Rvalue::PerformResult { .. }
        | Rvalue::Todo(_) => false,
    }
}

pub(crate) fn invalid_source_slice_classification_contract(
    root_fqn: &str,
    detail: String,
) -> EffectLoweringError {
    EffectLoweringError::InvalidSourceSliceClassificationContract {
        root_fqn: root_fqn.to_string(),
        detail,
    }
}
