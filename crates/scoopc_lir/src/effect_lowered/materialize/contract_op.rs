//! Boundary operand contract construction (call / class-ctor / perform / resume).

#![allow(dead_code)]

use super::*;

fn find_statement_anchor(
    state: &LateLoweredState,
    mut matches_site: impl FnMut(&LirStatement) -> bool,
) -> Option<(LirBodyAnchor, bool)> {
    let len = state.statements().len();
    state
        .statements()
        .iter()
        .enumerate()
        .find_map(|(index, stmt)| {
            matches_site(stmt).then(|| {
                (
                    LirBodyAnchor::statement(
                        state.state_id(),
                        LirStatementIndex::new(index as u32),
                    ),
                    index + 1 == len,
                )
            })
        })
}

fn compat_statement_consumption(
    root_fqn: &str,
    site_id: SiteId,
    kind: &'static str,
    state: &LateLoweredState,
    anchor: LirBodyAnchor,
    consumes_last_statement: bool,
) -> Result<LateLoweredBoundarySourceConsumption, EffectLoweringError> {
    let local_statement = match anchor {
        LirBodyAnchor::Statement { statement, .. } => statement.as_u32(),
        LirBodyAnchor::State { .. } | LirBodyAnchor::Terminator { .. } => 0,
    };
    let mut local_offset = 0u32;
    for source_slice in state.source_slices() {
        let slice_len = source_slice
            .end_statement_index()
            .saturating_sub(source_slice.start_statement_index());
        if local_statement < local_offset + slice_len {
            let statement_index =
                source_slice.start_statement_index() + local_statement - local_offset;
            return Ok(LateLoweredBoundarySourceConsumption::statement(
                *source_slice,
                statement_index,
                consumes_last_statement,
            ));
        }
        local_offset += slice_len;
    }
    Err(invalid_boundary_operand_contract(
        root_fqn,
        site_id,
        kind,
        format!(
            "owner state st{} 的 LIR statement{} 无对应 source slice",
            state.state_id().as_u32(),
            local_statement,
        ),
    ))
}

fn compat_terminator_consumption(
    root_fqn: &str,
    site_id: SiteId,
    kind: &'static str,
    state: &LateLoweredState,
) -> Result<LateLoweredBoundarySourceConsumption, EffectLoweringError> {
    let source_slice = state
        .source_slices()
        .iter()
        .copied()
        .find(|slice| slice.includes_terminator())
        .ok_or_else(|| {
            invalid_boundary_operand_contract(
                root_fqn,
                site_id,
                kind,
                format!(
                    "owner state st{} 无 terminator source slice",
                    state.state_id().as_u32(),
                ),
            )
        })?;
    Ok(LateLoweredBoundarySourceConsumption::terminator(
        source_slice,
    ))
}

fn lir_call_site(stmt: &LirStatement, site_id: SiteId) -> bool {
    matches!(
        &stmt.kind,
        LirStatementKind::Assign {
            value: LirRvalue::Call { site_id: stmt_site_id, .. },
            ..
        } if *stmt_site_id == site_id
    )
}

fn lir_class_ctor_site(stmt: &LirStatement, site_id: SiteId) -> bool {
    matches!(
        &stmt.kind,
        LirStatementKind::Assign {
            value:
                LirRvalue::ClassCtor { site_id: stmt_site_id, .. }
                | LirRvalue::TopLevelRef(LirTopLevelRef { site_id: Some(stmt_site_id), .. })
                | LirRvalue::MemberAccess { site_id: Some(stmt_site_id), .. },
            ..
        } if *stmt_site_id == site_id
    )
}

fn lir_resume_site(stmt: &LirStatement, site_id: SiteId) -> bool {
    matches!(
        &stmt.kind,
        LirStatementKind::Assign {
            value: LirRvalue::Call {
                site_id: stmt_site_id,
                kind: LirCallKind::Resume { .. },
                ..
            },
            ..
        } if *stmt_site_id == site_id
    )
}

