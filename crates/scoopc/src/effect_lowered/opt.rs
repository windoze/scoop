use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::effect_facts::CaseTag;

use super::ir::{
    BoundaryId, ContinuationObjectId, FrameSlotId, LateLoweredBoundary,
    LateLoweredBoundaryLowering, LateLoweredBoundaryMap, LateLoweredCallBoundaryLowering,
    LateLoweredCallable, LateLoweredCompleteStepDispatch, LateLoweredContinuationCapture,
    LateLoweredContinuationMethod, LateLoweredContinuationMethodReachability,
    LateLoweredContinuationObject, LateLoweredDynamicInvokeEntry, LateLoweredFrameSchema,
    LateLoweredFrameSlot, LateLoweredFrameSlotKind, LateLoweredHandleBoundaryLowering,
    LateLoweredPerformBoundaryLowering, LateLoweredProgram, LateLoweredResumeBoundaryLowering,
    LateLoweredResumeInterface, LateLoweredResumeState, LateLoweredResumeStateMap,
    LateLoweredRuntimeErrorBoundaryLowering, LateLoweredState, LateLoweredStateGraph,
    LateLoweredStateRole, LateLoweredStateTerminator, LateLoweredStepDispatchPlan,
    ResumeInterfaceId, StateId,
};

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct LateLoweredOptOptions {
    pub(crate) preserve_published_resume_shells: bool,
}

impl LateLoweredOptOptions {
    #[cfg_attr(not(feature = "llvm"), allow(dead_code))]
    pub(crate) const fn preserve_published_resume_shells() -> Self {
        Self {
            preserve_published_resume_shells: true,
        }
    }
}

/// 在 late-lowered IR 上执行窄的 post-lowering 收缩。
///
/// 该 pass 只消费 `LateLoweredProgram`：
/// - 不重新读取 HIR/P3 MIR/P4 solver 结果；
/// - 不改动 `StepSchema` / `CaseTag` / `ImplPlan` / canonical dynamic invoke contract；
/// - 只做 wrapper state 折叠、internal resume interface 去虚化，以及死代码/死 slot 清理。
pub(crate) fn optimize_program(program: LateLoweredProgram) -> LateLoweredProgram {
    optimize_program_with_options(program, LateLoweredOptOptions::default())
}

pub(crate) fn optimize_program_with_options(
    program: LateLoweredProgram,
    options: LateLoweredOptOptions,
) -> LateLoweredProgram {
    let mut optimized_objects =
        BTreeMap::<ContinuationObjectId, LateLoweredContinuationObject>::new();
    let mut optimized_callables = Vec::with_capacity(program.len());

    for callable in program.callables() {
        let continuation_object = program
            .continuation_object(callable.continuation_object())
            .expect("every callable should point at a published continuation object");
        let optimized = optimize_callable(callable, continuation_object, options);
        optimized_objects.insert(
            optimized.continuation_object.object_id(),
            optimized.continuation_object,
        );
        optimized_callables.push(optimized.callable);
    }

    if options.preserve_published_resume_shells {
        let continuation_objects = program
            .continuation_objects()
            .iter()
            .filter_map(|object| optimized_objects.remove(&object.object_id()))
            .collect::<Vec<_>>();
        return LateLoweredProgram::new(
            program.step_types().to_vec(),
            program.resume_packings().to_vec(),
            continuation_objects,
            optimized_callables,
        );
    }

    let live_methods_by_interface = collect_live_methods_by_interface(optimized_objects.values());
    let live_interface_ids = live_methods_by_interface
        .keys()
        .copied()
        .collect::<BTreeSet<_>>();

    let resume_packings = program
        .resume_packings()
        .iter()
        .filter_map(|interface| {
            prune_resume_interface(
                interface,
                live_methods_by_interface.get(&interface.interface_id()),
            )
        })
        .collect::<Vec<_>>();

    let continuation_objects = program
        .continuation_objects()
        .iter()
        .filter_map(|object| {
            optimized_objects
                .remove(&object.object_id())
                .map(|optimized| prune_object_interfaces(optimized, &live_interface_ids))
        })
        .collect::<Vec<_>>();
    let implemented_packings_by_object = continuation_objects
        .iter()
        .map(|object| (object.object_id(), object.implemented_packings().to_vec()))
        .collect::<BTreeMap<_, _>>();

    let callables = optimized_callables
        .into_iter()
        .map(|callable| {
            let resume_packings = implemented_packings_by_object
                .get(&callable.continuation_object())
                .cloned()
                .unwrap_or_default();
            LateLoweredCallable::new(
                callable.root_fqn().to_string(),
                callable.body_version_key().clone(),
                callable.step_schema(),
                callable.resolved_outward_cases().to_vec(),
                callable.dynamic_invoke_entry().clone(),
                callable.state_graph().clone(),
                callable.frame_schema().clone(),
                callable.boundary_map().clone(),
                callable.resume_state_map().clone(),
                callable.continuation_object(),
                resume_packings,
            )
        })
        .collect::<Vec<_>>();

    LateLoweredProgram::new(
        program.step_types().to_vec(),
        resume_packings,
        continuation_objects,
        callables,
    )
}

struct OptimizedCallable {
    callable: LateLoweredCallable,
    continuation_object: LateLoweredContinuationObject,
}

