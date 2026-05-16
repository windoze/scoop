use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::effect_facts::{
    BodyEffectFacts, CaseTag, ContinuationSchema, ContinuationSchemaId, ImplPlan, SiteEffectFacts,
    StepSchema, StepSchemaId,
};
use crate::mir::{
    BasicBlockId, Body, CallKind, LocalId, LocalSourceKind, Operand, Rvalue, SiteId, StatementKind,
    TerminatorKind,
};
use crate::ty::TypeStore;

use super::EffectLoweringError;
use super::ir::{
    BoundaryId, LateLoweredBoundaryMap, LateLoweredBoundarySource, LateLoweredContinuationCapture,
    LateLoweredFrameSchema, LateLoweredFrameSlot, LateLoweredFrameSlotKind,
    LateLoweredHandleBoundaryCaseRoutingAction, LateLoweredState, LateLoweredStateGraph,
    LateLoweredStateTerminator, StateId, SystemSlotKind,
};

pub(crate) struct FrameLiftingResult {
    pub(crate) state_graph: LateLoweredStateGraph,
    pub(crate) frame_schema: LateLoweredFrameSchema,
    pub(crate) continuation_captures: Vec<LateLoweredContinuationCapture>,
}

pub(crate) struct FrameBuildInputs<'a> {
    pub(crate) root_fqn: &'a str,
    pub(crate) body: &'a Body,
    pub(crate) _body_facts: &'a BodyEffectFacts,
    pub(crate) step_schema_id: StepSchemaId,
    pub(crate) step_schema: &'a StepSchema,
    pub(crate) continuation_schemas: &'a BTreeMap<ContinuationSchemaId, ContinuationSchema>,
    pub(crate) resolved_outward_cases: &'a [CaseTag],
    pub(crate) impl_plan: ImplPlan,
    pub(crate) state_graph: &'a LateLoweredStateGraph,
    pub(crate) boundary_map: &'a LateLoweredBoundaryMap,
    pub(crate) types: &'a TypeStore,
}

pub(crate) fn build_callable_frame(
    inputs: FrameBuildInputs<'_>,
) -> Result<FrameLiftingResult, EffectLoweringError> {
    let FrameBuildInputs {
        root_fqn,
        body,
        _body_facts: body_facts,
        step_schema_id,
        step_schema,
        continuation_schemas,
        resolved_outward_cases,
        impl_plan,
        state_graph,
        boundary_map,
        types,
    } = inputs;

    if boundary_map.entries().is_empty() && !state_graph_contains_handle_dispatch(state_graph) {
        return Ok(FrameLiftingResult {
            state_graph: state_graph.clone(),
            frame_schema: LateLoweredFrameSchema::empty(),
            continuation_captures: Vec::new(),
        });
    }

    let state_graph = attach_drop_state(state_graph);
    let builtins = types
        .builtins()
        .ok_or_else(|| EffectLoweringError::MissingBuiltinTypes {
            root_fqn: root_fqn.to_string(),
        })?;

    let binder_info_by_local = collect_binder_info(body);
    let routing_successors = collect_handle_routing_successors(&state_graph);
    let analysis = analyze_state_locals(
        body,
        &state_graph,
        &binder_info_by_local,
        &routing_successors,
    );
    let boundary_owner_live_out =
        collect_boundary_owner_live_outs(&state_graph, &analysis.live_out);
    let lifted_locals = boundary_owner_live_out
        .values()
        .flat_map(|locals| locals.iter().copied())
        .collect::<BTreeSet<_>>();
    let boundary_result_info = collect_boundary_result_info(body, boundary_map);
    let join_info_by_local = collect_join_info(&state_graph, &analysis);

    let mut slots = Vec::new();
    let mut next_slot_raw = 0u32;
    let mut seen_kinds = BTreeSet::new();

    for local in lifted_locals {
        let Some(kind) = classify_local_slot_kind(
            body,
            local,
            binder_info_by_local.get(&local),
            boundary_result_info.by_local.get(&local),
            join_info_by_local.get(&local),
        ) else {
            continue;
        };

        if !seen_kinds.insert(kind) {
            continue;
        }

        let ty = body.locals[local.as_u32() as usize].ty;
        let write_points = write_points_for_local(local, &boundary_owner_live_out);
        let read_points = analysis.read_points(local);
        slots.push(LateLoweredFrameSlot::new(
            super::ir::FrameSlotId::new(next_slot_raw),
            kind,
            ty,
            write_points,
            read_points,
        ));
        next_slot_raw += 1;
    }

    for info in boundary_result_info.by_boundary.values() {
        let kind = LateLoweredFrameSlotKind::BoundaryResult {
            boundary: info.boundary,
            local: info.local,
        };
        if !seen_kinds.insert(kind) {
            continue;
        }
        let mut write_points = vec![info.defining_state];
        write_points.extend(write_points_for_local(info.local, &boundary_owner_live_out));
        write_points.sort();
        write_points.dedup();
        let read_points = analysis.read_points(info.local);
        slots.push(LateLoweredFrameSlot::new(
            super::ir::FrameSlotId::new(next_slot_raw),
            kind,
            body.locals[info.local.as_u32() as usize].ty,
            write_points,
            read_points,
        ));
        next_slot_raw += 1;
    }

    for boundary in boundary_map.entries() {
        for case_tag in reachable_case_tags(step_schema, resolved_outward_cases, impl_plan) {
            let case = step_schema
                .cases()
                .iter()
                .find(|case| case.case_tag() == case_tag)
                .expect("reachable case tag should exist in StepSchema");
            let Some(continuation_schema) = continuation_schemas.get(&case.continuation_schema())
            else {
                return Err(EffectLoweringError::MissingContinuationSchema {
                    step_schema: step_schema_id.as_u32(),
                    continuation_schema: case.continuation_schema().as_u32(),
                    case_tag: case.case_tag().as_u32(),
                });
            };
            let kind = LateLoweredFrameSlotKind::ResumePayload {
                boundary: boundary.boundary_id(),
                case_tag,
            };
            if !seen_kinds.insert(kind) {
                continue;
            }
            slots.push(LateLoweredFrameSlot::new(
                super::ir::FrameSlotId::new(next_slot_raw),
                kind,
                continuation_schema.resume_tuple_ty(),
                vec![boundary.resume_state()],
                vec![boundary.resume_state()],
            ));
            next_slot_raw += 1;
        }
    }

    for (kind, ty) in collect_handle_pending_payload_slots(
        root_fqn,
        body_facts,
        step_schema_id,
        step_schema,
        &state_graph,
    )? {
        if !seen_kinds.insert(kind) {
            continue;
        }
        // Pending outward payload transport 会在 HandleDispatch contract 中继续发布 routing；
        // frame schema 这里只先保留稳定的 typed slot identity，避免后续 backend 再猜 carrier 形状。
        slots.push(LateLoweredFrameSlot::new(
            super::ir::FrameSlotId::new(next_slot_raw),
            kind,
            ty,
            Vec::new(),
            Vec::new(),
        ));
        next_slot_raw += 1;
    }

    for (kind, ty) in collect_handle_effect_ctx_slots(&state_graph, builtins.any) {
        if !seen_kinds.insert(kind) {
            continue;
        }
        slots.push(LateLoweredFrameSlot::new(
            super::ir::FrameSlotId::new(next_slot_raw),
            kind,
            ty,
            Vec::new(),
            Vec::new(),
        ));
        next_slot_raw += 1;
    }

    for (system_kind, ty) in [
        (SystemSlotKind::StateTag, builtins.int),
        (SystemSlotKind::ResumePayloadCarrier, builtins.any),
        (SystemSlotKind::CleanupFlag, builtins.bool_),
        (SystemSlotKind::OneShotFlag, builtins.bool_),
        (SystemSlotKind::CompletionTag, builtins.int),
        (SystemSlotKind::CurrentEffectCtx, builtins.any),
    ] {
        let kind = LateLoweredFrameSlotKind::System(system_kind);
        if !seen_kinds.insert(kind) {
            continue;
        }
        slots.push(LateLoweredFrameSlot::new(
            super::ir::FrameSlotId::new(next_slot_raw),
            kind,
            ty,
            Vec::new(),
            Vec::new(),
        ));
        next_slot_raw += 1;
    }

    let frame_schema = LateLoweredFrameSchema::new(slots);
    let continuation_captures = build_continuation_captures(&frame_schema, boundary_map);
    Ok(FrameLiftingResult {
        state_graph,
        frame_schema,
        continuation_captures,
    })
}

