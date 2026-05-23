//! Boundary operand contract construction (call / class-ctor / perform / resume).

#![allow(dead_code)]

use super::*;

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
    let mut published = None;
    for &source_slice in owner_state.source_slices() {
        validate_source_slice_bounds(root_fqn, site_id, "Call", body, source_slice)?;
        let block = &body.blocks[source_slice.block_id().as_u32() as usize];
        let start = source_slice.start_statement_index() as usize;
        let end = source_slice.end_statement_index() as usize;
        for (offset, stmt) in block.stmts[start..end].iter().enumerate() {
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
                continue;
            };
            if *stmt_site_id != site_id {
                continue;
            }
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
            if *target != result_local {
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
            let statement_index = source_slice.start_statement_index() + offset as u32;
            let carrier_source = match kind {
                CallKind::Direct { .. } => None,
                CallKind::Closure { callee, .. }
                | CallKind::FunValue { callee }
                | CallKind::FunPtr { callee } => Some(operand_source_with_inferred_ty(
                    root_fqn, site_id, "Call", body, callee, None,
                )?),
                CallKind::Virtual { receiver, .. } | CallKind::Interface { receiver, .. } => {
                    Some(operand_source_with_inferred_ty(
                        root_fqn, site_id, "Call", body, receiver, None,
                    )?)
                }
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
            let contract = LateLoweredCallBoundaryOperandContract::new(
                LateLoweredBoundarySourceConsumption::statement(
                    source_slice,
                    statement_index,
                    statement_index.saturating_add(1) == source_slice.end_statement_index(),
                ),
                carrier_source,
                arg_sources,
            );
            if published.replace(contract).is_some() {
                return Err(invalid_boundary_operand_contract(
                    root_fqn,
                    site_id,
                    "Call",
                    "owner state source_slices 中匹配到了多个 statement anchor",
                ));
            }
        }
    }
    published.ok_or_else(|| {
        invalid_boundary_operand_contract(
            root_fqn,
            site_id,
            "Call",
            format!(
                "在 owner state st{} 的 source_slices 中找不到 call statement anchor",
                boundary.owner_state().as_u32(),
            ),
        )
    })
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
    let mut published = None;
    for &source_slice in owner_state.source_slices() {
        validate_source_slice_bounds(root_fqn, site_id, "ClassCtor", body, source_slice)?;
        let block = &body.blocks[source_slice.block_id().as_u32() as usize];
        let start = source_slice.start_statement_index() as usize;
        let end = source_slice.end_statement_index() as usize;
        for (offset, stmt) in block.stmts[start..end].iter().enumerate() {
            let StatementKind::Assign { target, value } = &stmt.kind else {
                continue;
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
                    let Some(crate::mir::MemberTarget::Value { fqn }) = member.resolved.as_ref()
                    else {
                        return Err(invalid_boundary_operand_contract(
                            root_fqn,
                            site_id,
                            "ClassCtor",
                            "hidden member init boundary source 不是 resolved value member",
                        ));
                    };
                    fqn.clone()
                }
                _ => continue,
            };
            if *target != result_local {
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
            let statement_index = source_slice.start_statement_index() + offset as u32;
            let consumption = LateLoweredBoundarySourceConsumption::statement(
                source_slice,
                statement_index,
                statement_index.saturating_add(1) == source_slice.end_statement_index(),
            );
            if published.replace((source_fqn, consumption)).is_some() {
                return Err(invalid_boundary_operand_contract(
                    root_fqn,
                    site_id,
                    "ClassCtor",
                    "owner state source_slices 中匹配到了多个 statement anchor",
                ));
            }
        }
    }
    published.ok_or_else(|| {
        invalid_boundary_operand_contract(
            root_fqn,
            site_id,
            "ClassCtor",
            format!(
                "在 owner state st{} 的 source_slices 中找不到 class ctor statement anchor",
                boundary.owner_state().as_u32(),
            ),
        )
    })
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
    let mut published = None;
    for &source_slice in owner_state.source_slices() {
        validate_source_slice_bounds(root_fqn, site_id, "Perform", body, source_slice)?;
        if !source_slice.includes_terminator() {
            continue;
        }
        let block = &body.blocks[source_slice.block_id().as_u32() as usize];
        let TerminatorKind::Perform {
            site_id: term_site_id,
            args,
            ..
        } = &block.terminator.kind
        else {
            continue;
        };
        if *term_site_id != site_id {
            continue;
        }
        let payload_sources = build_ordered_perform_payload_sources(
            root_fqn,
            site_id,
            body,
            args,
            payload_tuple_ty,
            types,
            nominal_direct_supertypes,
        )?;
        let contract = LateLoweredPerformBoundaryOperandContract::new(
            LateLoweredBoundarySourceConsumption::terminator(source_slice),
            payload_sources,
        );
        if published.replace(contract).is_some() {
            return Err(invalid_boundary_operand_contract(
                root_fqn,
                site_id,
                "Perform",
                "owner state source_slices 中匹配到了多个 terminator anchor",
            ));
        }
    }
    published.ok_or_else(|| {
        invalid_boundary_operand_contract(
            root_fqn,
            site_id,
            "Perform",
            format!(
                "在 owner state st{} 的 source_slices 中找不到 perform terminator anchor",
                boundary.owner_state().as_u32(),
            ),
        )
    })
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
    let mut published = None;
    for &source_slice in owner_state.source_slices() {
        validate_source_slice_bounds(root_fqn, site_id, "Resume", body, source_slice)?;
        let block = &body.blocks[source_slice.block_id().as_u32() as usize];
        let start = source_slice.start_statement_index() as usize;
        let end = source_slice.end_statement_index() as usize;
        for (offset, stmt) in block.stmts[start..end].iter().enumerate() {
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
                continue;
            };
            if *stmt_site_id != site_id {
                continue;
            }
            if *target != result_local {
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
            if resume.resume_ty != facts.resume_tuple_ty() || resume.answer_ty != facts.answer_ty()
            {
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
            // Even when there is no deeper binder/member provenance to follow, the boundary must
            // still publish an authoritative self-route so later LLVM lowering never falls back to
            // source-type guesses for `k.resume(...)`.
            let underlying_continuation_route =
                resolved_continuation_route.route.unwrap_or_else(|| {
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
            let statement_index = source_slice.start_statement_index() + offset as u32;
            let contract = LateLoweredResumeBoundaryOperandContract::new(
                LateLoweredBoundarySourceConsumption::statement(
                    source_slice,
                    statement_index,
                    statement_index.saturating_add(1) == source_slice.end_statement_index(),
                ),
                continuation_source,
                arg_sources,
                underlying_continuation_route,
                resolved_continuation_route.compatible_route_set,
            );
            if published.replace(contract).is_some() {
                return Err(invalid_boundary_operand_contract(
                    root_fqn,
                    site_id,
                    "Resume",
                    "owner state source_slices 中匹配到了多个 statement anchor",
                ));
            }
        }
    }
    published.ok_or_else(|| {
        invalid_boundary_operand_contract(
            root_fqn,
            site_id,
            "Resume",
            format!(
                "在 owner state st{} 的 source_slices 中找不到 resume statement anchor",
                boundary.owner_state().as_u32(),
            ),
        )
    })
}