fn optimize_callable(
    callable: &LateLoweredCallable,
    continuation_object: &LateLoweredContinuationObject,
    options: LateLoweredOptOptions,
) -> OptimizedCallable {
    let redirects = collect_state_redirects(callable.state_graph());
    let state_graph = rewrite_state_graph(callable.state_graph(), &redirects);
    let live_states = state_graph
        .states()
        .iter()
        .map(LateLoweredState::state_id)
        .collect::<BTreeSet<_>>();
    let boundary_map = rewrite_boundary_map(callable.boundary_map(), &redirects, &live_states);
    let live_boundaries = boundary_map
        .entries()
        .iter()
        .map(LateLoweredBoundary::boundary_id)
        .collect::<BTreeSet<_>>();
    let frame_schema = rewrite_frame_schema(
        callable.frame_schema(),
        &redirects,
        &live_states,
        &live_boundaries,
    );
    let live_slots = frame_schema
        .slots()
        .iter()
        .map(LateLoweredFrameSlot::slot_id)
        .collect::<BTreeSet<_>>();
    let methods = continuation_object
        .methods()
        .iter()
        .filter(|method| {
            method.reachability() == LateLoweredContinuationMethodReachability::Reachable
        })
        .cloned()
        .collect::<Vec<_>>();
    let live_interfaces = methods
        .iter()
        .map(LateLoweredContinuationMethod::packing_interface_id)
        .collect::<BTreeSet<_>>();
    let implemented_packings = if options.preserve_published_resume_shells {
        continuation_object.implemented_packings().to_vec()
    } else {
        continuation_object
            .implemented_packings()
            .iter()
            .copied()
            .filter(|interface_id| live_interfaces.contains(interface_id))
            .collect::<Vec<_>>()
    };
    let captures = rewrite_captures(
        continuation_object.captures(),
        &redirects,
        &live_states,
        &live_slots,
    );
    let continuation_object = LateLoweredContinuationObject::new(
        continuation_object.object_id(),
        continuation_object.owner_version_key().clone(),
        continuation_object.continuation_obj_ty(),
        implemented_packings.clone(),
        captures,
        continuation_object.surface_resumes().to_vec(),
        methods,
    );
    let callable = LateLoweredCallable::new(
        callable.root_fqn().to_string(),
        callable.body_version_key().clone(),
        callable.step_schema(),
        callable.resolved_outward_cases().to_vec(),
        rewrite_dynamic_invoke_entry(callable.dynamic_invoke_entry(), &redirects),
        state_graph,
        frame_schema,
        boundary_map.clone(),
        resume_state_map_from_boundaries(&boundary_map),
        callable.continuation_object(),
        if options.preserve_published_resume_shells {
            callable.resume_packings().to_vec()
        } else {
            implemented_packings.clone()
        },
    );

    OptimizedCallable {
        callable,
        continuation_object,
    }
}

fn collect_live_methods_by_interface<'a>(
    continuation_objects: impl Iterator<Item = &'a LateLoweredContinuationObject>,
) -> BTreeMap<ResumeInterfaceId, BTreeSet<CaseTag>> {
    let mut live_methods = BTreeMap::<ResumeInterfaceId, BTreeSet<CaseTag>>::new();
    for continuation_object in continuation_objects {
        for method in continuation_object.methods() {
            live_methods
                .entry(method.packing_interface_id())
                .or_default()
                .insert(method.case_tag());
        }
    }
    live_methods
}

fn prune_resume_interface(
    interface: &LateLoweredResumeInterface,
    live_cases: Option<&BTreeSet<CaseTag>>,
) -> Option<LateLoweredResumeInterface> {
    let live_cases = live_cases?;
    let methods = interface
        .methods()
        .iter()
        .filter(|method| live_cases.contains(&method.case_tag()))
        .cloned()
        .collect::<Vec<_>>();
    if methods.is_empty() {
        return None;
    }

    Some(LateLoweredResumeInterface::new(
        interface.interface_id(),
        interface.effect_family().clone(),
        interface.return_step_schema(),
        methods,
    ))
}

fn prune_object_interfaces(
    continuation_object: LateLoweredContinuationObject,
    live_interface_ids: &BTreeSet<ResumeInterfaceId>,
) -> LateLoweredContinuationObject {
    let methods = continuation_object
        .methods()
        .iter()
        .filter(|method| live_interface_ids.contains(&method.packing_interface_id()))
        .cloned()
        .collect::<Vec<_>>();
    let used_interfaces = methods
        .iter()
        .map(LateLoweredContinuationMethod::packing_interface_id)
        .collect::<BTreeSet<_>>();
    let implemented_packings = continuation_object
        .implemented_packings()
        .iter()
        .copied()
        .filter(|interface_id| used_interfaces.contains(interface_id))
        .collect::<Vec<_>>();

    LateLoweredContinuationObject::new(
        continuation_object.object_id(),
        continuation_object.owner_version_key().clone(),
        continuation_object.continuation_obj_ty(),
        implemented_packings,
        continuation_object.captures().to_vec(),
        continuation_object.surface_resumes().to_vec(),
        methods,
    )
}

fn collect_state_redirects(state_graph: &LateLoweredStateGraph) -> BTreeMap<StateId, StateId> {
    let state_by_id = state_graph
        .states()
        .iter()
        .map(|state| (state.state_id(), state))
        .collect::<BTreeMap<_, _>>();

    state_graph
        .states()
        .iter()
        .filter_map(|state| {
            let redirected = resolve_redirect_target(state.state_id(), state_graph, &state_by_id);
            (redirected != state.state_id()).then_some((state.state_id(), redirected))
        })
        .collect()
}

fn resolve_redirect_target(
    state_id: StateId,
    state_graph: &LateLoweredStateGraph,
    state_by_id: &BTreeMap<StateId, &LateLoweredState>,
) -> StateId {
    let mut current = state_id;
    let mut seen = BTreeSet::new();
    while seen.insert(current) {
        let Some(state) = state_by_id.get(&current).copied() else {
            break;
        };
        let Some(target) = trivial_wrapper_target(state, state_graph) else {
            break;
        };
        current = target;
    }
    current
}

fn trivial_wrapper_target(
    state: &LateLoweredState,
    state_graph: &LateLoweredStateGraph,
) -> Option<StateId> {
    if state.state_id() == state_graph.entry_state()
        || state.state_id() == state_graph.complete_state()
        || state_graph.cleanup_state() == Some(state.state_id())
    {
        return None;
    }
    if state_graph.drop_state() == Some(state.state_id()) {
        return None;
    }
    if !matches!(
        state.role(),
        LateLoweredStateRole::Segment | LateLoweredStateRole::Resume
    ) {
        return None;
    }
    if !state.source_slices().is_empty() {
        return None;
    }
    match state.terminator() {
        LateLoweredStateTerminator::Goto { target } if *target != state.state_id() => Some(*target),
        _ => None,
    }
}