pub(crate) fn augment_frame_for_handle_dispatch(
    frame_schema: &LateLoweredFrameSchema,
    boundary_map: &LateLoweredBoundaryMap,
    state_graph: &LateLoweredStateGraph,
    any_ty: crate::ty::TypeId,
) -> FrameLiftingResult {
    let mut slots = frame_schema.slots().to_vec();
    let mut seen_kinds = slots
        .iter()
        .map(|slot| slot.kind())
        .collect::<BTreeSet<_>>();
    let mut next_slot_raw = slots
        .iter()
        .map(|slot| slot.slot_id().as_u32())
        .max()
        .unwrap_or(0)
        .saturating_add(u32::from(!slots.is_empty()));

    for (kind, ty) in collect_handle_effect_ctx_slots(state_graph, any_ty) {
        if !seen_kinds.insert(kind) {
            continue;
        }
        slots.push(LateLoweredFrameSlot::new(
            super::ir::FrameSlotId::new(next_slot_raw),
            kind,
            ty,
            Vec::new(),
            Vec::new(),
        ));
        next_slot_raw = next_slot_raw.saturating_add(1);
    }

    let frame_schema = LateLoweredFrameSchema::new(slots);
    let continuation_captures = build_continuation_captures(&frame_schema, boundary_map);
    FrameLiftingResult {
        state_graph: state_graph.clone(),
        frame_schema,
        continuation_captures,
    }
}

fn state_graph_contains_handle_dispatch(state_graph: &LateLoweredStateGraph) -> bool {
    state_graph.states().iter().any(|state| {
        matches!(
            state.terminator(),
            LateLoweredStateTerminator::HandleDispatch { .. }
        )
    })
}

#[derive(Debug, Clone, Copy)]
struct BinderInfo {
    site_id: SiteId,
    ordinal: u32,
}

#[derive(Debug, Clone, Copy)]
struct JoinInfo {
    block: BasicBlockId,
    ordinal: u32,
}

#[derive(Debug, Clone, Copy)]
struct BoundaryResultInfo {
    boundary: BoundaryId,
    local: LocalId,
    defining_state: StateId,
}

#[derive(Default)]
struct BoundaryResultIndex {
    by_local: HashMap<LocalId, BoundaryResultInfo>,
    by_boundary: BTreeMap<BoundaryId, BoundaryResultInfo>,
}

struct StateLocalAnalysis {
    read_states: HashMap<LocalId, BTreeSet<StateId>>,
    def_states: HashMap<LocalId, BTreeSet<StateId>>,
    predecessor_counts: BTreeMap<StateId, usize>,
    live_out: BTreeMap<StateId, BTreeSet<LocalId>>,
}

impl StateLocalAnalysis {
    fn read_points(&self, local: LocalId) -> Vec<StateId> {
        self.read_states
            .get(&local)
            .map(|states| states.iter().copied().collect())
            .unwrap_or_default()
    }
}

fn attach_drop_state(state_graph: &LateLoweredStateGraph) -> LateLoweredStateGraph {
    let next_state_raw = state_graph
        .states()
        .iter()
        .map(|state| state.state_id().as_u32())
        .max()
        .unwrap_or(0)
        .saturating_add(1);
    let drop_state = StateId::new(next_state_raw);
    let mut states = state_graph
        .clone()
        .with_drop_state(Some(drop_state))
        .states()
        .to_vec();
    states.push(LateLoweredState::new(
        drop_state,
        super::ir::LateLoweredStateRole::Drop,
        Vec::new(),
        LateLoweredStateTerminator::Abandon,
    ));
    LateLoweredStateGraph::new(
        state_graph.entry_state(),
        state_graph.complete_state(),
        state_graph.cleanup_state(),
        Some(drop_state),
        states,
    )
}