fn mir_call_statement(body: &Body, site_id: SiteId) -> Option<(LocalId, &CallKind, &[CallArg])> {
    body.blocks.iter().find_map(|block| {
        block.stmts.iter().find_map(|stmt| {
            let StatementKind::Assign {
                target,
                value:
                    Rvalue::Call {
                        site_id: stmt_site_id,
                        kind,
                        args,
                        ..
                    },
            } = &stmt.kind
            else {
                return None;
            };
            (*stmt_site_id == site_id).then_some((*target, kind, args.as_slice()))
        })
    })
}

fn mir_resume_statement(
    body: &Body,
    site_id: SiteId,
) -> Option<(LocalId, &Operand, &ResumeMetadata, &[CallArg])> {
    body.blocks.iter().find_map(|block| {
        block.stmts.iter().find_map(|stmt| {
            let StatementKind::Assign {
                target,
                value:
                    Rvalue::Call {
                        site_id: stmt_site_id,
                        kind:
                            CallKind::Resume {
                                continuation,
                                resume,
                            },
                        args,
                        ..
                    },
            } = &stmt.kind
            else {
                return None;
            };
            (*stmt_site_id == site_id).then_some((*target, continuation, resume, args.as_slice()))
        })
    })
}

fn mir_class_ctor_source(body: &Body, site_id: SiteId) -> Option<(LocalId, String)> {
    body.blocks.iter().find_map(|block| {
        block.stmts.iter().find_map(|stmt| {
            let StatementKind::Assign { target, value } = &stmt.kind else {
                return None;
            };
            let source_fqn = match value {
                Rvalue::ClassCtor {
                    site_id: stmt_site_id,
                    class_fqn,
                    ..
                } if *stmt_site_id == site_id => class_fqn.clone(),
                Rvalue::TopLevelRef(top_level)
                    if top_level.site_id == Some(site_id)
                        && !top_level.hidden_effects.is_pure() =>
                {
                    top_level.fqn.clone()
                }
                Rvalue::MemberAccess {
                    site_id: Some(stmt_site_id),
                    member,
                    ..
                } if *stmt_site_id == site_id && !member.hidden_effects.is_pure() => {
                    let Some(MemberTarget::Value { fqn }) = member.resolved.as_ref() else {
                        return None;
                    };
                    fqn.clone()
                }
                _ => return None,
            };
            Some((*target, source_fqn))
        })
    })
}