fn rewrite_state_graph(
    state_graph: &LateLoweredStateGraph,
    redirects: &BTreeMap<StateId, StateId>,
) -> LateLoweredStateGraph {
    let rewritten_states = state_graph
        .states()
        .iter()
        .filter(|state| !redirects.contains_key(&state.state_id()))
        .map(|state| rewrite_state(state, redirects))
        .collect::<Vec<_>>();
    let mut live_states = reachable_states(state_graph.entry_state(), &rewritten_states);
    // `drop_state` is entered via the continuation runtime contract rather than an ordinary CFG
    // successor edge, so DCE must seed reachability from it explicitly.
    if let Some(drop_state) = state_graph.drop_state() {
        live_states.extend(reachable_states(drop_state, &rewritten_states));
    }
    live_states.insert(state_graph.complete_state());

    let states = rewritten_states
        .into_iter()
        .filter(|state| live_states.contains(&state.state_id()))
        .collect::<Vec<_>>();
    let cleanup_state = state_graph
        .cleanup_state()
        .filter(|state_id| live_states.contains(state_id));
    let drop_state = state_graph
        .drop_state()
        .filter(|state_id| live_states.contains(state_id));

    LateLoweredStateGraph::new(
        state_graph.entry_state(),
        state_graph.complete_state(),
        cleanup_state,
        drop_state,
        states,
    )
}

fn reachable_states(entry_state: StateId, states: &[LateLoweredState]) -> BTreeSet<StateId> {
    let state_by_id = states
        .iter()
        .map(|state| (state.state_id(), state))
        .collect::<BTreeMap<_, _>>();
    let mut reachable = BTreeSet::new();
    let mut pending = VecDeque::from([entry_state]);
    while let Some(state_id) = pending.pop_front() {
        if !reachable.insert(state_id) {
            continue;
        }
        let Some(state) = state_by_id.get(&state_id) else {
            continue;
        };
        pending.extend(state.successors().iter().copied());
    }
    reachable
}

fn rewrite_state(
    state: &LateLoweredState,
    redirects: &BTreeMap<StateId, StateId>,
) -> LateLoweredState {
    LateLoweredState::new(
        state.state_id(),
        state.role(),
        state.source_slices().to_vec(),
        rewrite_terminator(state.terminator(), redirects),
    )
}

fn rewrite_terminator(
    terminator: &LateLoweredStateTerminator,
    redirects: &BTreeMap<StateId, StateId>,
) -> LateLoweredStateTerminator {
    match terminator.clone() {
        LateLoweredStateTerminator::Suspend {
            boundary_ids,
            resume_state,
            local_runtime_error_states,
            cleanup_state,
            drop_state,
        } => LateLoweredStateTerminator::Suspend {
            boundary_ids,
            resume_state: redirect_state_id(resume_state, redirects),
            local_runtime_error_states: local_runtime_error_states
                .into_iter()
                .map(|state_id| redirect_state_id(state_id, redirects))
                .collect(),
            cleanup_state: cleanup_state.map(|state_id| redirect_state_id(state_id, redirects)),
            drop_state: drop_state.map(|state_id| redirect_state_id(state_id, redirects)),
        },
        LateLoweredStateTerminator::Goto { target } => LateLoweredStateTerminator::Goto {
            target: redirect_state_id(target, redirects),
        },
        LateLoweredStateTerminator::Branch {
            cond_local,
            then_state,
            else_state,
        } => LateLoweredStateTerminator::Branch {
            cond_local,
            then_state: redirect_state_id(then_state, redirects),
            else_state: redirect_state_id(else_state, redirects),
        },
        LateLoweredStateTerminator::Return {
            value_local,
            complete_state,
        } => LateLoweredStateTerminator::Return {
            value_local,
            complete_state: redirect_state_id(complete_state, redirects),
        },
        LateLoweredStateTerminator::HandleDispatch {
            site_id,
            body_state,
            arm_states,
            finally_state,
            exit_state,
            contract,
            boundary_ids,
            drop_state,
        } => LateLoweredStateTerminator::HandleDispatch {
            site_id,
            body_state: redirect_state_id(body_state, redirects),
            arm_states: arm_states
                .into_iter()
                .map(|state_id| redirect_state_id(state_id, redirects))
                .collect(),
            finally_state: finally_state.map(|state_id| redirect_state_id(state_id, redirects)),
            exit_state: redirect_state_id(exit_state, redirects),
            contract: redirect_handle_dispatch_contract(contract, redirects),
            boundary_ids,
            drop_state: drop_state.map(|state_id| redirect_state_id(state_id, redirects)),
        },
        other => other,
    }
}