fn build_continuation_captures(
    frame_schema: &LateLoweredFrameSchema,
    boundary_map: &LateLoweredBoundaryMap,
) -> Vec<LateLoweredContinuationCapture> {
    let mut captures = frame_schema
        .slots()
        .iter()
        .map(|slot| LateLoweredContinuationCapture::FrameSlot(slot.slot_id()))
        .collect::<Vec<_>>();

    let mut seen_states = BTreeSet::new();
    for boundary in boundary_map.entries() {
        if seen_states.insert(boundary.resume_state()) {
            captures.push(LateLoweredContinuationCapture::State(
                boundary.resume_state(),
            ));
        }
    }
    captures
}

fn collect_boundary_owner_live_outs(
    state_graph: &LateLoweredStateGraph,
    live_out: &BTreeMap<StateId, BTreeSet<LocalId>>,
) -> BTreeMap<StateId, BTreeSet<LocalId>> {
    state_graph
        .states()
        .iter()
        .filter_map(|state| match state.terminator() {
            LateLoweredStateTerminator::Suspend { boundary_ids, .. }
                if !boundary_ids.is_empty() =>
            {
                Some((
                    state.state_id(),
                    live_out.get(&state.state_id()).cloned().unwrap_or_default(),
                ))
            }
            LateLoweredStateTerminator::HandleDispatch { boundary_ids, .. }
                if !boundary_ids.is_empty() =>
            {
                Some((
                    state.state_id(),
                    live_out.get(&state.state_id()).cloned().unwrap_or_default(),
                ))
            }
            _ => None,
        })
        .collect()
}

fn collect_handle_routing_successors(
    state_graph: &LateLoweredStateGraph,
) -> BTreeMap<StateId, BTreeSet<StateId>> {
    let mut successors = BTreeMap::<StateId, BTreeSet<StateId>>::new();
    for state in state_graph.states() {
        let LateLoweredStateTerminator::HandleDispatch {
            body_state,
            arm_states,
            finally_state,
            exit_state,
            contract,
            drop_state,
            ..
        } = state.terminator()
        else {
            continue;
        };
        let mut stop_states = arm_states.iter().copied().collect::<BTreeSet<_>>();
        stop_states.insert(*exit_state);
        if let Some(finally_state) = *finally_state {
            stop_states.insert(finally_state);
        }
        if let Some(drop_state) = *drop_state {
            stop_states.insert(drop_state);
        }
        let body_region_states =
            collect_handle_body_region_states(state_graph, *body_state, &stop_states);
        for owner_state in body_region_states {
            successors
                .entry(owner_state)
                .or_default()
                .extend(arm_states.iter().copied());
        }
        for routing in contract.boundary_routings() {
            let entry = successors.entry(routing.owner_state()).or_default();
            for case in routing.case_routings() {
                match case.action() {
                    LateLoweredHandleBoundaryCaseRoutingAction::ConsumeToArm {
                        arm_state, ..
                    } => {
                        entry.insert(arm_state);
                    }
                    LateLoweredHandleBoundaryCaseRoutingAction::PendingCompletion { .. } => {
                        if let Some(finally_state) = *finally_state {
                            entry.insert(finally_state);
                        }
                    }
                    LateLoweredHandleBoundaryCaseRoutingAction::EmitOutward => {}
                }
            }
        }
    }
    successors
}

fn collect_handle_body_region_states(
    state_graph: &LateLoweredStateGraph,
    body_state: StateId,
    stop_states: &BTreeSet<StateId>,
) -> BTreeSet<StateId> {
    let mut visited = BTreeSet::new();
    let mut worklist = vec![body_state];
    while let Some(state_id) = worklist.pop() {
        if stop_states.contains(&state_id) || !visited.insert(state_id) {
            continue;
        }
        let Some(state) = state_graph.state(state_id) else {
            continue;
        };
        worklist.extend(state.successors().iter().rev().copied());
    }
    visited
}

fn write_points_for_local(
    local: LocalId,
    boundary_owner_live_out: &BTreeMap<StateId, BTreeSet<LocalId>>,
) -> Vec<StateId> {
    boundary_owner_live_out
        .iter()
        .filter_map(|(state_id, live_out)| live_out.contains(&local).then_some(*state_id))
        .collect()
}

fn classify_local_slot_kind(
    body: &Body,
    local: LocalId,
    binder_info: Option<&BinderInfo>,
    boundary_result: Option<&BoundaryResultInfo>,
    join_info: Option<&JoinInfo>,
) -> Option<LateLoweredFrameSlotKind> {
    if let Some(boundary_result) = boundary_result {
        return Some(LateLoweredFrameSlotKind::BoundaryResult {
            boundary: boundary_result.boundary,
            local,
        });
    }
    if let Some(info) = binder_info {
        return Some(LateLoweredFrameSlotKind::HandleBinder {
            site_id: info.site_id,
            local,
            ordinal: info.ordinal,
        });
    }
    if let Some(info) = join_info {
        return Some(LateLoweredFrameSlotKind::JoinValue {
            local,
            block: info.block,
            ordinal: info.ordinal,
        });
    }
    let source = body.locals.get(local.as_u32() as usize)?.source;
    match source {
        LocalSourceKind::SourceLocal => Some(LateLoweredFrameSlotKind::SourceLocal(local)),
        LocalSourceKind::CompilerTemporary => {
            Some(LateLoweredFrameSlotKind::CompilerTemporary(local))
        }
    }
}

