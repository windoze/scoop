//! Source-slice statement classification (resume payload / completion / boundary result injection).

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
    let mut seen_statements = BTreeSet::<(BasicBlockId, u32)>::new();
    let mut matched_boundary_statement_anchors = BTreeSet::<BoundaryId>::new();

    for state in state_graph.states() {
        for &source_slice in state.source_slices() {
            let block = body
                .blocks
                .get(source_slice.block_id().as_u32() as usize)
                .ok_or_else(|| {
                    invalid_source_slice_classification_contract(
                        root_fqn,
                        format!(
                            "state st{} source slice 指向缺失的 canonical MIR block bb{}",
                            state.state_id().as_u32(),
                            source_slice.block_id().as_u32(),
                        ),
                    )
                })?;
            let start = source_slice.start_statement_index() as usize;
            let end = source_slice.end_statement_index() as usize;
            if start > end || end > block.stmts.len() {
                return Err(invalid_source_slice_classification_contract(
                    root_fqn,
                    format!(
                        "state st{} source slice [{}..{}) 越界于 canonical MIR block bb{}（stmt_count={}）",
                        state.state_id().as_u32(),
                        source_slice.start_statement_index(),
                        source_slice.end_statement_index(),
                        source_slice.block_id().as_u32(),
                        block.stmts.len(),
                    ),
                ));
            }

            for stmt_index in
                source_slice.start_statement_index()..source_slice.end_statement_index()
            {
                let key = (source_slice.block_id(), stmt_index);
                if !seen_statements.insert(key) {
                    return Err(invalid_source_slice_classification_contract(
                        root_fqn,
                        format!(
                            "source-slice statement bb{} stmt{} 被多个 state 覆盖，classification contract 不再唯一",
                            source_slice.block_id().as_u32(),
                            stmt_index,
                        ),
                    ));
                }
                let stmt = &block.stmts[stmt_index as usize];
                let kind = classify_source_statement(
                    state.state_id(),
                    body,
                    source_slice,
                    stmt_index,
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
                            "source-slice statement bb{} stmt{} has unsupported source classification: {reason}",
                            source_slice.block_id().as_u32(),
                            stmt_index,
                        ),
                    ));
                }
                classifications.push(LateLoweredSourceStatementClassification::new(
                    source_slice,
                    stmt_index,
                    kind,
                ));
            }
        }
    }

    for (key, boundary_id) in &boundary_statement_anchors {
        if !matched_boundary_statement_anchors.contains(boundary_id) {
            return Err(invalid_source_slice_classification_contract(
                root_fqn,
                format!(
                    "boundary bd{} 的 statement anchor bb{} stmt{} 未落入任何 source-slice classification",
                    boundary_id.as_u32(),
                    key.0.as_u32(),
                    key.1,
                ),
            ));
        }
    }
    Ok(classifications)
}