fn redirect_handle_dispatch_contract(
    contract: crate::effect_lowered::ir::LateLoweredHandleDispatchContract,
    redirects: &BTreeMap<StateId, StateId>,
) -> crate::effect_lowered::ir::LateLoweredHandleDispatchContract {
    crate::effect_lowered::ir::LateLoweredHandleDispatchContract::new(
        contract.carrier(),
        redirect_state_id(contract.body_complete_target(), redirects),
        redirect_state_id(contract.arm_complete_target(), redirects),
        contract
            .finally_complete_target()
            .map(|state_id| redirect_state_id(state_id, redirects)),
        contract
            .handled_arms()
            .iter()
            .map(|arm| {
                crate::effect_lowered::ir::LateLoweredHandleArmDispatch::new(
                    arm.handled_case(),
                    redirect_state_id(arm.arm_state(), redirects),
                    arm.arm_ordinal(),
                    arm.payload_tuple_ty(),
                    arm.payload_binders().to_vec(),
                    arm.continuation_binder(),
                    arm.arm_outward_cases().to_vec(),
                )
            })
            .collect(),
        contract.body_outward_cases().to_vec(),
        contract.finally_outward_cases().to_vec(),
        contract.outward_emissions().to_vec(),
        contract.pending_completions().to_vec(),
        contract
            .state_regions()
            .iter()
            .map(|entry| {
                crate::effect_lowered::ir::LateLoweredHandleStateRegionEntry::new(
                    redirect_state_id(entry.state_id(), redirects),
                    entry.region(),
                )
            })
            .collect(),
        contract
            .boundary_routings()
            .iter()
            .map(|routing| {
                let case_routings = routing
                    .case_routings()
                    .iter()
                    .map(|route| {
                        let action = match route.action() {
                            crate::effect_lowered::ir::LateLoweredHandleBoundaryCaseRoutingAction::ConsumeToArm {
                                arm_state,
                                arm_ordinal,
                                continuation_resume_state,
                            } => crate::effect_lowered::ir::LateLoweredHandleBoundaryCaseRoutingAction::ConsumeToArm {
                                arm_state: redirect_state_id(arm_state, redirects),
                                arm_ordinal,
                                continuation_resume_state: redirect_state_id(
                                    continuation_resume_state,
                                    redirects,
                                ),
                            },
                            crate::effect_lowered::ir::LateLoweredHandleBoundaryCaseRoutingAction::PendingCompletion {
                                completion,
                            } => crate::effect_lowered::ir::LateLoweredHandleBoundaryCaseRoutingAction::PendingCompletion {
                                completion,
                            },
                            crate::effect_lowered::ir::LateLoweredHandleBoundaryCaseRoutingAction::EmitOutward => crate::effect_lowered::ir::LateLoweredHandleBoundaryCaseRoutingAction::EmitOutward,
                        };
                        crate::effect_lowered::ir::LateLoweredHandleBoundaryCaseRouting::new(
                            route.case_tag(),
                            action,
                        )
                    })
                    .collect();
                crate::effect_lowered::ir::LateLoweredHandleBoundaryRouting::new(
                    routing.boundary_id(),
                    redirect_state_id(routing.owner_state(), redirects),
                    routing.owner_region(),
                    redirect_state_id(routing.resume_state(), redirects),
                    case_routings,
                )
            })
            .collect(),
        contract
            .abandon_target()
            .map(|state_id| redirect_state_id(state_id, redirects)),
    )
}

fn redirect_state_id(state_id: StateId, redirects: &BTreeMap<StateId, StateId>) -> StateId {
    let mut current = state_id;
    let mut seen = BTreeSet::new();
    while let Some(next) = redirects.get(&current).copied() {
        if !seen.insert(current) {
            break;
        }
        current = next;
    }
    current
}

fn rewrite_dynamic_invoke_entry(
    entry: &LateLoweredDynamicInvokeEntry,
    redirects: &BTreeMap<StateId, StateId>,
) -> LateLoweredDynamicInvokeEntry {
    LateLoweredDynamicInvokeEntry::new(
        entry.invoke_args_tuple_ty(),
        entry.step_schema(),
        redirect_state_id(entry.entry_state(), redirects),
        redirect_state_id(entry.complete_state(), redirects),
    )
}

fn rewrite_boundary_map(
    boundary_map: &LateLoweredBoundaryMap,
    redirects: &BTreeMap<StateId, StateId>,
    live_states: &BTreeSet<StateId>,
) -> LateLoweredBoundaryMap {
    let entries = boundary_map
        .entries()
        .iter()
        .filter_map(|boundary| {
            let owner_state = redirect_state_id(boundary.owner_state(), redirects);
            let resume_state = redirect_state_id(boundary.resume_state(), redirects);
            if !live_states.contains(&owner_state) || !live_states.contains(&resume_state) {
                return None;
            }
            let boundary = LateLoweredBoundary::new(
                boundary.boundary_id(),
                boundary.source(),
                owner_state,
                resume_state,
            );
            Some(
                match boundary_map
                    .boundary(boundary.boundary_id())
                    .and_then(|b| b.lowering())
                {
                    Some(lowering) => {
                        boundary.with_lowering(rewrite_boundary_lowering(lowering, redirects))
                    }
                    None => boundary,
                },
            )
        })
        .collect::<Vec<_>>();
    LateLoweredBoundaryMap::new(entries)
}

fn rewrite_boundary_lowering(
    lowering: &LateLoweredBoundaryLowering,
    redirects: &BTreeMap<StateId, StateId>,
) -> LateLoweredBoundaryLowering {
    match lowering {
        LateLoweredBoundaryLowering::Call(lowering) => {
            LateLoweredBoundaryLowering::Call(LateLoweredCallBoundaryLowering::new(
                lowering.facts().clone(),
                lowering.result_local(),
                lowering.operand_contract().clone(),
                rewrite_step_dispatch(lowering.dispatch(), redirects),
                lowering.consumed_runtime_error_case().cloned(),
            ))
        }
        LateLoweredBoundaryLowering::Perform(lowering) => {
            LateLoweredBoundaryLowering::Perform(LateLoweredPerformBoundaryLowering::new(
                lowering.facts().clone(),
                lowering.operand_contract().clone(),
                lowering.emitted_step().clone(),
            ))
        }
        LateLoweredBoundaryLowering::Resume(lowering) => {
            LateLoweredBoundaryLowering::Resume(LateLoweredResumeBoundaryLowering::new(
                lowering.facts().clone(),
                lowering.result_local(),
                lowering.runtime_error_boundary(),
                lowering.operand_contract().clone(),
                rewrite_step_dispatch(lowering.dispatch(), redirects),
            ))
        }
        LateLoweredBoundaryLowering::RuntimeError(lowering) => {
            LateLoweredBoundaryLowering::RuntimeError(LateLoweredRuntimeErrorBoundaryLowering::new(
                lowering.origin_site(),
                lowering.resume_boundary(),
                lowering.emitted_step().clone(),
            ))
        }
        LateLoweredBoundaryLowering::Handle(lowering) => {
            LateLoweredBoundaryLowering::Handle(LateLoweredHandleBoundaryLowering::new(
                lowering.facts().clone(),
                lowering.outward_emissions().to_vec(),
            ))
        }
    }
}