fn collect_handle_pending_payload_slots(
    root_fqn: &str,
    body_facts: &BodyEffectFacts,
    step_schema_id: StepSchemaId,
    step_schema: &StepSchema,
    state_graph: &LateLoweredStateGraph,
) -> Result<Vec<(LateLoweredFrameSlotKind, crate::ty::TypeId)>, EffectLoweringError> {
    let mut slots = Vec::new();
    for state in state_graph.states() {
        let LateLoweredStateTerminator::HandleDispatch {
            site_id,
            finally_state,
            boundary_ids,
            ..
        } = state.terminator()
        else {
            continue;
        };
        if finally_state.is_none() || boundary_ids.is_empty() {
            continue;
        }
        let handle_facts = match body_facts.site(*site_id) {
            Some(SiteEffectFacts::Handle(facts)) => facts,
            Some(other) => {
                return Err(EffectLoweringError::UnexpectedSiteFactsKind {
                    root_fqn: root_fqn.to_string(),
                    site_id: site_id.as_u32(),
                    expected: "Handle",
                    actual: match other {
                        SiteEffectFacts::Call(_) => "Call",
                        SiteEffectFacts::ClassCtor(_) => "ClassCtor",
                        SiteEffectFacts::Perform(_) => "Perform",
                        SiteEffectFacts::Resume(_) => "Resume",
                        SiteEffectFacts::Handle(_) => unreachable!("已在上方匹配 Handle"),
                    },
                });
            }
            None => {
                return Err(EffectLoweringError::MissingSiteFacts {
                    root_fqn: root_fqn.to_string(),
                    site_id: site_id.as_u32(),
                });
            }
        };
        let mut pending_cases = handle_facts
            .body_outward_cases()
            .tags()
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        for arm in handle_facts.arm_facts() {
            pending_cases.extend(arm.arm_outward_cases().tags().iter().copied());
        }
        for case_tag in pending_cases {
            let payload_tuple_ty = step_schema
                .cases()
                .iter()
                .find(|case| case.case_tag() == case_tag)
                .map(|case| case.payload_tuple_ty())
                .ok_or_else(|| EffectLoweringError::MissingInputStepCase {
                    root_fqn: root_fqn.to_string(),
                    step_schema: step_schema_id.as_u32(),
                    case_tag: case_tag.as_u32(),
                })?;
            slots.push((
                LateLoweredFrameSlotKind::HandlePendingPayload {
                    site_id: *site_id,
                    case_tag,
                },
                payload_tuple_ty,
            ));
        }
    }
    Ok(slots)
}

fn collect_handle_effect_ctx_slots(
    state_graph: &LateLoweredStateGraph,
    any_ty: crate::ty::TypeId,
) -> Vec<(LateLoweredFrameSlotKind, crate::ty::TypeId)> {
    let mut slots = Vec::new();
    for state in state_graph.states() {
        let LateLoweredStateTerminator::HandleDispatch {
            site_id, contract, ..
        } = state.terminator()
        else {
            continue;
        };
        slots.push((
            LateLoweredFrameSlotKind::HandleSavedEffectCtx { site_id: *site_id },
            any_ty,
        ));
        for arm in contract.handled_arms() {
            slots.push((
                LateLoweredFrameSlotKind::HandleArmEffectCtx {
                    site_id: *site_id,
                    arm_ordinal: arm.arm_ordinal(),
                },
                any_ty,
            ));
        }
    }
    slots
}

fn collect_binder_info(body: &Body) -> HashMap<LocalId, BinderInfo> {
    let mut binders = HashMap::new();
    for block in &body.blocks {
        let TerminatorKind::Handle {
            site_id,
            arms,
            arm_targets: _,
            ..
        } = &block.terminator.kind
        else {
            continue;
        };

        for arm in arms {
            for (ordinal, local) in arm.binder_locals.iter().copied().enumerate() {
                binders.insert(
                    local,
                    BinderInfo {
                        site_id: *site_id,
                        ordinal: ordinal as u32,
                    },
                );
            }
        }
    }
    binders
}

fn collect_boundary_result_info(
    body: &Body,
    boundary_map: &LateLoweredBoundaryMap,
) -> BoundaryResultIndex {
    let mut call_result_targets = HashMap::new();
    let mut perform_result_targets = HashMap::new();

    for block in &body.blocks {
        for stmt in &block.stmts {
            let StatementKind::Assign {
                target,
                value: Rvalue::Call { site_id, .. },
            } = &stmt.kind
            else {
                continue;
            };
            call_result_targets.insert(*site_id, *target);
        }

        if let TerminatorKind::Perform { site_id, .. } = block.terminator.kind {
            let local = block.stmts.iter().find_map(|stmt| match &stmt.kind {
                StatementKind::Assign {
                    target,
                    value: Rvalue::PerformResult { .. },
                } => Some(*target),
                StatementKind::Nop
                | StatementKind::Todo(_)
                | StatementKind::Assign { .. }
                | StatementKind::StoreMember { .. }
                | StatementKind::StoreTopLevelVar { .. } => None,
            });
            if let Some(local) = local {
                perform_result_targets.insert(site_id, local);
            }
        }
    }

    let mut index = BoundaryResultIndex::default();
    for boundary in boundary_map.entries() {
        let info = match boundary.source() {
            LateLoweredBoundarySource::Site {
                site_id,
                kind: super::ir::BoundarySiteKind::Call | super::ir::BoundarySiteKind::Resume,
            } => call_result_targets
                .get(&site_id)
                .copied()
                .map(|local| BoundaryResultInfo {
                    boundary: boundary.boundary_id(),
                    local,
                    defining_state: boundary.resume_state(),
                }),
            LateLoweredBoundarySource::Site {
                site_id,
                kind: super::ir::BoundarySiteKind::Perform,
            } => perform_result_targets
                .get(&site_id)
                .map(|local| BoundaryResultInfo {
                    boundary: boundary.boundary_id(),
                    local: *local,
                    defining_state: boundary.resume_state(),
                }),
            LateLoweredBoundarySource::Site {
                kind: super::ir::BoundarySiteKind::ClassCtor | super::ir::BoundarySiteKind::Handle,
                ..
            }
            | LateLoweredBoundarySource::RuntimeError { .. } => None,
        };

        if let Some(info) = info {
            index.by_boundary.insert(boundary.boundary_id(), info);
            index.by_local.entry(info.local).or_insert(info);
        }
    }
    index
}