pub(crate) fn collect_boundary_statement_anchors(
    root_fqn: &str,
    boundary_map: &LateLoweredBoundaryMap,
) -> Result<BTreeMap<(BasicBlockId, u32), BoundaryId>, EffectLoweringError> {
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
        let key = (source_slice.block_id(), statement_index);
        if let Some(existing) = anchors.insert(key, boundary.boundary_id()) {
            return Err(invalid_source_slice_classification_contract(
                root_fqn,
                format!(
                    "bb{} stmt{} 同时被 boundary bd{} 与 bd{} 声明为 consumed anchor",
                    source_slice.block_id().as_u32(),
                    statement_index,
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
    source_slice: LateLoweredStateSlice,
    stmt_index: u32,
    stmt: &crate::mir::Statement,
    frame_schema: &LateLoweredFrameSchema,
    boundary_statement_anchors: &BTreeMap<(BasicBlockId, u32), BoundaryId>,
    handle_binder_locals: &BTreeMap<(StateId, LocalId), SiteId>,
    matched_boundary_statement_anchors: &mut BTreeSet<BoundaryId>,
) -> LateLoweredSourceStatementClassificationKind {
    let key = (source_slice.block_id(), stmt_index);
    if let Some(boundary_id) = boundary_statement_anchors.get(&key).copied() {
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
    stmt: &crate::mir::Statement,
) -> Option<LateLoweredResumePayloadBinding> {
    let StatementKind::Assign {
        target,
        value: Rvalue::PerformResult { .. },
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
    stmt: &crate::mir::Statement,
) -> Option<LateLoweredResumePayloadBinding> {
    let StatementKind::Assign { target, .. } = &stmt.kind else {
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
    stmt: &crate::mir::Statement,
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
        StatementKind::Assign { target, .. } if *target == *local
    )
    .then_some(binding)
}

pub(crate) fn handle_binder_statement(
    handle_binder_locals: &BTreeMap<(StateId, LocalId), SiteId>,
    state_id: StateId,
    stmt: &crate::mir::Statement,
) -> Option<SiteId> {
    let StatementKind::Assign { target, .. } = &stmt.kind else {
        return None;
    };
    handle_binder_locals.get(&(state_id, *target)).copied()
}

pub(crate) fn classify_effect_neutral_source_statement(
    body: &Body,
    stmt: &crate::mir::Statement,
) -> LateLoweredSourceStatementClassificationKind {
    match &stmt.kind {
        StatementKind::Nop => LateLoweredSourceStatementClassificationKind::ElidedUnreachable,
        StatementKind::StoreMember { .. } | StatementKind::StoreTopLevelVar { .. } => {
            LateLoweredSourceStatementClassificationKind::EffectNeutralValue
        }
        StatementKind::Assign { target, value } => {
            if matches!(value, Rvalue::Todo("missing expr"))
                && local_is_only_value_member_namespace_receiver(body, *target)
            {
                return LateLoweredSourceStatementClassificationKind::EffectNeutralValue;
            }
            classify_effect_neutral_rvalue(value)
        }
        StatementKind::Todo(reason) => {
            LateLoweredSourceStatementClassificationKind::Unsupported { reason }
        }
    }
}

pub(crate) fn classify_effect_neutral_rvalue(
    value: &Rvalue,
) -> LateLoweredSourceStatementClassificationKind {
    match value {
        Rvalue::Use(_)
        | Rvalue::Transport { .. }
        | Rvalue::TopLevelRef(_)
        | Rvalue::TypeCheck { .. }
        | Rvalue::Cast { .. }
        | Rvalue::SizeOf { .. }
        | Rvalue::KindOf { .. }
        | Rvalue::AlignOf { .. }
        | Rvalue::DescOf { .. }
        | Rvalue::TypeMetadataLiteral(_)
        | Rvalue::MemberAccess { .. }
        | Rvalue::EnumVariant { .. }
        | Rvalue::ClassCtor { .. }
        | Rvalue::Call { .. }
        | Rvalue::MakeClosure { .. }
        | Rvalue::MakeTuple { .. }
        | Rvalue::StructLit { .. }
        | Rvalue::InterpolatedString { .. }
        | Rvalue::TupleGet { .. }
        | Rvalue::CaptureBoxNew { .. }
        | Rvalue::CaptureBoxGet { .. }
        | Rvalue::CaptureBoxSet { .. }
        | Rvalue::PatternMatch { .. }
        | Rvalue::PatternExtract { .. } => {
            LateLoweredSourceStatementClassificationKind::EffectNeutralValue
        }
        Rvalue::UnresolvedName { .. } => {
            LateLoweredSourceStatementClassificationKind::Unsupported {
                reason: "unresolved name requires earlier lowering",
            }
        }
        Rvalue::PerformResult { .. } => LateLoweredSourceStatementClassificationKind::Unsupported {
            reason: "perform result requires published resume payload injection",
        },
        Rvalue::Todo(reason) => {
            LateLoweredSourceStatementClassificationKind::Unsupported { reason }
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
        | Rvalue::CaptureBoxNew { value: operand, .. }
        | Rvalue::CaptureBoxGet {
            box_operand: operand,
            ..
        }
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
        Rvalue::CaptureBoxSet {
            box_operand, value, ..
        } => operand_mentions_local(box_operand, local) || operand_mentions_local(value, local),
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