fn rewrite_step_dispatch(
    dispatch: &LateLoweredStepDispatchPlan,
    redirects: &BTreeMap<StateId, StateId>,
) -> LateLoweredStepDispatchPlan {
    LateLoweredStepDispatchPlan::new(
        dispatch.input_step_schema(),
        rewrite_complete_dispatch(dispatch.complete(), redirects),
        dispatch.outward_cases().to_vec(),
    )
}

fn rewrite_complete_dispatch(
    complete: &LateLoweredCompleteStepDispatch,
    redirects: &BTreeMap<StateId, StateId>,
) -> LateLoweredCompleteStepDispatch {
    LateLoweredCompleteStepDispatch::new(
        complete.answer_ty(),
        redirect_state_id(complete.target_state(), redirects),
        complete.result_local(),
    )
}

fn resume_state_map_from_boundaries(
    boundary_map: &LateLoweredBoundaryMap,
) -> LateLoweredResumeStateMap {
    LateLoweredResumeStateMap::new(
        boundary_map
            .entries()
            .iter()
            .map(|boundary| {
                LateLoweredResumeState::new(boundary.boundary_id(), boundary.resume_state())
            })
            .collect(),
    )
}

fn rewrite_frame_schema(
    frame_schema: &LateLoweredFrameSchema,
    redirects: &BTreeMap<StateId, StateId>,
    live_states: &BTreeSet<StateId>,
    live_boundaries: &BTreeSet<BoundaryId>,
) -> LateLoweredFrameSchema {
    let slots = frame_schema
        .slots()
        .iter()
        .filter_map(|slot| {
            let kind = rewrite_frame_slot_kind(slot.kind(), live_boundaries)?;
            let write_points = rewrite_state_id_list(slot.write_points(), redirects, live_states);
            let read_points = rewrite_state_id_list(slot.read_points(), redirects, live_states);
            if !slot_is_live(kind, &read_points) {
                return None;
            }
            Some(LateLoweredFrameSlot::new(
                slot.slot_id(),
                kind,
                slot.ty(),
                write_points,
                read_points,
            ))
        })
        .collect();
    LateLoweredFrameSchema::new(slots)
}

fn rewrite_frame_slot_kind(
    kind: LateLoweredFrameSlotKind,
    live_boundaries: &BTreeSet<BoundaryId>,
) -> Option<LateLoweredFrameSlotKind> {
    match kind {
        LateLoweredFrameSlotKind::ResumePayload { boundary, .. }
        | LateLoweredFrameSlotKind::BoundaryResult { boundary, .. }
            if !live_boundaries.contains(&boundary) =>
        {
            None
        }
        other => Some(other),
    }
}