fn collect_join_info(
    state_graph: &LateLoweredStateGraph,
    analysis: &StateLocalAnalysis,
) -> HashMap<LocalId, JoinInfo> {
    let mut pending = Vec::new();
    for (local, def_states) in &analysis.def_states {
        if def_states.len() < 2 {
            continue;
        }
        let Some((state_id, state)) = analysis
            .read_states
            .get(local)
            .into_iter()
            .flat_map(|states| states.iter())
            .filter_map(|state_id| {
                let predecessor_count = analysis
                    .predecessor_counts
                    .get(state_id)
                    .copied()
                    .unwrap_or(0);
                (predecessor_count > 1)
                    .then(|| state_graph.state(*state_id).map(|state| (*state_id, state)))
                    .flatten()
            })
            .next()
        else {
            continue;
        };
        let merge_block = state
            .source_slices()
            .first()
            .map(|slice| slice.block_id())
            .unwrap_or(BasicBlockId::from_raw(0));
        pending.push((*local, state_id, merge_block));
    }

    pending.sort_by_key(|(local, _, block)| (block.as_u32(), local.as_u32()));
    let mut next_ordinal_by_block = BTreeMap::<BasicBlockId, u32>::new();
    let mut out = HashMap::new();
    for (local, _, block) in pending {
        let ordinal = next_ordinal_by_block.entry(block).or_insert(0);
        out.insert(
            local,
            JoinInfo {
                block,
                ordinal: *ordinal,
            },
        );
        *ordinal += 1;
    }
    out
}

fn analyze_state_locals(
    body: &Body,
    state_graph: &LateLoweredStateGraph,
    binder_info_by_local: &HashMap<LocalId, BinderInfo>,
    routing_successors: &BTreeMap<StateId, BTreeSet<StateId>>,
) -> StateLocalAnalysis {
    let implicit_defs_by_block = collect_implicit_defs_by_block(body);
    let mut defs = BTreeMap::new();
    let mut uses_before_def = BTreeMap::new();
    let mut read_states = HashMap::<LocalId, BTreeSet<StateId>>::new();
    let mut def_states = HashMap::<LocalId, BTreeSet<StateId>>::new();
    let predecessor_counts = collect_predecessor_counts(state_graph, routing_successors);

    for state in state_graph.states() {
        let mut state_defs = BTreeSet::new();
        let mut state_uses = BTreeSet::new();

        for slice in state.source_slices() {
            if slice.start_statement_index() == 0
                && let Some(implicit_defs) = implicit_defs_by_block.get(&slice.block_id())
            {
                for local in implicit_defs {
                    state_defs.insert(*local);
                    def_states
                        .entry(*local)
                        .or_default()
                        .insert(state.state_id());
                }
            }

            let block = &body.blocks[slice.block_id().as_u32() as usize];
            for stmt in &block.stmts
                [slice.start_statement_index() as usize..slice.end_statement_index() as usize]
            {
                collect_statement_uses_before_def(
                    stmt,
                    &mut state_defs,
                    &mut state_uses,
                    &mut read_states,
                    state.state_id(),
                );
                if let StatementKind::Assign { target, .. } = stmt.kind {
                    state_defs.insert(target);
                    def_states
                        .entry(target)
                        .or_default()
                        .insert(state.state_id());
                }
            }

            if slice.includes_terminator() {
                collect_terminator_uses_before_def(
                    &block.terminator.kind,
                    &mut state_defs,
                    &mut state_uses,
                    &mut read_states,
                    state.state_id(),
                );
            }
        }

        for local in binder_info_by_local.keys() {
            if state_defs.contains(local) {
                def_states
                    .entry(*local)
                    .or_default()
                    .insert(state.state_id());
            }
        }

        defs.insert(state.state_id(), state_defs);
        uses_before_def.insert(state.state_id(), state_uses);
    }

    let live_out = solve_live_out(state_graph, &defs, &uses_before_def, routing_successors);
    StateLocalAnalysis {
        read_states,
        def_states,
        predecessor_counts,
        live_out,
    }
}

fn collect_implicit_defs_by_block(body: &Body) -> BTreeMap<BasicBlockId, Vec<LocalId>> {
    let mut implicit_defs = BTreeMap::<BasicBlockId, Vec<LocalId>>::new();
    for block in &body.blocks {
        let TerminatorKind::Handle {
            arms, arm_targets, ..
        } = &block.terminator.kind
        else {
            continue;
        };

        for (target, arm) in arm_targets.iter().copied().zip(arms.iter()) {
            let defs = implicit_defs.entry(target).or_default();
            defs.extend(arm.binder_locals.iter().copied());
            if let Some(local) = arm.continuation_local {
                defs.push(local);
            }
        }
    }
    implicit_defs
}

fn collect_predecessor_counts(
    state_graph: &LateLoweredStateGraph,
    routing_successors: &BTreeMap<StateId, BTreeSet<StateId>>,
) -> BTreeMap<StateId, usize> {
    let mut counts = BTreeMap::new();
    for state in state_graph.states() {
        counts.entry(state.state_id()).or_insert(0);
        for successor in state_successors_with_routing(state, routing_successors) {
            *counts.entry(successor).or_insert(0) += 1;
        }
    }
    counts
}

fn state_successors_with_routing(
    state: &LateLoweredState,
    routing_successors: &BTreeMap<StateId, BTreeSet<StateId>>,
) -> BTreeSet<StateId> {
    let mut successors = state.successors().iter().copied().collect::<BTreeSet<_>>();
    if let Some(routed) = routing_successors.get(&state.state_id()) {
        successors.extend(routed.iter().copied());
    }
    successors
}

fn solve_live_out(
    state_graph: &LateLoweredStateGraph,
    defs: &BTreeMap<StateId, BTreeSet<LocalId>>,
    uses_before_def: &BTreeMap<StateId, BTreeSet<LocalId>>,
    routing_successors: &BTreeMap<StateId, BTreeSet<StateId>>,
) -> BTreeMap<StateId, BTreeSet<LocalId>> {
    let mut live_in = BTreeMap::<StateId, BTreeSet<LocalId>>::new();
    let mut live_out = BTreeMap::<StateId, BTreeSet<LocalId>>::new();

    let mut changed = true;
    while changed {
        changed = false;
        for state in state_graph.states().iter().rev() {
            let out = state_successors_with_routing(state, routing_successors)
                .iter()
                .flat_map(|successor| {
                    live_in
                        .get(successor)
                        .into_iter()
                        .flat_map(|set| set.iter())
                })
                .copied()
                .collect::<BTreeSet<_>>();
            let mut input = uses_before_def
                .get(&state.state_id())
                .cloned()
                .unwrap_or_default();
            let defs = defs.get(&state.state_id()).cloned().unwrap_or_default();
            input.extend(out.iter().copied().filter(|local| !defs.contains(local)));

            if live_out.get(&state.state_id()) != Some(&out) {
                live_out.insert(state.state_id(), out);
                changed = true;
            }
            if live_in.get(&state.state_id()) != Some(&input) {
                live_in.insert(state.state_id(), input);
                changed = true;
            }
        }
    }

    live_out
}