fn mir_perform_args(body: &Body, site_id: SiteId) -> Option<&[PerformArg]> {
    body.blocks.iter().find_map(|block| {
        let TerminatorKind::Perform {
            site_id: term_site_id,
            args,
            ..
        } = &block.terminator.kind
        else {
            return None;
        };
        (*term_site_id == site_id).then_some(args.as_slice())
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn build_call_boundary_operand_contract(
    root_fqn: &str,
    body: &Body,
    state_graph: &LateLoweredStateGraph,
    boundary: &crate::effect_lowered::ir::LateLoweredBoundary,
    facts: &CallSiteEffectFacts,
    result_local: LocalId,
    types: &TypeStore,
    nominal_direct_supertypes: &NominalDirectSupertypeIndex,
) -> Result<LateLoweredCallBoundaryOperandContract, EffectLoweringError> {
    let LateLoweredBoundarySource::Site {
        site_id,
        kind: BoundarySiteKind::Call,
    } = boundary.source()
    else {
        unreachable!("Call boundary helper 只能消费 Call site source");
    };
    let owner_state = state_graph.state(boundary.owner_state()).ok_or_else(|| {
        invalid_boundary_operand_contract(
            root_fqn,
            site_id,
            "Call",
            format!("缺少 owner state st{}", boundary.owner_state().as_u32()),
        )
    })?;
    let (target, kind, args) = mir_call_statement(body, site_id).ok_or_else(|| {
        invalid_boundary_operand_contract(
            root_fqn,
            site_id,
            "Call",
            "canonical MIR body 中找不到 call statement",
        )
    })?;
    if !call_kind_matches_facts(kind, facts) {
        return Err(invalid_boundary_operand_contract(
            root_fqn,
            site_id,
            "Call",
            format!(
                "canonical MIR call kind {kind:?} 与 published Call facts kind {:?} 不一致",
                facts.kind(),
            ),
        ));
    }
    if target != result_local {
        return Err(invalid_boundary_operand_contract(
            root_fqn,
            site_id,
            "Call",
            format!(
                "statement anchor 写入 local{}，但 boundary lowering 发布的 result local 为 local{}",
                target.as_u32(),
                result_local.as_u32(),
            ),
        ));
    }
    let (anchor, consumes_last_statement) =
        find_statement_anchor(owner_state, |stmt| lir_call_site(stmt, site_id)).ok_or_else(
            || {
                invalid_boundary_operand_contract(
                    root_fqn,
                    site_id,
                    "Call",
                    format!(
                        "在 owner state st{} 的 LIR statements 中找不到 call statement anchor",
                        boundary.owner_state().as_u32(),
                    ),
                )
            },
        )?;
    let carrier_source = match kind {
        CallKind::Direct { .. } => None,
        CallKind::Closure { callee, .. }
        | CallKind::FunValue { callee }
        | CallKind::FunPtr { callee } => Some(operand_source_with_inferred_ty(
            root_fqn, site_id, "Call", body, callee, None,
        )?),
        CallKind::Virtual { receiver, .. } | CallKind::Interface { receiver, .. } => Some(
            operand_source_with_inferred_ty(root_fqn, site_id, "Call", body, receiver, None)?,
        ),
        CallKind::Resume { .. } => {
            return Err(invalid_boundary_operand_contract(
                root_fqn,
                site_id,
                "Call",
                "boundary anchor 意外指向了 Resume MIR call kind",
            ));
        }
    };
    let arg_sources = match kind {
        CallKind::Closure { callee, .. } | CallKind::FunValue { callee }
            if facts.target_mode() == CallTargetMode::KnownInstance =>
        {
            match build_known_instance_closure_call_arg_sources(
                root_fqn,
                site_id,
                "Call",
                body,
                types,
                nominal_direct_supertypes,
                callee,
                args,
                facts.invoke_args_tuple_ty(),
            )? {
                Some(sources) => sources,
                None => build_ordered_call_arg_sources(
                    root_fqn,
                    site_id,
                    "Call",
                    body,
                    args,
                    facts.invoke_args_tuple_ty(),
                    types,
                    nominal_direct_supertypes,
                )?,
            }
        }
        _ => build_ordered_call_arg_sources(
            root_fqn,
            site_id,
            "Call",
            body,
            args,
            facts.invoke_args_tuple_ty(),
            types,
            nominal_direct_supertypes,
        )?,
    };
    Ok(LateLoweredCallBoundaryOperandContract::new(
        compat_statement_consumption(
            root_fqn,
            site_id,
            "Call",
            owner_state,
            anchor,
            consumes_last_statement,
        )?,
        carrier_source,
        arg_sources,
    ))
}

pub(crate) fn build_class_ctor_boundary_source_contract(
    root_fqn: &str,
    body: &Body,
    state_graph: &LateLoweredStateGraph,
    boundary: &crate::effect_lowered::ir::LateLoweredBoundary,
    result_local: LocalId,
) -> Result<(String, LateLoweredBoundarySourceConsumption), EffectLoweringError> {
    let LateLoweredBoundarySource::Site {
        site_id,
        kind: BoundarySiteKind::ClassCtor,
    } = boundary.source()
    else {
        unreachable!("ClassCtor boundary helper 只能消费 ClassCtor site source");
    };
    let owner_state = state_graph.state(boundary.owner_state()).ok_or_else(|| {
        invalid_boundary_operand_contract(
            root_fqn,
            site_id,
            "ClassCtor",
            format!("缺少 owner state st{}", boundary.owner_state().as_u32()),
        )
    })?;
    let (target, source_fqn) = mir_class_ctor_source(body, site_id).ok_or_else(|| {
        invalid_boundary_operand_contract(
            root_fqn,
            site_id,
            "ClassCtor",
            "canonical MIR body 中找不到 class ctor statement anchor",
        )
    })?;
    if target != result_local {
        return Err(invalid_boundary_operand_contract(
            root_fqn,
            site_id,
            "ClassCtor",
            format!(
                "statement anchor 写入 local{}，但 boundary lowering 发布的 result local 为 local{}",
                target.as_u32(),
                result_local.as_u32(),
            ),
        ));
    }
    let (anchor, consumes_last_statement) = find_statement_anchor(owner_state, |stmt| {
        lir_class_ctor_site(stmt, site_id)
    })
    .ok_or_else(|| {
        invalid_boundary_operand_contract(
            root_fqn,
            site_id,
            "ClassCtor",
            format!(
                "在 owner state st{} 的 LIR statements 中找不到 class ctor statement anchor",
                boundary.owner_state().as_u32(),
            ),
        )
    })?;
    Ok((
        source_fqn,
        compat_statement_consumption(
            root_fqn,
            site_id,
            "ClassCtor",
            owner_state,
            anchor,
            consumes_last_statement,
        )?,
    ))
}

pub(crate) fn build_perform_boundary_operand_contract(
    root_fqn: &str,
    body: &Body,
    state_graph: &LateLoweredStateGraph,
    boundary: &crate::effect_lowered::ir::LateLoweredBoundary,
    payload_tuple_ty: crate::ty::TypeId,
    types: &TypeStore,
    nominal_direct_supertypes: &NominalDirectSupertypeIndex,
) -> Result<LateLoweredPerformBoundaryOperandContract, EffectLoweringError> {
    let LateLoweredBoundarySource::Site {
        site_id,
        kind: BoundarySiteKind::Perform,
    } = boundary.source()
    else {
        unreachable!("Perform boundary helper 只能消费 Perform site source");
    };
    let owner_state = state_graph.state(boundary.owner_state()).ok_or_else(|| {
        invalid_boundary_operand_contract(
            root_fqn,
            site_id,
            "Perform",
            format!("缺少 owner state st{}", boundary.owner_state().as_u32()),
        )
    })?;
    let args = mir_perform_args(body, site_id).ok_or_else(|| {
        invalid_boundary_operand_contract(
            root_fqn,
            site_id,
            "Perform",
            "canonical MIR body 中找不到 perform terminator anchor",
        )
    })?;
    let payload_sources = build_ordered_perform_payload_sources(
        root_fqn,
        site_id,
        body,
        args,
        payload_tuple_ty,
        types,
        nominal_direct_supertypes,
    )?;
    if !matches!(
        owner_state.terminator(),
        LateLoweredStateTerminator::Suspend { .. }
    ) {
        return Err(invalid_boundary_operand_contract(
            root_fqn,
            site_id,
            "Perform",
            format!(
                "owner state st{} 不是 perform boundary 的 Suspend terminator",
                boundary.owner_state().as_u32(),
            ),
        ));
    }
    Ok(LateLoweredPerformBoundaryOperandContract::new(
        compat_terminator_consumption(root_fqn, site_id, "Perform", owner_state)?,
        payload_sources,
    ))
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn build_resume_boundary_operand_contract(
    root_fqn: &str,
    owner_version_key: &LateLoweredBodyVersionKey,
    body: &Body,
    state_graph: &LateLoweredStateGraph,
    boundary: &crate::effect_lowered::ir::LateLoweredBoundary,
    facts: &ResumeSiteEffectFacts,
    result_local: LocalId,
    continuation_provenance: &PublishedContinuationProvenance,
    continuation_object: ContinuationObjectId,
    types: &TypeStore,
    nominal_direct_supertypes: &NominalDirectSupertypeIndex,
) -> Result<LateLoweredResumeBoundaryOperandContract, EffectLoweringError> {
    let LateLoweredBoundarySource::Site {
        site_id,
        kind: BoundarySiteKind::Resume,
    } = boundary.source()
    else {
        unreachable!("Resume boundary helper 只能消费 Resume site source");
    };
    let owner_state = state_graph.state(boundary.owner_state()).ok_or_else(|| {
        invalid_boundary_operand_contract(
            root_fqn,
            site_id,
            "Resume",
            format!("缺少 owner state st{}", boundary.owner_state().as_u32()),
        )
    })?;
    let (target, continuation, resume, args) =
        mir_resume_statement(body, site_id).ok_or_else(|| {
            invalid_boundary_operand_contract(
                root_fqn,
                site_id,
                "Resume",
                "canonical MIR body 中找不到 resume statement anchor",
            )
        })?;
    if target != result_local {
        return Err(invalid_boundary_operand_contract(
            root_fqn,
            site_id,
            "Resume",
            format!(
                "statement anchor 写入 local{}，但 boundary lowering 发布的 result local 为 local{}",
                target.as_u32(),
                result_local.as_u32(),
            ),
        ));
    }
    if resume.resume_ty != facts.resume_tuple_ty() || resume.answer_ty != facts.answer_ty() {
        return Err(invalid_boundary_operand_contract(
            root_fqn,
            site_id,
            "Resume",
            format!(
                "canonical MIR resume metadata 与 published facts 漂移：resume_tuple=t{} answer_ty=t{}，facts=(t{}, t{})",
                resume.resume_ty.as_u32(),
                resume.answer_ty.as_u32(),
                facts.resume_tuple_ty().as_u32(),
                facts.answer_ty().as_u32(),
            ),
        ));
    }
    let (anchor, consumes_last_statement) =
        find_statement_anchor(owner_state, |stmt| lir_resume_site(stmt, site_id)).ok_or_else(
            || {
                invalid_boundary_operand_contract(
                    root_fqn,
                    site_id,
                    "Resume",
                    format!(
                        "在 owner state st{} 的 LIR statements 中找不到 resume statement anchor",
                        boundary.owner_state().as_u32(),
                    ),
                )
            },
        )?;
    let continuation_source = operand_source_with_expected_ty(
        root_fqn,
        site_id,
        "Resume",
        body,
        types,
        nominal_direct_supertypes,
        continuation,
        resume.continuation_ty,
        None,
    )?;
    let resolved_continuation_route = match continuation_source.value() {
        crate::effect_lowered::ir::LateLoweredOperandValueSource::Local(local) => {
            continuation_provenance.resolve_resume_local_route(root_fqn, site_id, *local)?
        }
        crate::effect_lowered::ir::LateLoweredOperandValueSource::Const(_) => {
            ResolvedResumeLocalRoute {
                route: None,
                compatible_route_set: false,
            }
        }
    };
    let underlying_continuation_route = resolved_continuation_route.route.unwrap_or_else(|| {
        LateLoweredContinuationRoute::new(
            facts.continuation_schema(),
            LateLoweredSurfaceResumeDispatchPublication::ResumeBoundary {
                owner_version_key: owner_version_key.clone(),
                owner_continuation_object: continuation_object,
                site_id,
            },
        )
    });
    let arg_sources = build_ordered_call_arg_sources(
        root_fqn,
        site_id,
        "Resume",
        body,
        args,
        facts.resume_tuple_ty(),
        types,
        nominal_direct_supertypes,
    )?;
    Ok(LateLoweredResumeBoundaryOperandContract::new(
        compat_statement_consumption(
            root_fqn,
            site_id,
            "Resume",
            owner_state,
            anchor,
            consumes_last_statement,
        )?,
        continuation_source,
        arg_sources,
        underlying_continuation_route,
        resolved_continuation_route.compatible_route_set,
    ))
}