fn rewrite_state_id_list(
    state_ids: &[StateId],
    redirects: &BTreeMap<StateId, StateId>,
    live_states: &BTreeSet<StateId>,
) -> Vec<StateId> {
    state_ids
        .iter()
        .copied()
        .map(|state_id| redirect_state_id(state_id, redirects))
        .filter(|state_id| live_states.contains(state_id))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn slot_is_live(kind: LateLoweredFrameSlotKind, read_points: &[StateId]) -> bool {
    match kind {
        LateLoweredFrameSlotKind::System(_)
        | LateLoweredFrameSlotKind::ResumePayload { .. }
        | LateLoweredFrameSlotKind::BoundaryResult { .. } => true,
        _ => !read_points.is_empty(),
    }
}

fn rewrite_captures(
    captures: &[LateLoweredContinuationCapture],
    redirects: &BTreeMap<StateId, StateId>,
    live_states: &BTreeSet<StateId>,
    live_slots: &BTreeSet<FrameSlotId>,
) -> Vec<LateLoweredContinuationCapture> {
    let mut seen_slots = BTreeSet::new();
    let mut seen_states = BTreeSet::new();
    let mut rewritten = Vec::new();

    for capture in captures {
        match *capture {
            LateLoweredContinuationCapture::FrameSlot(slot_id)
                if live_slots.contains(&slot_id) && seen_slots.insert(slot_id) =>
            {
                rewritten.push(LateLoweredContinuationCapture::FrameSlot(slot_id));
            }
            LateLoweredContinuationCapture::State(state_id) => {
                let redirected = redirect_state_id(state_id, redirects);
                if live_states.contains(&redirected) && seen_states.insert(redirected) {
                    rewritten.push(LateLoweredContinuationCapture::State(redirected));
                }
            }
            LateLoweredContinuationCapture::FrameSlot(_) => {}
        }
    }

    rewritten
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::optimize_program;
    use crate::effect_facts::{
        CaseTag, ConcreteOpKey, ContinuationSchemaId, EffectFamilyKey, ImplPlan, StepSchemaId,
    };
    use crate::effect_lowered::ir::{
        BoundaryId, BoundarySiteKind, ContinuationObjectId, FrameSlotId, LateLoweredBodyVersionKey,
        LateLoweredBoundary, LateLoweredBoundaryMap, LateLoweredBoundarySource,
        LateLoweredCallable, LateLoweredContinuationCapture, LateLoweredContinuationContract,
        LateLoweredContinuationMethod, LateLoweredContinuationObject,
        LateLoweredContinuationResumeBody, LateLoweredContinuationSurfaceResume,
        LateLoweredDynamicInvokeEntry, LateLoweredFrameSchema, LateLoweredFrameSlot,
        LateLoweredFrameSlotKind, LateLoweredOneShotPolicy, LateLoweredProgram,
        LateLoweredResumeInterface, LateLoweredResumeMethod, LateLoweredResumeState,
        LateLoweredResumeStateMap, LateLoweredState, LateLoweredStateGraph, LateLoweredStateRole,
        LateLoweredStateSlice, LateLoweredStateTerminator, LateLoweredStepCase,
        LateLoweredStepType, ResumeInterfaceId, StateId, SystemSlotKind,
    };
    use crate::effect_refactor_pipeline::load_effect_lowered_stage_output_for_dump;
    use crate::mir::{BasicBlockId, InstanceKey, LocalId, SiteId, TemplateKey};
    use crate::session::{EffectPipelineMode, Session, SessionOptions};
    use crate::source::SourceFile;
    use crate::span::Span;
    use crate::ty::{EffectRow, NominalType, RefTypeKind, TypeId, TypeKind, TypeStore};

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

    fn sample_instance_key(fqn: &str) -> InstanceKey {
        InstanceKey {
            template: TemplateKey {
                fqn: fqn.to_string(),
                source_path: PathBuf::from("<mem>/late_opt.scoop"),
                decl_span: Span::new(0, 0),
            },
            type_args: Vec::new(),
            eff_args: Vec::new(),
        }
    }

    fn nominal_effect(types: &mut TypeStore, fqn: &str) -> TypeId {
        types.intern(TypeKind::Ref(RefTypeKind::Nominal(NominalType {
            fqn: fqn.to_string(),
            args: Vec::new(),
            eff: None,
        })))
    }

    fn sample_opt_program() -> LateLoweredProgram {
        let mut types = TypeStore::new();
        let builtins = types.intern_builtins();
        let ping_effect = nominal_effect(&mut types, "sample.Ping");
        let allowed_row = EffectRow::new(vec![ping_effect]);
        let invoke_args_tuple_ty = types.ty_tuple(vec![builtins.int]);
        let payload_tuple_ty = types.ty_tuple(vec![builtins.string]);
        let resume_tuple_ty = types.ty_tuple(vec![builtins.int]);
        let continuation_obj_ty = nominal_effect(&mut types, "sample.CompilerContinuation");
        let surface_ty0 = nominal_effect(&mut types, "sample.SurfaceContinuation0");
        let surface_ty1 = nominal_effect(&mut types, "sample.SurfaceContinuation1");
        let ping_family = EffectFamilyKey::new("sample.Ping".to_string(), Vec::new());

        let step_schema = StepSchemaId::new(7);
        let case0 = CaseTag::new(0);
        let case1 = CaseTag::new(1);
        let contract0 = LateLoweredContinuationContract::new(
            ContinuationSchemaId::new(3),
            resume_tuple_ty,
            builtins.unit,
            step_schema,
            surface_ty0,
        );
        let contract1 = LateLoweredContinuationContract::new(
            ContinuationSchemaId::new(4),
            builtins.unit,
            builtins.unit,
            step_schema,
            surface_ty1,
        );
        let step_type = LateLoweredStepType::new(
            step_schema,
            invoke_args_tuple_ty,
            builtins.unit,
            continuation_obj_ty,
            vec![
                LateLoweredStepCase::new(
                    case0,
                    ConcreteOpKey::new(sample_instance_key("sample.Ping.hit"), ping_family.clone()),
                    payload_tuple_ty,
                    contract0,
                ),
                LateLoweredStepCase::new(
                    case1,
                    ConcreteOpKey::new(
                        sample_instance_key("sample.Ping.pong"),
                        ping_family.clone(),
                    ),
                    builtins.unit,
                    contract1,
                ),
            ],
        );
        let interface_id = ResumeInterfaceId::new(0);
        let resume_interface = LateLoweredResumeInterface::new(
            interface_id,
            ping_family.clone(),
            step_schema,
            vec![
                LateLoweredResumeMethod::new(
                    case0,
                    ConcreteOpKey::new(sample_instance_key("sample.Ping.hit"), ping_family.clone()),
                    contract0,
                ),
                LateLoweredResumeMethod::new(
                    case1,
                    ConcreteOpKey::new(
                        sample_instance_key("sample.Ping.pong"),
                        ping_family.clone(),
                    ),
                    contract1,
                ),
            ],
        );

        let version_key = LateLoweredBodyVersionKey::new(
            sample_instance_key("sample.worker"),
            allowed_row,
            ImplPlan::SingleCase(case0),
            true,
        );
        let continuation_object_id = ContinuationObjectId::new(0);
        let entry_state = StateId::new(0);
        let invoke_wrapper_state = StateId::new(1);
        let owner_state = StateId::new(2);
        let resume_wrapper_state = StateId::new(3);
        let resume_state = StateId::new(4);
        let dead_state = StateId::new(5);
        let complete_state = StateId::new(6);
        let boundary_id = BoundaryId::new(0);
        let live_slot = FrameSlotId::new(0);
        let dead_slot = FrameSlotId::new(1);
        let system_slot = FrameSlotId::new(2);

        let continuation_object = LateLoweredContinuationObject::new(
            continuation_object_id,
            version_key.clone(),
            continuation_obj_ty,
            vec![interface_id],
            vec![
                LateLoweredContinuationCapture::FrameSlot(live_slot),
                LateLoweredContinuationCapture::FrameSlot(dead_slot),
                LateLoweredContinuationCapture::State(resume_wrapper_state),
            ],
            vec![
                LateLoweredContinuationSurfaceResume::new(
                    case0,
                    ConcreteOpKey::new(sample_instance_key("sample.Ping.hit"), ping_family.clone()),
                    contract0,
                    LateLoweredContinuationResumeBody::ResumeCapturedState {
                        repeated_resume: LateLoweredOneShotPolicy::OrdinaryRuntimeErrorOutward,
                    },
                ),
                LateLoweredContinuationSurfaceResume::new(
                    case1,
                    ConcreteOpKey::new(
                        sample_instance_key("sample.Ping.pong"),
                        ping_family.clone(),
                    ),
                    contract1,
                    LateLoweredContinuationResumeBody::Unreachable,
                ),
            ],
            vec![
                LateLoweredContinuationMethod::new(
                    interface_id,
                    case0,
                    ConcreteOpKey::new(sample_instance_key("sample.Ping.hit"), ping_family.clone()),
                    contract0,
                    LateLoweredContinuationResumeBody::ResumeCapturedState {
                        repeated_resume: LateLoweredOneShotPolicy::OrdinaryRuntimeErrorOutward,
                    },
                ),
                LateLoweredContinuationMethod::new(
                    interface_id,
                    case1,
                    ConcreteOpKey::new(sample_instance_key("sample.Ping.pong"), ping_family),
                    contract1,
                    LateLoweredContinuationResumeBody::Unreachable,
                ),
            ],
        );
        let callable = LateLoweredCallable::new(
            "sample.worker".to_string(),
            version_key,
            step_schema,
            vec![case0],
            LateLoweredDynamicInvokeEntry::new(
                invoke_args_tuple_ty,
                step_schema,
                entry_state,
                complete_state,
            ),
            LateLoweredStateGraph::new(
                entry_state,
                complete_state,
                None,
                None,
                vec![
                    LateLoweredState::new(
                        entry_state,
                        LateLoweredStateRole::Entry,
                        Vec::new(),
                        LateLoweredStateTerminator::Goto {
                            target: invoke_wrapper_state,
                        },
                    ),
                    LateLoweredState::new(
                        invoke_wrapper_state,
                        LateLoweredStateRole::Segment,
                        Vec::new(),
                        LateLoweredStateTerminator::Goto {
                            target: owner_state,
                        },
                    ),
                    LateLoweredState::new(
                        owner_state,
                        LateLoweredStateRole::Segment,
                        vec![LateLoweredStateSlice::new(
                            BasicBlockId::from_raw(0),
                            0,
                            1,
                            false,
                        )],
                        LateLoweredStateTerminator::Suspend {
                            boundary_ids: vec![boundary_id],
                            resume_state: resume_wrapper_state,
                            local_runtime_error_states: Vec::new(),
                            cleanup_state: None,
                            drop_state: None,
                        },
                    ),
                    LateLoweredState::new(
                        resume_wrapper_state,
                        LateLoweredStateRole::Resume,
                        Vec::new(),
                        LateLoweredStateTerminator::Goto {
                            target: resume_state,
                        },
                    ),
                    LateLoweredState::new(
                        resume_state,
                        LateLoweredStateRole::Resume,
                        vec![LateLoweredStateSlice::new(
                            BasicBlockId::from_raw(0),
                            1,
                            1,
                            true,
                        )],
                        LateLoweredStateTerminator::Return {
                            value_local: Some(LocalId::from_raw(0)),
                            complete_state,
                        },
                    ),
                    LateLoweredState::new(
                        dead_state,
                        LateLoweredStateRole::Segment,
                        vec![LateLoweredStateSlice::new(
                            BasicBlockId::from_raw(1),
                            0,
                            0,
                            false,
                        )],
                        LateLoweredStateTerminator::Goto {
                            target: complete_state,
                        },
                    ),
                    LateLoweredState::new(
                        complete_state,
                        LateLoweredStateRole::Complete,
                        Vec::new(),
                        LateLoweredStateTerminator::Unreachable,
                    ),
                ],
            ),
            LateLoweredFrameSchema::new(vec![
                LateLoweredFrameSlot::new(
                    live_slot,
                    LateLoweredFrameSlotKind::SourceLocal(LocalId::from_raw(0)),
                    builtins.int,
                    vec![owner_state],
                    vec![resume_wrapper_state],
                ),
                LateLoweredFrameSlot::new(
                    dead_slot,
                    LateLoweredFrameSlotKind::CompilerTemporary(LocalId::from_raw(1)),
                    builtins.int,
                    vec![dead_state],
                    Vec::new(),
                ),
                LateLoweredFrameSlot::new(
                    system_slot,
                    LateLoweredFrameSlotKind::System(SystemSlotKind::StateTag),
                    builtins.int,
                    Vec::new(),
                    Vec::new(),
                ),
            ]),
            LateLoweredBoundaryMap::new(vec![LateLoweredBoundary::new(
                boundary_id,
                LateLoweredBoundarySource::Site {
                    site_id: SiteId::from_raw(1),
                    kind: BoundarySiteKind::Perform,
                },
                owner_state,
                resume_wrapper_state,
            )]),
            LateLoweredResumeStateMap::new(vec![LateLoweredResumeState::new(
                boundary_id,
                resume_wrapper_state,
            )]),
            continuation_object_id,
            vec![interface_id],
        );

        LateLoweredProgram::new(
            vec![step_type],
            vec![resume_interface],
            vec![continuation_object],
            vec![callable],
        )
    }

    #[test]
    fn refactor_late_opt_devirt_prunes_unreachable_internal_resume_methods() {
        let optimized = optimize_program(sample_opt_program());
        let callable = optimized
            .callable("sample.worker")
            .expect("优化后应保留 sample.worker callable");
        let continuation_object = optimized
            .continuation_object(callable.continuation_object())
            .expect("优化后应保留 continuation object");
        let resume_interface = optimized
            .resume_packing(callable.resume_packings()[0])
            .expect("优化后应保留 live resume interface");

        assert_eq!(callable.resume_packings().len(), 1);
        assert_eq!(resume_interface.methods().len(), 1);
        assert_eq!(resume_interface.methods()[0].case_tag(), CaseTag::new(0));
        assert_eq!(continuation_object.methods().len(), 1);
        assert_eq!(continuation_object.methods()[0].case_tag(), CaseTag::new(0));
        assert_eq!(continuation_object.surface_resumes().len(), 2);
        assert_eq!(
            optimized
                .step_type(callable.step_schema())
                .expect("优化后 Step shell 仍应存在")
                .cases()
                .len(),
            2,
        );
    }

    #[test]
    fn refactor_late_opt_inline_collapses_trivial_invoke_and_resume_wrappers() {
        let optimized = optimize_program(sample_opt_program());
        let callable = optimized
            .callable("sample.worker")
            .expect("优化后应保留 sample.worker callable");
        let continuation_object = optimized
            .continuation_object(callable.continuation_object())
            .expect("优化后应保留 continuation object");
        let boundary = callable
            .boundary_map()
            .boundary(BoundaryId::new(0))
            .expect("优化后应保留 boundary");

        assert!(callable.state_graph().state(StateId::new(1)).is_none());
        assert!(callable.state_graph().state(StateId::new(3)).is_none());
        assert!(matches!(
            callable
                .state_graph()
                .state(StateId::new(0))
                .expect("entry state 应保留")
                .terminator(),
            LateLoweredStateTerminator::Goto { target } if *target == StateId::new(2)
        ));
        assert_eq!(boundary.resume_state(), StateId::new(4));
        assert_eq!(
            callable.resume_state_map().state_for(BoundaryId::new(0)),
            Some(StateId::new(4))
        );
        assert!(
            continuation_object
                .captures()
                .contains(&LateLoweredContinuationCapture::State(StateId::new(4)))
        );
        assert!(
            !continuation_object
                .captures()
                .contains(&LateLoweredContinuationCapture::State(StateId::new(3)))
        );
    }

    #[test]
    fn refactor_late_opt_dce_removes_dead_states_and_unused_frame_slots() {
        let optimized = optimize_program(sample_opt_program());
        let callable = optimized
            .callable("sample.worker")
            .expect("优化后应保留 sample.worker callable");
        let continuation_object = optimized
            .continuation_object(callable.continuation_object())
            .expect("优化后应保留 continuation object");

        assert!(callable.state_graph().state(StateId::new(5)).is_none());
        assert!(
            callable
                .frame_schema()
                .slot_for_kind(LateLoweredFrameSlotKind::CompilerTemporary(
                    LocalId::from_raw(1)
                ))
                .is_none(),
            "无读者且只挂在死状态上的 compiler temporary 应被删除"
        );
        assert!(
            callable
                .frame_schema()
                .slot_for_kind(LateLoweredFrameSlotKind::System(SystemSlotKind::StateTag))
                .is_some(),
            "语义必需的系统 slot 不应被误删"
        );
        assert!(!continuation_object.captures().contains(
            &LateLoweredContinuationCapture::FrameSlot(FrameSlotId::new(1))
        ));
    }

    #[test]
    fn refactor_late_opt_preserves_contract_for_step_and_continuation_surface() {
        let before = sample_opt_program();
        let before_callable = before
            .callable("sample.worker")
            .expect("raw program 应保留 sample.worker callable")
            .clone();
        let before_step_type = before
            .step_type(before_callable.step_schema())
            .expect("raw program 应保留 step shell")
            .clone();
        let before_object = before
            .continuation_object(before_callable.continuation_object())
            .expect("raw program 应保留 continuation object")
            .clone();

        let optimized = optimize_program(before);
        let after_callable = optimized
            .callable("sample.worker")
            .expect("优化后应保留 sample.worker callable");
        let after_step_type = optimized
            .step_type(after_callable.step_schema())
            .expect("优化后应保留 step shell");
        let after_object = optimized
            .continuation_object(after_callable.continuation_object())
            .expect("优化后应保留 continuation object");
        let production_source = include_str!("opt.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap_or(include_str!("opt.rs"));

        assert_eq!(after_callable.step_schema(), before_callable.step_schema());
        assert_eq!(after_callable.impl_plan(), before_callable.impl_plan());
        assert_eq!(
            after_callable.resolved_outward_cases(),
            before_callable.resolved_outward_cases()
        );
        assert_eq!(
            after_callable.dynamic_invoke_entry().invoke_args_tuple_ty(),
            before_callable
                .dynamic_invoke_entry()
                .invoke_args_tuple_ty()
        );
        assert_eq!(
            after_callable.dynamic_invoke_entry().step_schema(),
            before_callable.dynamic_invoke_entry().step_schema()
        );
        assert_eq!(after_step_type, &before_step_type);
        assert_eq!(
            after_object.surface_resumes(),
            before_object.surface_resumes()
        );

        for forbidden in [
            "MaterializedEffectFacts",
            "MaterializedMirPassView",
            "RefactorEffectFactsStageOutput",
            "LateLoweredProgramBuilder",
            "build_callable_segmentation",
            "build_callable_frame",
        ] {
            assert!(
                !production_source.contains(forbidden),
                "late opt pass 不应重新依赖高层 effect facts/segmentation 输入: {forbidden}"
            );
        }
    }

    #[test]
    fn refactor_late_opt_devirt_stage_output_is_post_opt_final() {
        let session = refactor_session();
        let source = load_fixture("effect_facts", "single_case_impl_plan.scoop");
        let output = load_effect_lowered_stage_output_for_dump(&session, &source)
            .expect("fixture 应可通过 refactor late-lowering stage");
        let leaf = output
            .program()
            .callable("sample.leaf")
            .expect("stage output 应发布 sample.leaf callable shell");
        let continuation_object = output
            .program()
            .continuation_object(leaf.continuation_object())
            .expect("callable 应能回查 continuation object shell");

        assert_eq!(leaf.resume_packings().len(), 1);
        assert_eq!(continuation_object.methods().len(), 1);
        assert_eq!(continuation_object.surface_resumes().len(), 2);
    }

    #[test]
    fn refactor_late_opt_preserves_dedicated_drop_state_paths() {
        let session = refactor_session();
        let source = load_fixture(
            "effect_lowered_src",
            "dropped_continuation_abandons_remaining_work.scoop",
        );
        let output = load_effect_lowered_stage_output_for_dump(&session, &source)
            .expect("fixture 应可通过 refactor late-lowering stage");
        let callable = output
            .program()
            .callable("sample.helper")
            .expect("stage output 应保留 sample.helper callable shell");
        let drop_state = callable
            .state_graph()
            .drop_state()
            .expect("post-opt output 仍应保留 dedicated drop state");
        let drop_node = callable
            .state_graph()
            .state(drop_state)
            .expect("drop state 应可回查");

        assert_eq!(drop_node.role(), LateLoweredStateRole::Drop);
        assert!(matches!(
            drop_node.terminator(),
            LateLoweredStateTerminator::Abandon
        ));
        assert!(callable.state_graph().states().iter().any(|state| {
            matches!(
                state.terminator(),
                LateLoweredStateTerminator::Suspend {
                    cleanup_state: Some(cleanup_state),
                    drop_state: Some(explicit_drop_state),
                    ..
                } if *cleanup_state != drop_state && *explicit_drop_state == drop_state
            )
        }));
    }
}