fn collect_statement_uses_before_def(
    stmt: &crate::mir::Statement,
    defs: &mut BTreeSet<LocalId>,
    uses_before_def: &mut BTreeSet<LocalId>,
    read_states: &mut HashMap<LocalId, BTreeSet<StateId>>,
    state_id: StateId,
) {
    match &stmt.kind {
        StatementKind::Nop | StatementKind::Todo(_) => {}
        StatementKind::Assign { value, .. } => {
            collect_rvalue_uses(value, defs, uses_before_def, read_states, state_id)
        }
        StatementKind::StoreMember {
            receiver,
            value,
            continuation_route,
            ..
        } => {
            collect_operand_use(receiver, defs, uses_before_def, read_states, state_id);
            collect_operand_use(value, defs, uses_before_def, read_states, state_id);
            if let crate::mir::StoredContinuationRoutePublication::Unique(route) =
                continuation_route
            {
                collect_operand_use(
                    &Operand::Local(route.source_local),
                    defs,
                    uses_before_def,
                    read_states,
                    state_id,
                );
            }
        }
        StatementKind::StoreTopLevelVar { value, .. } => {
            collect_operand_use(value, defs, uses_before_def, read_states, state_id);
        }
    }
}

fn collect_terminator_uses_before_def(
    kind: &TerminatorKind,
    defs: &mut BTreeSet<LocalId>,
    uses_before_def: &mut BTreeSet<LocalId>,
    read_states: &mut HashMap<LocalId, BTreeSet<StateId>>,
    state_id: StateId,
) {
    match kind {
        TerminatorKind::Return { value } => {
            if let Some(value) = value {
                collect_operand_use(value, defs, uses_before_def, read_states, state_id);
            }
        }
        TerminatorKind::CondBr { cond, .. } => {
            collect_operand_use(cond, defs, uses_before_def, read_states, state_id);
        }
        TerminatorKind::Perform { args, .. } => {
            for arg in args {
                collect_operand_use(&arg.value, defs, uses_before_def, read_states, state_id);
            }
        }
        TerminatorKind::Goto { .. }
        | TerminatorKind::Handle { .. }
        | TerminatorKind::ResumeUnwind
        | TerminatorKind::Unreachable
        | TerminatorKind::Todo(_) => {}
    }
}

fn collect_rvalue_uses(
    value: &Rvalue,
    defs: &BTreeSet<LocalId>,
    uses_before_def: &mut BTreeSet<LocalId>,
    read_states: &mut HashMap<LocalId, BTreeSet<StateId>>,
    state_id: StateId,
) {
    match value {
        Rvalue::Use(operand)
        | Rvalue::Transport { value: operand, .. }
        | Rvalue::Unary { operand, .. }
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
        } => collect_operand_use(operand, defs, uses_before_def, read_states, state_id),
        Rvalue::Binary { lhs, rhs, .. } => {
            collect_operand_use(lhs, defs, uses_before_def, read_states, state_id);
            collect_operand_use(rhs, defs, uses_before_def, read_states, state_id);
        }
        Rvalue::MemberAccess { receiver, .. } => {
            collect_operand_use(receiver, defs, uses_before_def, read_states, state_id);
        }
        Rvalue::Call { kind, args, .. } => {
            collect_call_kind_uses(kind, defs, uses_before_def, read_states, state_id);
            for arg in args {
                collect_operand_use(&arg.value, defs, uses_before_def, read_states, state_id);
            }
        }
        Rvalue::EnumVariant { args, .. } => {
            for arg in args {
                collect_operand_use(&arg.value, defs, uses_before_def, read_states, state_id);
            }
        }
        Rvalue::ClassCtor { args, .. } => {
            for arg in args {
                collect_operand_use(&arg.value, defs, uses_before_def, read_states, state_id);
            }
        }
        Rvalue::MakeTuple { elements, .. } => {
            for element in elements {
                collect_operand_use(element, defs, uses_before_def, read_states, state_id);
            }
        }
        Rvalue::StructLit { fields, .. } => {
            for field in fields {
                collect_operand_use(&field.value, defs, uses_before_def, read_states, state_id);
            }
        }
        Rvalue::InterpolatedString { parts, .. } => {
            for part in parts {
                if let crate::mir::InterpolatedStringPart::Expr { value, .. } = part {
                    collect_operand_use(value, defs, uses_before_def, read_states, state_id);
                }
            }
        }
        Rvalue::CaptureBoxSet {
            box_operand, value, ..
        } => {
            collect_operand_use(box_operand, defs, uses_before_def, read_states, state_id);
            collect_operand_use(value, defs, uses_before_def, read_states, state_id);
        }
        Rvalue::MakeClosure { env, .. } => {
            collect_operand_use(env, defs, uses_before_def, read_states, state_id);
        }
        Rvalue::TopLevelRef(_)
        | Rvalue::UnresolvedName { .. }
        | Rvalue::SizeOf { .. }
        | Rvalue::TypeMetadataLiteral(_)
        | Rvalue::PerformResult { .. }
        | Rvalue::Todo(_) => {}
    }
}

fn collect_call_kind_uses(
    kind: &CallKind,
    defs: &BTreeSet<LocalId>,
    uses_before_def: &mut BTreeSet<LocalId>,
    read_states: &mut HashMap<LocalId, BTreeSet<StateId>>,
    state_id: StateId,
) {
    match kind {
        CallKind::Direct { .. } => {}
        CallKind::Closure { callee, .. }
        | CallKind::FunValue { callee }
        | CallKind::FunPtr { callee } => {
            collect_operand_use(callee, defs, uses_before_def, read_states, state_id);
        }
        CallKind::Virtual { receiver, .. } | CallKind::Interface { receiver, .. } => {
            collect_operand_use(receiver, defs, uses_before_def, read_states, state_id);
        }
        CallKind::Resume { continuation, .. } => {
            collect_operand_use(continuation, defs, uses_before_def, read_states, state_id);
        }
    }
}

fn collect_operand_use(
    operand: &Operand,
    defs: &BTreeSet<LocalId>,
    uses_before_def: &mut BTreeSet<LocalId>,
    read_states: &mut HashMap<LocalId, BTreeSet<StateId>>,
    state_id: StateId,
) {
    let Operand::Local(local) = operand else {
        return;
    };
    read_states.entry(*local).or_default().insert(state_id);
    if !defs.contains(local) {
        uses_before_def.insert(*local);
    }
}

fn reachable_case_tags(
    step_schema: &StepSchema,
    resolved_outward_cases: &[CaseTag],
    impl_plan: ImplPlan,
) -> Vec<CaseTag> {
    let resolved = resolved_outward_cases
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    step_schema
        .cases()
        .iter()
        .filter_map(|case| {
            let case_tag = case.case_tag();
            if !resolved.contains(&case_tag) {
                return None;
            }
            match impl_plan {
                ImplPlan::NoOutward => None,
                ImplPlan::SingleCase(selected) if selected == case_tag => Some(case_tag),
                ImplPlan::SingleCase(_) => None,
                ImplPlan::CanonicalFull => Some(case_tag),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::effect_lowered::ir::SystemSlotKind;
    use crate::pipeline::load_effect_lowered_stage_output_for_dump;
    use crate::session::{Session, SessionOptions};
    use crate::source::SourceFile;

    fn session() -> Session {
        Session::with_options(SessionOptions::new()).unwrap()
    }

    fn load_output(source: &SourceFile) -> crate::pipeline::EffectLoweredStageOutput {
        let session = session();
        load_effect_lowered_stage_output_for_dump(&session, source)
            .expect("fixture 应可通过 late-lowering stage")
    }

    fn callable<'a>(
        output: &'a crate::pipeline::EffectLoweredStageOutput,
        fqn: &str,
    ) -> &'a crate::effect_lowered::LateLoweredCallable {
        output
            .program()
            .callable(fqn)
            .unwrap_or_else(|| panic!("late-lowered program 应发布 {fqn}"))
    }

    #[test]
    fn frame_lifting_lifts_locals_temporaries_resume_slots_and_system_fields() {
        let output = load_output(&SourceFile::new_virtual(
            "<mem>/effect_lowered_expr_context.scoop",
            r#"
package sample

effect Step {
    fun next(seed: Int): Int
}

fun box_int(value: Int): Int {
    return value
}

fun helper(seed: Int): Int / Step {
    val handled: Int = handle {
        Step.next(seed)
    } with {
        Step.next(value), k -> k.resume(value + 1)
    }
    val via_arg: Int = box_int((seed + 1) + Step.next(handled))
    return via_arg + seed
}

fun main(): Int {
    return 0
}
"#,
        ));
        let callable = callable(&output, "sample.helper");
        let dump = callable.frame_schema().slots();

        assert!(
            dump.iter().any(|slot| matches!(
                slot.kind(),
                crate::effect_lowered::ir::LateLoweredFrameSlotKind::SourceLocal(_)
            )),
            "frame lifting 应包含 source local slot"
        );
        assert!(
            dump.iter().any(|slot| matches!(
                slot.kind(),
                crate::effect_lowered::ir::LateLoweredFrameSlotKind::CompilerTemporary(_)
            )),
            "frame lifting 应包含 compiler temp slot"
        );
        assert!(
            dump.iter().any(|slot| matches!(
                slot.kind(),
                crate::effect_lowered::ir::LateLoweredFrameSlotKind::ResumePayload { .. }
            )),
            "frame lifting 应包含 resume payload slot"
        );
        assert!(
            dump.iter().any(|slot| matches!(
                slot.kind(),
                crate::effect_lowered::ir::LateLoweredFrameSlotKind::BoundaryResult { .. }
            )),
            "frame lifting 应包含 boundary result slot\n{}",
            output.program().stable_dump()
        );
        for system in [
            SystemSlotKind::StateTag,
            SystemSlotKind::ResumePayloadCarrier,
            SystemSlotKind::CleanupFlag,
            SystemSlotKind::OneShotFlag,
            SystemSlotKind::CompletionTag,
            SystemSlotKind::CurrentEffectCtx,
        ] {
            assert!(
                callable
                    .frame_schema()
                    .slot_for_kind(crate::effect_lowered::ir::LateLoweredFrameSlotKind::System(
                        system,
                    ))
                    .is_some(),
                "frame lifting 应包含系统槽位 {system:?}"
            );
        }
        assert!(
            dump.iter().any(|slot| matches!(
                slot.kind(),
                crate::effect_lowered::ir::LateLoweredFrameSlotKind::HandleSavedEffectCtx { .. }
            )),
            "frame lifting 应包含 handle saved EffectCtx slot"
        );
        assert!(
            dump.iter().any(|slot| matches!(
                slot.kind(),
                crate::effect_lowered::ir::LateLoweredFrameSlotKind::HandleArmEffectCtx { .. }
            )),
            "frame lifting 应包含 handle arm derived EffectCtx slot"
        );
        assert!(
            output
                .program()
                .continuation_object(callable.continuation_object())
                .expect("callable 应能回查 continuation shell")
                .captures()
                .iter()
                .any(|capture| matches!(
                    capture,
                    crate::effect_lowered::ir::LateLoweredContinuationCapture::FrameSlot(_)
                )),
            "continuation captures 应显式引用 lifted frame slots"
        );
    }

    #[test]
    fn frame_lifting_uses_stable_mir_local_source_metadata() {
        let output = load_output(&SourceFile::new_virtual(
            "<mem>/effect_lowered_tmp_named_source_local.scoop",
            r#"
package sample

effect Step {
    fun next(seed: Int): Int
}

fun box_int(value: Int): Int {
    return value
}

fun helper(seed: Int): Int / Step {
    val tmp_seed: Int = seed
    val via_arg: Int = box_int((seed + 1) + Step.next(tmp_seed))
    return via_arg + tmp_seed
}

fun main(): Int {
    return 0
}
"#,
        ));
        let callable = callable(&output, "sample.helper");
        let pass_view = output.materialized_pass_view();
        let mir_fun = pass_view
            .callable("sample.helper")
            .expect("sample.helper 应保留 canonical MIR body");
        let body = mir_fun.body.as_ref().expect("sample.helper 应有 MIR body");

        let (tmp_seed_local, tmp_seed_decl) = body
            .locals
            .iter()
            .enumerate()
            .find_map(|(idx, decl)| {
                (decl.name.as_deref() == Some("tmp_seed"))
                    .then_some((crate::mir::LocalId::from_raw(idx as u32), decl))
            })
            .expect("fixture 应包含源码 local `tmp_seed`");
        assert_eq!(
            tmp_seed_decl.source,
            crate::mir::LocalSourceKind::SourceLocal,
            "canonical MIR 必须把源码 tmp* local 标记为 SourceLocal"
        );
        assert!(
            callable
                .frame_schema()
                .slot_for_kind(
                    crate::effect_lowered::ir::LateLoweredFrameSlotKind::SourceLocal(
                        tmp_seed_local,
                    )
                )
                .is_some(),
            "源码 tmp* local 进入 frame 后仍应保持 SourceLocal\n{}",
            output.program().stable_dump()
        );
        assert!(
            callable
                .frame_schema()
                .slot_for_kind(
                    crate::effect_lowered::ir::LateLoweredFrameSlotKind::CompilerTemporary(
                        tmp_seed_local,
                    ),
                )
                .is_none(),
            "源码 tmp* local 不应仅因名字前缀被误判为 CompilerTemporary\n{}",
            output.program().stable_dump()
        );

        let lifted_compiler_temp = body
            .locals
            .iter()
            .enumerate()
            .find_map(|(idx, decl)| {
                if decl.source != crate::mir::LocalSourceKind::CompilerTemporary {
                    return None;
                }
                let local = crate::mir::LocalId::from_raw(idx as u32);
                callable
                    .frame_schema()
                    .slot_for_kind(
                        crate::effect_lowered::ir::LateLoweredFrameSlotKind::CompilerTemporary(
                            local,
                        ),
                    )
                    .map(|_| local)
            })
            .expect("fixture 应至少 lift 一个真正的 compiler temporary");
        assert_eq!(
            body.locals[lifted_compiler_temp.as_u32() as usize].source,
            crate::mir::LocalSourceKind::CompilerTemporary,
            "frame 中的 compiler temporary slot 必须来自稳定的 MIR 来源元数据"
        );
    }

    #[test]
    fn frame_lifting_marks_handle_binders_that_cross_nested_boundaries() {
        let output = load_output(&SourceFile::new_virtual(
            "<mem>/effect_lowered_handle_binder.scoop",
            r#"
package sample

effect Step {
    fun next(seed: Int): Int
}

effect Outer {
    fun ping(seed: Int): Int
}

fun cleanup() {}

fun helper(): Int / Outer {
    return handle {
        Step.next(1)
        0
    } with {
        Step.next(value) -> {
            Outer.ping(value)
            value
        }
    } finally {
        cleanup()
    }
}

fun main(): Int {
    return 0
}
"#,
        ));
        let callable = callable(&output, "sample.helper");

        assert!(
            callable.frame_schema().slots().iter().any(|slot| matches!(
                slot.kind(),
                crate::effect_lowered::ir::LateLoweredFrameSlotKind::HandleBinder { .. }
            )),
            "穿过后续 boundary 的 handler binder 应被显式 lift\n{}",
            output.program().stable_dump()
        );
    }

    #[test]
    fn frame_lifting_captures_locals_used_by_routed_handle_arm() {
        let output = load_output(&SourceFile::new_virtual(
            "<mem>/effect_lowered_nested_arm_replay_capture.scoop",
            r#"
package sample

import scoop.core.*

effect Inner {
    fun enter(): Int
}

effect Boom {
    fun next(): Int
}

class Cell(var saved: Continuation<Int, Int>?)

fun start(cell: Cell): Int {
    return handle {
        val nested: Int = handle {
            val x: Int = Inner.enter()
            val y: Int = Boom.next()
            x + y
        } with {
            Inner.enter(), k -> {
                val resumed: Int = try {
                    k.resume(7)
                } catch (e: RuntimeError) {
                    0
                }
                resumed + 1
            }
        }
        nested + 100
    } with {
        Boom.next(), k -> {
            cell.saved = Some(k)
            18
        }
    }
}

fun main() {}
"#,
        ));
        let callable = callable(&output, "sample.start");
        let pass_view = output.materialized_pass_view();
        let mir_fun = pass_view
            .callable("sample.start")
            .expect("sample.start 应保留 canonical MIR body");
        let cell_param = mir_fun
            .params
            .first()
            .expect("start 应包含 cell 参数")
            .local;

        assert!(
            callable
                .frame_schema()
                .slot_for_kind(
                    crate::effect_lowered::ir::LateLoweredFrameSlotKind::SourceLocal(cell_param,)
                )
                .is_some(),
            "boundary outward 被外层 handle arm 消费时，该 arm 读取的 source local 应被 lift 到 continuation frame\n{}",
            output.program().stable_dump()
        );
    }

    #[test]
    fn frame_lifting_marks_phi_like_join_values_that_cross_later_boundaries() {
        let output = load_output(&SourceFile::new_virtual(
            "<mem>/effect_lowered_join_value.scoop",
            r#"
package sample

effect Step {
    fun next(seed: Int): Int
}

fun helper(): Int / Step {
    val cond: Bool = true
    var merged: Int = 0
    if (cond) {
        merged = 1
    } else {
        merged = 2
    }

    Step.next(merged)
    return merged
}

fun main(): Int {
    return 0
}
"#,
        ));
        let callable = callable(&output, "sample.helper");

        assert!(
            callable.frame_schema().slots().iter().any(|slot| matches!(
                slot.kind(),
                crate::effect_lowered::ir::LateLoweredFrameSlotKind::JoinValue { .. }
            )),
            "跨后续 boundary 继续读取的合流值应被标成 JoinValue slot\n{}",
            output.program().stable_dump()
        );
    }
}
