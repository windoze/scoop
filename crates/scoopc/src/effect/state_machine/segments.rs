type HandleSegmentId = PlanStateId;

#[derive(Debug, Clone)]
pub(crate) struct HandleSegmentList {
    handle_span: Span,
    result_ty: TypeId,
    entry_segment: HandleSegmentId,
    frame_slots: Vec<FrameSlot>,
    lifted_locals: Vec<hir::SymbolId>,
    dispatch_entries: Vec<HandleSegmentDispatchEntry>,
    arm_bodies: Vec<HandleSegmentArmBody>,
    segments: Vec<HandleSegment>,
    edges: Vec<HandleSegmentEdge>,
    suspend_sites: Vec<HandleSegmentSuspendSite>,
    cleanup_scopes: Vec<HandleSegmentCleanupScope>,
    nested_handles: Vec<HandleSegmentList>,
}

#[derive(Debug, Clone)]
struct HandleSegmentDispatchEntry {
    op_fqn: String,
    arm_ids: Vec<ArmPlanId>,
    targets: Vec<HandleSegmentDispatchTarget>,
}

#[derive(Debug, Clone)]
struct HandleSegmentDispatchTarget {
    arm_id: ArmPlanId,
    entry_segment: HandleSegmentId,
}

#[derive(Debug, Clone)]
struct HandleSegmentArmBody {
    arm_id: ArmPlanId,
    op_fqn: String,
    effect_ty: TypeId,
    body_entry_segment: HandleSegmentId,
    body_segments: Vec<HandleSegmentId>,
    binder_slots: Vec<hir::SymbolId>,
    capture_locals: Vec<hir::SymbolId>,
    body_may_suspend_outward: bool,
    cleanup_scope_stack: Vec<CleanupScopeId>,
}

#[derive(Debug, Clone)]
struct HandleSegment {
    id: HandleSegmentId,
    label: String,
    source_span: Option<Span>,
    dispatch_context: HandleSegmentDispatchContext,
    cleanup_scope_stack: Vec<CleanupScopeId>,
    ops: Vec<HandleStateOp>,
    terminator: HandleSegmentTerminator,
}

#[derive(Debug, Clone, Copy)]
enum HandleSegmentDispatchContext {
    Main,
    Cleanup {
        scope_id: CleanupScopeId,
        kind: CleanupScopeKind,
    },
    Arm {
        arm_id: ArmPlanId,
    },
}

#[derive(Debug, Clone)]
enum HandleSegmentTerminator {
    Goto {
        next_segment: HandleSegmentId,
    },
    Branch {
        condition: HandleBranchCondition,
        then_segment: HandleSegmentId,
        else_segment: HandleSegmentId,
        merge_segment: HandleSegmentId,
    },
    Suspend {
        site_id: SuspendSiteId,
        resume_segment: HandleSegmentId,
    },
    CleanupEnter {
        scope_id: CleanupScopeId,
        next_segment: HandleSegmentId,
    },
    ReturnHandle,
    ReturnFromFunction,
    ArmExit {
        exit: ArmBodyExit,
    },
}

#[derive(Debug, Clone)]
struct HandleSegmentEdge {
    from: HandleSegmentId,
    to: HandleSegmentId,
    kind: HandleSegmentEdgeKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum HandleSegmentEdgeKind {
    Goto,
    BranchThen,
    BranchElse,
    SuspendResume,
    CleanupEnter,
}

#[derive(Debug, Clone)]
struct HandleSegmentSuspendSite {
    id: SuspendSiteId,
    span: Span,
    kind: SuspendSiteKind,
    owner_segment: HandleSegmentId,
    resume_segment: HandleSegmentId,
    escape_resume_segment: Option<HandleSegmentId>,
    matching_arms: Vec<ArmPlanId>,
    available_locals: Vec<hir::SymbolId>,
    capture_locals: Vec<hir::SymbolId>,
    source_path: Option<SuspendSourcePath>,
    resume_path: Option<SuspendResumePath>,
    continuation_escape: ContinuationEscapeState,
}

#[derive(Debug, Clone)]
struct HandleSegmentCleanupScope {
    id: CleanupScopeId,
    kind: CleanupScopeKind,
    entry_segment: HandleSegmentId,
    exit_segment: HandleSegmentId,
    note: String,
}

impl HandleStateMachinePlan {
    fn build_segment_list(&self) -> HandleSegmentList {
        HandleSegmentList::from_plan(self)
    }

    fn build_from_segments(segment_list: &HandleSegmentList) -> Result<Self, String> {
        segment_list.build_state_machine_plan()
    }
}

impl HandleSegmentList {
    // T2003r2: the unified builder stage must reconstruct the full
    // HandleStateMachinePlan from the frozen segment contract alone.
    fn build_state_machine_plan(&self) -> Result<HandleStateMachinePlan, String> {
        self.validate_builder_contract()?;

        let frame_slots = self
            .frame_slots
            .iter()
            .cloned()
            .map(|slot| (slot.id, slot))
            .collect::<HashMap<_, _>>();

        let lifted_locals = self
            .lifted_locals
            .iter()
            .map(|local_id| {
                frame_slots.get(local_id).cloned().ok_or_else(|| {
                    format!(
                        "segment builder input missing slot metadata for lifted local local#{}",
                        local_id.as_u32()
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        let arm_plans = self
            .arm_bodies
            .iter()
            .map(|arm| arm.to_plan(&frame_slots))
            .collect::<Result<Vec<_>, _>>()?;

        let mut arm_binders = arm_plans
            .iter()
            .flat_map(|arm| arm.binder_slots.clone())
            .collect::<Vec<_>>();
        arm_binders.sort_by_key(|slot| (slot.owner_arm.unwrap_or(0), slot.id.as_u32()));

        let states = self
            .segments
            .iter()
            .map(HandleSegment::to_plan_state)
            .collect::<Vec<_>>();
        let suspend_sites = self
            .suspend_sites
            .iter()
            .map(HandleSegmentSuspendSite::to_plan)
            .collect::<Vec<_>>();
        let cleanup_scopes = self
            .cleanup_scopes
            .iter()
            .map(HandleSegmentCleanupScope::to_plan)
            .collect::<Vec<_>>();
        let nested_handles = self
            .nested_handles
            .iter()
            .map(Self::build_state_machine_plan)
            .collect::<Result<Vec<_>, _>>()?;

        let dispatch_plan = DispatchPlan {
            entries: self
                .dispatch_entries
                .iter()
                .map(HandleSegmentDispatchEntry::to_plan)
                .collect(),
        };
        let has_one_shot_flag = states.iter().any(|state| {
            matches!(
                state.terminator,
                StateTerminator::ArmExit(ArmBodyExit::MaterializeContinuation)
            )
        });

        Ok(HandleStateMachinePlan {
            handle_span: self.handle_span,
            result_ty: self.result_ty,
            entry_state: self.entry_segment,
            states,
            suspend_sites,
            arm_plans,
            cleanup_scopes,
            frame_layout: FrameLayoutPlan {
                slots: frame_slots,
                lifted_locals,
                arm_binders,
                has_cleanup_flag: !self.cleanup_scopes.is_empty(),
                has_one_shot_flag,
            },
            dispatch_plan,
            nested_handles,
        })
    }

    fn from_plan(plan: &HandleStateMachinePlan) -> Self {
        let segment_successors = build_segment_successor_map(&plan.states, &plan.suspend_sites);
        let state_cleanup_execution_scopes = build_state_cleanup_execution_scopes(
            &plan.cleanup_scopes,
            &plan.states,
            &segment_successors,
        );
        let state_cleanup_scope_stacks = build_state_cleanup_scope_stacks(
            &plan.cleanup_scopes,
            &plan.states,
            &state_cleanup_execution_scopes,
        );
        let arm_bodies = build_segment_arm_bodies(
            &plan.arm_plans,
            &plan.states,
            &segment_successors,
            &state_cleanup_scope_stacks,
        );
        let state_dispatch_contexts = build_state_dispatch_contexts(
            &plan.states,
            &plan.cleanup_scopes,
            &state_cleanup_execution_scopes,
            &arm_bodies,
        );
        let dispatch_entries = build_segment_dispatch_entries(&plan.dispatch_plan, &arm_bodies);
        let suspend_sites = plan
            .suspend_sites
            .iter()
            .map(HandleSegmentSuspendSite::from_plan)
            .collect::<Vec<_>>();
        let resume_targets = suspend_sites
            .iter()
            .map(|site| (site.id, site.resume_segment))
            .collect::<HashMap<_, _>>();

        let segments = plan
            .states
            .iter()
            .map(|state| {
                HandleSegment::from_plan_state(
                    state,
                    plan.handle_span,
                    &suspend_sites,
                    &resume_targets,
                    *state_dispatch_contexts
                        .get(&state.id)
                        .unwrap_or(&HandleSegmentDispatchContext::Main),
                    state_cleanup_scope_stacks
                        .get(&state.id)
                        .cloned()
                        .unwrap_or_default(),
                )
            })
            .collect::<Vec<_>>();
        let edges = segments
            .iter()
            .flat_map(HandleSegment::outgoing_edges)
            .collect::<Vec<_>>();
        let cleanup_scopes = plan
            .cleanup_scopes
            .iter()
            .map(HandleSegmentCleanupScope::from_plan)
            .collect();
        let nested_handles = plan
            .nested_handles
            .iter()
            .map(Self::from_plan)
            .collect::<Vec<_>>();

        Self {
            handle_span: plan.handle_span,
            result_ty: plan.result_ty,
            entry_segment: plan.entry_state,
            frame_slots: build_segment_frame_slots(&plan.frame_layout),
            lifted_locals: plan
                .frame_layout
                .lifted_locals
                .iter()
                .map(|slot| slot.id)
                .collect(),
            dispatch_entries,
            arm_bodies,
            segments,
            edges,
            suspend_sites,
            cleanup_scopes,
            nested_handles,
        }
    }

    fn structural_signature(&self) -> usize {
        let mut acc = self.handle_span.start
            ^ self.handle_span.end
            ^ self.result_ty.as_u32() as usize
            ^ self.entry_segment as usize;
        for slot in &self.frame_slots {
            acc ^= slot.structural_signature();
        }
        for local_id in &self.lifted_locals {
            acc ^= (local_id.as_u32() as usize) << 6;
        }
        for dispatch_entry in &self.dispatch_entries {
            acc ^= dispatch_entry.structural_signature();
        }
        for arm_body in &self.arm_bodies {
            acc ^= arm_body.structural_signature();
        }
        for segment in &self.segments {
            acc ^= segment.structural_signature();
        }
        for edge in &self.edges {
            acc ^= edge.structural_signature();
        }
        for site in &self.suspend_sites {
            acc ^= site.structural_signature();
        }
        for scope in &self.cleanup_scopes {
            acc ^= scope.structural_signature();
        }
        for nested in &self.nested_handles {
            acc ^= nested.structural_signature();
        }
        acc
    }

    // Freeze the phase-1 segment contract before T2003r2 switches the builder
    // to consume HandleSegmentList as its only input.
    fn validate_builder_contract(&self) -> Result<(), String> {
        self.validate_builder_contract_with_path("root")
    }

    fn validate_builder_contract_with_path(&self, path: &str) -> Result<(), String> {
        let mut frame_slots_by_id = HashMap::<hir::SymbolId, &FrameSlot>::new();
        let mut previous_slot_id = None::<hir::SymbolId>;
        for slot in &self.frame_slots {
            if let Some(prev_id) = previous_slot_id
                && prev_id.as_u32() >= slot.id.as_u32()
            {
                return Err(format!(
                    "{path}: frame_slots[] is not strictly sorted by symbol id at {}",
                    slot.display_name()
                ));
            }
            previous_slot_id = Some(slot.id);
            if frame_slots_by_id.insert(slot.id, slot).is_some() {
                return Err(format!(
                    "{path}: duplicate frame slot {}",
                    slot.display_name()
                ));
            }
        }

        let mut lifted_local_ids = HashSet::<hir::SymbolId>::new();
        let mut previous_lifted_id = None::<hir::SymbolId>;
        for local_id in &self.lifted_locals {
            if let Some(prev_id) = previous_lifted_id
                && prev_id.as_u32() >= local_id.as_u32()
            {
                return Err(format!(
                    "{path}: lifted_locals[] is not strictly sorted by symbol id at {}",
                    describe_segment_local(*local_id, &frame_slots_by_id)
                ));
            }
            previous_lifted_id = Some(*local_id);
            if !lifted_local_ids.insert(*local_id) {
                return Err(format!(
                    "{path}: lifted_locals[] repeats {}",
                    describe_segment_local(*local_id, &frame_slots_by_id)
                ));
            }
            let slot = frame_slots_by_id.get(local_id).ok_or_else(|| {
                format!(
                    "{path}: lifted_locals[] references missing slot metadata for {}",
                    describe_segment_local(*local_id, &frame_slots_by_id)
                )
            })?;
            if let Some(owner_arm) = slot.owner_arm {
                return Err(format!(
                    "{path}: lifted_locals[] contains binder {} owned by arm{}",
                    slot.display_name(),
                    owner_arm
                ));
            }
        }

        let mut segment_by_id = HashMap::<HandleSegmentId, &HandleSegment>::new();
        let mut previous_segment_id = None::<HandleSegmentId>;
        for segment in &self.segments {
            if let Some(prev_id) = previous_segment_id
                && prev_id >= segment.id
            {
                return Err(format!(
                    "{path}: segments[] is not strictly sorted by segment id at seg{}",
                    segment.id
                ));
            }
            previous_segment_id = Some(segment.id);
            if segment_by_id.insert(segment.id, segment).is_some() {
                return Err(format!("{path}: duplicate segment id seg{}", segment.id));
            }
        }
        if !segment_by_id.contains_key(&self.entry_segment) {
            return Err(format!(
                "{path}: entry segment seg{} is missing from segments[]",
                self.entry_segment
            ));
        }

        let mut arm_bodies_by_id = HashMap::<ArmPlanId, &HandleSegmentArmBody>::new();
        let mut previous_arm_id = None::<ArmPlanId>;
        for arm in &self.arm_bodies {
            if let Some(prev_id) = previous_arm_id
                && prev_id >= arm.arm_id
            {
                return Err(format!(
                    "{path}: arm_bodies[] is not strictly sorted by arm id at arm{}",
                    arm.arm_id
                ));
            }
            previous_arm_id = Some(arm.arm_id);
            if arm_bodies_by_id.insert(arm.arm_id, arm).is_some() {
                return Err(format!("{path}: duplicate arm body arm{}", arm.arm_id));
            }
        }

        let mut binder_slot_ids = HashSet::<hir::SymbolId>::new();
        let mut expected_lifted_ids = HashSet::<hir::SymbolId>::new();

        let mut cleanup_scopes_by_id = HashMap::<CleanupScopeId, &HandleSegmentCleanupScope>::new();
        let mut previous_cleanup_scope_id = None::<CleanupScopeId>;
        for scope in &self.cleanup_scopes {
            if let Some(prev_id) = previous_cleanup_scope_id
                && prev_id >= scope.id
            {
                return Err(format!(
                    "{path}: cleanup_scopes[] is not strictly sorted by cleanup id at cleanup{}",
                    scope.id
                ));
            }
            previous_cleanup_scope_id = Some(scope.id);
            if cleanup_scopes_by_id.insert(scope.id, scope).is_some() {
                return Err(format!(
                    "{path}: duplicate cleanup scope cleanup{}",
                    scope.id
                ));
            }
        }

        let mut suspend_sites_by_id = HashMap::<SuspendSiteId, &HandleSegmentSuspendSite>::new();
        let mut previous_suspend_site_id = None::<SuspendSiteId>;
        for site in &self.suspend_sites {
            if let Some(prev_id) = previous_suspend_site_id
                && prev_id >= site.id
            {
                return Err(format!(
                    "{path}: suspend_sites[] is not strictly sorted by site id at site{}",
                    site.id
                ));
            }
            previous_suspend_site_id = Some(site.id);
            if suspend_sites_by_id.insert(site.id, site).is_some() {
                return Err(format!("{path}: duplicate suspend site site{}", site.id));
            }
        }

        let mut edge_keys =
            HashSet::<(HandleSegmentId, HandleSegmentId, HandleSegmentEdgeKind)>::new();
        for edge in &self.edges {
            if !segment_by_id.contains_key(&edge.from) {
                return Err(format!(
                    "{path}: edge source seg{} is missing from segments[]",
                    edge.from
                ));
            }
            if !segment_by_id.contains_key(&edge.to) {
                return Err(format!(
                    "{path}: edge target seg{} is missing from segments[]",
                    edge.to
                ));
            }
            if !edge_keys.insert((edge.from, edge.to, edge.kind)) {
                return Err(format!(
                    "{path}: duplicate edge seg{} -{}-> seg{}",
                    edge.from,
                    edge.kind.label(),
                    edge.to
                ));
            }

            let from_segment = segment_by_id
                .get(&edge.from)
                .expect("validated edge source should exist");
            let matches_terminator = match &from_segment.terminator {
                HandleSegmentTerminator::Goto { next_segment } => {
                    edge.kind == HandleSegmentEdgeKind::Goto && edge.to == *next_segment
                }
                HandleSegmentTerminator::Branch {
                    then_segment,
                    else_segment,
                    ..
                } => {
                    (edge.kind == HandleSegmentEdgeKind::BranchThen && edge.to == *then_segment)
                        || (edge.kind == HandleSegmentEdgeKind::BranchElse
                            && edge.to == *else_segment)
                }
                HandleSegmentTerminator::Suspend { resume_segment, .. } => {
                    edge.kind == HandleSegmentEdgeKind::SuspendResume && edge.to == *resume_segment
                }
                HandleSegmentTerminator::CleanupEnter { next_segment, .. } => {
                    edge.kind == HandleSegmentEdgeKind::CleanupEnter && edge.to == *next_segment
                }
                HandleSegmentTerminator::ReturnHandle
                | HandleSegmentTerminator::ReturnFromFunction
                | HandleSegmentTerminator::ArmExit { .. } => false,
            };
            if !matches_terminator {
                return Err(format!(
                    "{path}: edge seg{} -{}-> seg{} does not match terminator {}",
                    edge.from,
                    edge.kind.label(),
                    edge.to,
                    from_segment.terminator.label()
                ));
            }
        }

        let mut dispatched_arm_ids = HashSet::<ArmPlanId>::new();
        let mut dispatch_ops = HashSet::<&str>::new();
        let mut previous_dispatch_op = None::<&str>;
        for entry in &self.dispatch_entries {
            if let Some(prev_op) = previous_dispatch_op
                && prev_op >= entry.op_fqn.as_str()
            {
                return Err(format!(
                    "{path}: dispatch_entries[] is not strictly sorted by op_fqn at {}",
                    entry.op_fqn
                ));
            }
            previous_dispatch_op = Some(entry.op_fqn.as_str());
            if !dispatch_ops.insert(entry.op_fqn.as_str()) {
                return Err(format!(
                    "{path}: duplicate dispatch entry for {}",
                    entry.op_fqn
                ));
            }
            if entry.arm_ids.len() != entry.targets.len() {
                return Err(format!(
                    "{path}: dispatch entry {} has {} arm ids but {} targets",
                    entry.op_fqn,
                    entry.arm_ids.len(),
                    entry.targets.len()
                ));
            }

            let mut arm_ids = HashSet::<ArmPlanId>::new();
            let mut previous_arm_id = None::<ArmPlanId>;
            for (idx, (arm_id, target)) in entry.arm_ids.iter().zip(&entry.targets).enumerate() {
                if let Some(prev_id) = previous_arm_id
                    && prev_id >= *arm_id
                {
                    return Err(format!(
                        "{path}: dispatch entry {} arm_ids[] is not strictly sorted at arm{}",
                        entry.op_fqn, arm_id
                    ));
                }
                previous_arm_id = Some(*arm_id);
                if !arm_ids.insert(*arm_id) {
                    return Err(format!(
                        "{path}: dispatch entry {} repeats arm{}",
                        entry.op_fqn, arm_id
                    ));
                }
                if *arm_id != target.arm_id {
                    return Err(format!(
                        "{path}: dispatch entry {} target#{idx} points to arm{} but arm_ids[{idx}] is arm{}",
                        entry.op_fqn, target.arm_id, arm_id
                    ));
                }

                let arm = arm_bodies_by_id.get(arm_id).ok_or_else(|| {
                    format!(
                        "{path}: dispatch entry {} references missing arm{}",
                        entry.op_fqn, arm_id
                    )
                })?;
                if arm.op_fqn != entry.op_fqn {
                    return Err(format!(
                        "{path}: dispatch entry {} points to arm{} for {}",
                        entry.op_fqn, arm_id, arm.op_fqn
                    ));
                }
                if target.entry_segment != arm.body_entry_segment {
                    return Err(format!(
                        "{path}: dispatch entry {} arm{} entry seg{} does not match arm body seg{}",
                        entry.op_fqn, arm_id, target.entry_segment, arm.body_entry_segment
                    ));
                }

                dispatched_arm_ids.insert(*arm_id);
            }
        }

        if dispatched_arm_ids.len() != arm_bodies_by_id.len() {
            let missing = arm_bodies_by_id
                .keys()
                .copied()
                .filter(|arm_id| !dispatched_arm_ids.contains(arm_id))
                .map(|arm_id| format!("arm{arm_id}"))
                .collect::<Vec<_>>()
                .join(", ");
            return Err(format!(
                "{path}: dispatch metadata is missing [{}]",
                missing
            ));
        }

        for scope in &self.cleanup_scopes {
            if !segment_by_id.contains_key(&scope.entry_segment) {
                return Err(format!(
                    "{path}: cleanup{} entry seg{} is missing from segments[]",
                    scope.id, scope.entry_segment
                ));
            }
            if !segment_by_id.contains_key(&scope.exit_segment) {
                return Err(format!(
                    "{path}: cleanup{} exit seg{} is missing from segments[]",
                    scope.id, scope.exit_segment
                ));
            }
        }

        for arm in &self.arm_bodies {
            if !segment_by_id.contains_key(&arm.body_entry_segment) {
                return Err(format!(
                    "{path}: arm{} entry seg{} is missing from segments[]",
                    arm.arm_id, arm.body_entry_segment
                ));
            }
            if !arm.body_segments.contains(&arm.body_entry_segment) {
                return Err(format!(
                    "{path}: arm{} body does not include its entry seg{}",
                    arm.arm_id, arm.body_entry_segment
                ));
            }

            let mut body_segment_ids = HashSet::<HandleSegmentId>::new();
            for segment_id in &arm.body_segments {
                if !body_segment_ids.insert(*segment_id) {
                    return Err(format!(
                        "{path}: arm{} body repeats seg{}",
                        arm.arm_id, segment_id
                    ));
                }
                let segment = segment_by_id.get(segment_id).ok_or_else(|| {
                    format!(
                        "{path}: arm{} body references missing seg{}",
                        arm.arm_id, segment_id
                    )
                })?;
                match segment.dispatch_context {
                    HandleSegmentDispatchContext::Arm { arm_id } if arm_id == arm.arm_id => {}
                    _ => {
                        return Err(format!(
                            "{path}: arm{} body segment seg{} has mismatched dispatch context {}",
                            arm.arm_id,
                            segment_id,
                            segment.dispatch_context.label()
                        ));
                    }
                }
                if segment.cleanup_scope_stack != arm.cleanup_scope_stack {
                    return Err(format!(
                        "{path}: arm{} body segment seg{} has cleanup stack [{}] but arm body expects [{}]",
                        arm.arm_id,
                        segment_id,
                        render_segment_cleanup_scope_ids(&segment.cleanup_scope_stack),
                        render_segment_cleanup_scope_ids(&arm.cleanup_scope_stack)
                    ));
                }
            }

            let mut arm_binder_ids = HashSet::<hir::SymbolId>::new();
            for local_id in &arm.binder_slots {
                if !arm_binder_ids.insert(*local_id) {
                    return Err(format!(
                        "{path}: arm{} binder metadata repeats {}",
                        arm.arm_id,
                        describe_segment_local(*local_id, &frame_slots_by_id)
                    ));
                }
                let slot = frame_slots_by_id.get(local_id).ok_or_else(|| {
                    format!(
                        "{path}: arm{} binder metadata references missing slot {}",
                        arm.arm_id,
                        describe_segment_local(*local_id, &frame_slots_by_id)
                    )
                })?;
                if slot.owner_arm != Some(arm.arm_id) {
                    let owner = slot.owner_arm.map_or_else(
                        || "handle-body".to_string(),
                        |owner_arm| format!("arm{owner_arm}"),
                    );
                    return Err(format!(
                        "{path}: arm{} binder {} is owned by {}",
                        arm.arm_id,
                        slot.display_name(),
                        owner
                    ));
                }
                binder_slot_ids.insert(*local_id);
            }

            let mut arm_capture_ids = HashSet::<hir::SymbolId>::new();
            for local_id in &arm.capture_locals {
                if !arm_capture_ids.insert(*local_id) {
                    return Err(format!(
                        "{path}: arm{} capture metadata repeats {}",
                        arm.arm_id,
                        describe_segment_local(*local_id, &frame_slots_by_id)
                    ));
                }
                if !frame_slots_by_id.contains_key(local_id) {
                    return Err(format!(
                        "{path}: arm{} capture metadata references missing slot {}",
                        arm.arm_id,
                        describe_segment_local(*local_id, &frame_slots_by_id)
                    ));
                }
                expected_lifted_ids.insert(*local_id);
            }
        }

        for slot in &self.frame_slots {
            if let Some(owner_arm) = slot.owner_arm
                && !binder_slot_ids.contains(&slot.id)
            {
                return Err(format!(
                    "{path}: frame slot {} owned by arm{} is missing from arm binder metadata",
                    slot.display_name(),
                    owner_arm
                ));
            }
        }

        for site in &self.suspend_sites {
            if !segment_by_id.contains_key(&site.owner_segment) {
                return Err(format!(
                    "{path}: site{} owner seg{} is missing from segments[]",
                    site.id, site.owner_segment
                ));
            }
            if !segment_by_id.contains_key(&site.resume_segment) {
                return Err(format!(
                    "{path}: site{} resume seg{} is missing from segments[]",
                    site.id, site.resume_segment
                ));
            }
            if let Some(escape_resume_segment) = site.escape_resume_segment
                && !segment_by_id.contains_key(&escape_resume_segment)
            {
                return Err(format!(
                    "{path}: site{} escape-resume seg{} is missing from segments[]",
                    site.id, escape_resume_segment
                ));
            }

            let owner = segment_by_id
                .get(&site.owner_segment)
                .expect("validated owner segment should exist");
            match owner.terminator {
                HandleSegmentTerminator::Suspend {
                    site_id,
                    resume_segment,
                } if site_id == site.id && resume_segment == site.resume_segment => {}
                _ => {
                    return Err(format!(
                        "{path}: site{} owner seg{} terminator does not point back to site",
                        site.id, site.owner_segment
                    ));
                }
            }

            let mut matching_arms = HashSet::<ArmPlanId>::new();
            for arm_id in &site.matching_arms {
                if !matching_arms.insert(*arm_id) {
                    return Err(format!(
                        "{path}: site{} repeats matching arm{}",
                        site.id, arm_id
                    ));
                }
                if !arm_bodies_by_id.contains_key(arm_id) {
                    return Err(format!(
                        "{path}: site{} references missing arm{}",
                        site.id, arm_id
                    ));
                }
            }

            match &site.kind {
                SuspendSiteKind::Perform { op_fqn } => {
                    for arm_id in &site.matching_arms {
                        let arm = arm_bodies_by_id
                            .get(arm_id)
                            .expect("validated arm should exist");
                        if arm.op_fqn != *op_fqn {
                            return Err(format!(
                                "{path}: site{} kind={} for {} matches arm{} for {}",
                                site.id,
                                describe_suspend_site_kind(&site.kind),
                                op_fqn,
                                arm_id,
                                arm.op_fqn
                            ));
                        }
                    }
                    if site.resume_path.is_none() {
                        return Err(format!(
                            "{path}: site{} kind={} missing resume_path metadata",
                            site.id,
                            describe_suspend_site_kind(&site.kind)
                        ));
                    }
                }
                SuspendSiteKind::RuntimeRaise { reason } => {
                    for arm_id in &site.matching_arms {
                        let arm = arm_bodies_by_id
                            .get(arm_id)
                            .expect("validated arm should exist");
                        if arm.op_fqn != "scoop.core.Raise.raise" {
                            return Err(format!(
                                "{path}: site{} kind={} must only match scoop.core.Raise.raise but arm{} handles {}",
                                site.id,
                                describe_suspend_site_kind(&site.kind),
                                arm_id,
                                arm.op_fqn
                            ));
                        }
                    }
                    if site.source_path.is_some() {
                        return Err(format!(
                            "{path}: site{} kind={} must not carry source_path metadata",
                            site.id,
                            describe_suspend_site_kind(&site.kind)
                        ));
                    }
                    if site.resume_path.is_some() && reason != "Continuation.resume" {
                        return Err(format!(
                            "{path}: site{} kind={} must not carry resume_path metadata unless it is Continuation.resume",
                            site.id,
                            describe_suspend_site_kind(&site.kind)
                        ));
                    }
                }
                SuspendSiteKind::CallMaySuspend { .. }
                | SuspendSiteKind::CallStateMachineCallee { .. }
                | SuspendSiteKind::ObjectInitAccess { .. }
                | SuspendSiteKind::TopLevelValueInitAccess { .. }
                | SuspendSiteKind::ClassCtorInit { .. }
                | SuspendSiteKind::NestedHandleBoundary { .. } => {
                    if !site.matching_arms.is_empty() {
                        let matching = site
                            .matching_arms
                            .iter()
                            .map(|arm_id| format!("arm{arm_id}"))
                            .collect::<Vec<_>>()
                            .join(", ");
                        return Err(format!(
                            "{path}: site{} kind={} must not list matching arms [{}]",
                            site.id,
                            describe_suspend_site_kind(&site.kind),
                            matching
                        ));
                    }
                    if matches!(
                        &site.kind,
                        SuspendSiteKind::CallMaySuspend { .. }
                            | SuspendSiteKind::CallStateMachineCallee { .. }
                            | SuspendSiteKind::ClassCtorInit { .. }
                            | SuspendSiteKind::NestedHandleBoundary { .. }
                    ) && site.resume_path.is_none()
                    {
                        return Err(format!(
                            "{path}: site{} kind={} missing resume_path metadata",
                            site.id,
                            describe_suspend_site_kind(&site.kind)
                        ));
                    }
                    if matches!(
                        &site.kind,
                        SuspendSiteKind::ObjectInitAccess { .. }
                            | SuspendSiteKind::TopLevelValueInitAccess { .. }
                    ) && site.resume_path.is_some()
                    {
                        return Err(format!(
                            "{path}: site{} kind={} must not carry resume_path metadata",
                            site.id,
                            describe_suspend_site_kind(&site.kind)
                        ));
                    }
                    if matches!(
                        &site.kind,
                        SuspendSiteKind::ObjectInitAccess { .. }
                            | SuspendSiteKind::TopLevelValueInitAccess { .. }
                    ) && site.source_path.is_some()
                    {
                        return Err(format!(
                            "{path}: site{} kind={} must not carry source_path metadata",
                            site.id,
                            describe_suspend_site_kind(&site.kind)
                        ));
                    }
                }
            }

            let mut available_locals = HashSet::<hir::SymbolId>::new();
            for local_id in &site.available_locals {
                if !available_locals.insert(*local_id) {
                    return Err(format!(
                        "{path}: site{} available local metadata repeats {}",
                        site.id,
                        describe_segment_local(*local_id, &frame_slots_by_id)
                    ));
                }
                if !frame_slots_by_id.contains_key(local_id) {
                    return Err(format!(
                        "{path}: site{} available local metadata references missing slot {}",
                        site.id,
                        describe_segment_local(*local_id, &frame_slots_by_id)
                    ));
                }
            }
            let mut capture_locals = HashSet::<hir::SymbolId>::new();
            for local_id in &site.capture_locals {
                if !capture_locals.insert(*local_id) {
                    return Err(format!(
                        "{path}: site{} capture metadata repeats {}",
                        site.id,
                        describe_segment_local(*local_id, &frame_slots_by_id)
                    ));
                }
                if !frame_slots_by_id.contains_key(local_id) {
                    return Err(format!(
                        "{path}: site{} capture metadata references missing slot {}",
                        site.id,
                        describe_segment_local(*local_id, &frame_slots_by_id)
                    ));
                }
                if !available_locals.contains(local_id) {
                    return Err(format!(
                        "{path}: site{} capture {} is not listed in available_locals",
                        site.id,
                        describe_segment_local(*local_id, &frame_slots_by_id)
                    ));
                }
                expected_lifted_ids.insert(*local_id);
            }
        }

        let mut missing_lifted = expected_lifted_ids
            .iter()
            .copied()
            .filter(|local_id| !lifted_local_ids.contains(local_id))
            .collect::<Vec<_>>();
        missing_lifted.sort_by_key(|id| id.as_u32());
        if !missing_lifted.is_empty() {
            return Err(format!(
                "{path}: lifted_locals[] is missing [{}]",
                render_segment_symbol_ids(&missing_lifted, &frame_slots_by_id)
            ));
        }

        let mut stale_lifted = lifted_local_ids
            .iter()
            .copied()
            .filter(|local_id| !expected_lifted_ids.contains(local_id))
            .collect::<Vec<_>>();
        stale_lifted.sort_by_key(|id| id.as_u32());
        if !stale_lifted.is_empty() {
            return Err(format!(
                "{path}: lifted_locals[] contains stale entries [{}]",
                render_segment_symbol_ids(&stale_lifted, &frame_slots_by_id)
            ));
        }

        for segment in &self.segments {
            for scope_id in &segment.cleanup_scope_stack {
                if !cleanup_scopes_by_id.contains_key(scope_id) {
                    return Err(format!(
                        "{path}: seg{} references missing cleanup{} in cleanup stack",
                        segment.id, scope_id
                    ));
                }
            }

            match &segment.terminator {
                HandleSegmentTerminator::Goto { next_segment } => {
                    if !segment_by_id.contains_key(next_segment) {
                        return Err(format!(
                            "{path}: seg{} goto target seg{} is missing from segments[]",
                            segment.id, next_segment
                        ));
                    }
                    if !edge_keys.contains(&(
                        segment.id,
                        *next_segment,
                        HandleSegmentEdgeKind::Goto,
                    )) {
                        return Err(format!(
                            "{path}: seg{} goto target seg{} is missing from edges[]",
                            segment.id, next_segment
                        ));
                    }
                }
                HandleSegmentTerminator::Branch {
                    then_segment,
                    else_segment,
                    merge_segment,
                    ..
                } => {
                    for target in [then_segment, else_segment, merge_segment] {
                        if !segment_by_id.contains_key(target) {
                            return Err(format!(
                                "{path}: seg{} branch target seg{} is missing from segments[]",
                                segment.id, target
                            ));
                        }
                    }
                    if !edge_keys.contains(&(
                        segment.id,
                        *then_segment,
                        HandleSegmentEdgeKind::BranchThen,
                    )) {
                        return Err(format!(
                            "{path}: seg{} missing branch-then edge to seg{}",
                            segment.id, then_segment
                        ));
                    }
                    if !edge_keys.contains(&(
                        segment.id,
                        *else_segment,
                        HandleSegmentEdgeKind::BranchElse,
                    )) {
                        return Err(format!(
                            "{path}: seg{} missing branch-else edge to seg{}",
                            segment.id, else_segment
                        ));
                    }
                }
                HandleSegmentTerminator::Suspend {
                    site_id,
                    resume_segment,
                } => {
                    let site = suspend_sites_by_id.get(site_id).ok_or_else(|| {
                        format!(
                            "{path}: seg{} suspend site{} is missing from suspend_sites[]",
                            segment.id, site_id
                        )
                    })?;
                    let is_escape_replay_segment = site.escape_resume_segment == Some(segment.id);
                    if site.owner_segment != segment.id && !is_escape_replay_segment {
                        return Err(format!(
                            "{path}: seg{} points to site{} but site owner is seg{}",
                            segment.id, site_id, site.owner_segment
                        ));
                    }
                    if site.resume_segment != *resume_segment {
                        return Err(format!(
                            "{path}: seg{} resume seg{} disagrees with site{} resume seg{}",
                            segment.id, resume_segment, site_id, site.resume_segment
                        ));
                    }
                    if !edge_keys.contains(&(
                        segment.id,
                        *resume_segment,
                        HandleSegmentEdgeKind::SuspendResume,
                    )) {
                        return Err(format!(
                            "{path}: seg{} missing suspend-resume edge to seg{}",
                            segment.id, resume_segment
                        ));
                    }
                }
                HandleSegmentTerminator::CleanupEnter {
                    scope_id,
                    next_segment,
                } => {
                    if !cleanup_scopes_by_id.contains_key(scope_id) {
                        return Err(format!(
                            "{path}: seg{} cleanup enter references missing cleanup{}",
                            segment.id, scope_id
                        ));
                    }
                    if !segment_by_id.contains_key(next_segment) {
                        return Err(format!(
                            "{path}: seg{} cleanup target seg{} is missing from segments[]",
                            segment.id, next_segment
                        ));
                    }
                    if !edge_keys.contains(&(
                        segment.id,
                        *next_segment,
                        HandleSegmentEdgeKind::CleanupEnter,
                    )) {
                        return Err(format!(
                            "{path}: seg{} missing cleanup-enter edge to seg{}",
                            segment.id, next_segment
                        ));
                    }
                }
                HandleSegmentTerminator::ReturnHandle
                | HandleSegmentTerminator::ReturnFromFunction
                | HandleSegmentTerminator::ArmExit { .. } => {}
            }

            match segment.dispatch_context {
                HandleSegmentDispatchContext::Main => {}
                HandleSegmentDispatchContext::Cleanup { scope_id, kind } => {
                    let scope = cleanup_scopes_by_id.get(&scope_id).ok_or_else(|| {
                        format!(
                            "{path}: seg{} cleanup-body references missing cleanup{}",
                            segment.id, scope_id
                        )
                    })?;
                    if scope.kind != kind {
                        return Err(format!(
                            "{path}: seg{} cleanup-body kind mismatch for cleanup{}",
                            segment.id, scope_id
                        ));
                    }
                    if segment.cleanup_scope_stack.contains(&scope_id) {
                        return Err(format!(
                            "{path}: seg{} cleanup-body still lists cleanup{} in cleanup stack",
                            segment.id, scope_id
                        ));
                    }
                }
                HandleSegmentDispatchContext::Arm { arm_id } => {
                    let arm = arm_bodies_by_id.get(&arm_id).ok_or_else(|| {
                        format!(
                            "{path}: seg{} arm-body references missing arm{}",
                            segment.id, arm_id
                        )
                    })?;
                    if !arm.body_segments.contains(&segment.id) {
                        return Err(format!(
                            "{path}: seg{} is marked as arm{} body but arm metadata does not include it",
                            segment.id, arm_id
                        ));
                    }
                }
            }
        }

        for (idx, nested) in self.nested_handles.iter().enumerate() {
            nested.validate_builder_contract_with_path(&format!("{path}/nested#{idx}"))?;
        }

        Ok(())
    }

    #[cfg(test)]
    pub(super) fn pretty_dump(&self, types: &TypeStore) -> String {
        let mut out = String::new();
        self.write_pretty_dump(types, 0, &mut out);
        out
    }

    #[cfg(test)]
    fn write_pretty_dump(&self, types: &TypeStore, indent: usize, out: &mut String) {
        let pad = " ".repeat(indent);
        let frame_slots_by_id = self
            .frame_slots
            .iter()
            .map(|slot| (slot.id, slot))
            .collect::<HashMap<_, _>>();
        let lifted_local_ids = self.lifted_locals.iter().copied().collect::<HashSet<_>>();
        out.push_str(&format!(
            "{pad}handle-segments span={:?} result={} entry=seg{}\n",
            self.handle_span,
            types.display(self.result_ty),
            self.entry_segment
        ));

        out.push_str(&format!("{pad}dispatch:\n"));
        if self.dispatch_entries.is_empty() {
            out.push_str(&format!("{pad}  []\n"));
        } else {
            for entry in &self.dispatch_entries {
                let targets = entry
                    .targets
                    .iter()
                    .map(HandleSegmentDispatchTarget::label)
                    .collect::<Vec<_>>()
                    .join(", ");
                out.push_str(&format!("{pad}  {} => [{}]\n", entry.op_fqn, targets));
            }
        }

        out.push_str(&format!("{pad}frame-slots:\n"));
        if self.frame_slots.is_empty() {
            out.push_str(&format!("{pad}  []\n"));
        } else {
            for slot in &self.frame_slots {
                let owner = slot.owner_arm.map_or_else(
                    || "handle-body".to_string(),
                    |arm_id| format!("arm{arm_id}"),
                );
                out.push_str(&format!(
                    "{pad}  {}:{} owner={} lifted={}\n",
                    slot.display_name(),
                    types.display(slot.ty),
                    owner,
                    yes_no(lifted_local_ids.contains(&slot.id))
                ));
            }
        }

        out.push_str(&format!("{pad}arm-bodies:\n"));
        if self.arm_bodies.is_empty() {
            out.push_str(&format!("{pad}  []\n"));
        } else {
            for arm in &self.arm_bodies {
                out.push_str(&format!(
                    "{pad}  arm{} op={} entry=seg{} segments=[{}]\n",
                    arm.arm_id,
                    arm.op_fqn,
                    arm.body_entry_segment,
                    render_segment_ids(&arm.body_segments)
                ));
                out.push_str(&format!(
                    "{pad}    binders=[{}]\n",
                    render_segment_slot_refs(&arm.binder_slots, &frame_slots_by_id, types)
                ));
                out.push_str(&format!(
                    "{pad}    captures=[{}]\n",
                    render_segment_symbol_ids(&arm.capture_locals, &frame_slots_by_id)
                ));
                out.push_str(&format!(
                    "{pad}    cleanup-stack=[{}]\n",
                    render_segment_cleanup_scope_ids(&arm.cleanup_scope_stack)
                ));
            }
        }

        out.push_str(&format!("{pad}cleanup-scopes:\n"));
        if self.cleanup_scopes.is_empty() {
            out.push_str(&format!("{pad}  []\n"));
        } else {
            for scope in &self.cleanup_scopes {
                out.push_str(&format!(
                    "{pad}  cleanup{} kind={} entry=seg{} exit=seg{} note={}\n",
                    scope.id,
                    scope.kind.label(),
                    scope.entry_segment,
                    scope.exit_segment,
                    scope.note
                ));
            }
        }

        out.push_str(&format!("{pad}suspend-sites:\n"));
        if self.suspend_sites.is_empty() {
            out.push_str(&format!("{pad}  []\n"));
        } else {
            for site in &self.suspend_sites {
                let arms = site
                    .matching_arms
                    .iter()
                    .map(|arm_id| format!("arm{arm_id}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                let available =
                    render_segment_symbol_ids(&site.available_locals, &frame_slots_by_id);
                let captures = render_segment_symbol_ids(&site.capture_locals, &frame_slots_by_id);
                out.push_str(&format!(
                    "{pad}  site{} kind={} span={:?} owner=seg{} resume=seg{} arms=[{}]\n",
                    site.id,
                    site.kind.label(),
                    site.span,
                    site.owner_segment,
                    site.resume_segment,
                    arms
                ));
                out.push_str(&format!("{pad}    available=[{available}]\n"));
                out.push_str(&format!("{pad}    captures=[{captures}]\n"));
                if let Some(escape_resume_segment) = site.escape_resume_segment {
                    out.push_str(&format!(
                        "{pad}    escape-resume=seg{}\n",
                        escape_resume_segment
                    ));
                }
                if let Some(detail) = site.kind.detail() {
                    out.push_str(&format!("{pad}    detail={detail}\n"));
                }
                if let Some(source_path) = &site.source_path {
                    out.push_str(&format!("{pad}    path={}\n", source_path.label()));
                }
                if let Some(resume_path) = &site.resume_path {
                    out.push_str(&format!("{pad}    resume-path={}\n", resume_path.label()));
                }
            }
        }

        out.push_str(&format!("{pad}edges:\n"));
        if self.edges.is_empty() {
            out.push_str(&format!("{pad}  []\n"));
        } else {
            for edge in &self.edges {
                out.push_str(&format!(
                    "{pad}  edge seg{} -{}-> seg{}\n",
                    edge.from,
                    edge.kind.label(),
                    edge.to
                ));
            }
        }

        out.push_str(&format!("{pad}segments:\n"));
        let frame_slot_map = self
            .frame_slots
            .iter()
            .cloned()
            .map(|slot| (slot.id, slot))
            .collect::<HashMap<_, _>>();
        for segment in &self.segments {
            let source_span = segment
                .source_span
                .map_or_else(|| "none".to_string(), |span| format!("{span:?}"));
            out.push_str(&format!(
                "{pad}  seg{} {} span={source_span}:\n",
                segment.id, segment.label
            ));
            out.push_str(&format!(
                "{pad}    context={}\n",
                segment.dispatch_context.label()
            ));
            out.push_str(&format!(
                "{pad}    cleanup-stack=[{}]\n",
                render_segment_cleanup_scope_ids(&segment.cleanup_scope_stack)
            ));
            for op in &segment.ops {
                out.push_str(&format!("{pad}    {}\n", op.label(&frame_slot_map, types)));
            }
            out.push_str(&format!(
                "{pad}    terminator={}\n",
                segment.terminator.label()
            ));
        }

        out.push_str(&format!("{pad}nested-handles:\n"));
        if self.nested_handles.is_empty() {
            out.push_str(&format!("{pad}  []\n"));
        } else {
            for (idx, nested) in self.nested_handles.iter().enumerate() {
                out.push_str(&format!("{pad}  nested#{idx}\n"));
                nested.write_pretty_dump(types, indent + 4, out);
            }
        }
    }
}

impl HandleSegment {
    fn from_plan_state(
        state: &PlanState,
        default_span: Span,
        suspend_sites: &[HandleSegmentSuspendSite],
        resume_targets: &HashMap<SuspendSiteId, HandleSegmentId>,
        dispatch_context: HandleSegmentDispatchContext,
        cleanup_scope_stack: Vec<CleanupScopeId>,
    ) -> Self {
        let source_span = match &state.terminator {
            StateTerminator::Suspend { site_id } => suspend_sites
                .iter()
                .find(|site| site.id == *site_id)
                .map(|site| site.span),
            _ => Some(default_span),
        };

        Self {
            id: state.id,
            label: state.label.clone(),
            source_span,
            dispatch_context,
            cleanup_scope_stack,
            ops: state.actions.clone(),
            terminator: HandleSegmentTerminator::from_plan(&state.terminator, resume_targets),
        }
    }

    fn outgoing_edges(&self) -> Vec<HandleSegmentEdge> {
        self.terminator.outgoing_edges(self.id)
    }

    fn to_plan_state(&self) -> PlanState {
        PlanState {
            id: self.id,
            label: self.label.clone(),
            actions: self.ops.clone(),
            terminator: self.terminator.to_plan(),
            // reads are only needed while deriving captures in the legacy
            // HIR-driven builder. The frozen segment contract already carries
            // the computed suspend-site / arm capture sets, so round-tripping
            // the executable plan does not need to recover them.
            reads: Vec::new(),
        }
    }

    fn structural_signature(&self) -> usize {
        let mut acc = self.id as usize ^ self.label.len();
        if let Some(span) = self.source_span {
            acc ^= span.start ^ (span.end << 1);
        }
        acc ^= self.dispatch_context.structural_signature();
        for scope_id in &self.cleanup_scope_stack {
            acc ^= (*scope_id as usize) << 2;
        }
        for op in &self.ops {
            acc ^= op.structural_signature();
        }
        acc ^ self.terminator.structural_signature()
    }
}

impl HandleSegmentDispatchContext {
    fn structural_signature(self) -> usize {
        match self {
            Self::Main => 1,
            Self::Cleanup { scope_id, kind } => {
                2 ^ (scope_id as usize) ^ (kind.structural_signature() << 1)
            }
            Self::Arm { arm_id } => 3 ^ (arm_id as usize),
        }
    }

    fn label(self) -> String {
        match self {
            Self::Main => "handle-body".to_string(),
            Self::Cleanup { scope_id, kind } => {
                format!("cleanup-body cleanup{scope_id} kind={}", kind.label())
            }
            Self::Arm { arm_id } => format!("arm-body arm{arm_id}"),
        }
    }
}

impl HandleSegmentTerminator {
    fn from_plan(
        terminator: &StateTerminator,
        resume_targets: &HashMap<SuspendSiteId, HandleSegmentId>,
    ) -> Self {
        match terminator {
            StateTerminator::Goto(state_id) => Self::Goto {
                next_segment: *state_id,
            },
            StateTerminator::Branch {
                condition,
                then_state,
                else_state,
                merge_state,
            } => Self::Branch {
                condition: condition.clone(),
                then_segment: *then_state,
                else_segment: *else_state,
                merge_segment: *merge_state,
            },
            StateTerminator::Suspend { site_id } => Self::Suspend {
                site_id: *site_id,
                resume_segment: *resume_targets
                    .get(site_id)
                    .expect("segment projection missing suspend resume target"),
            },
            StateTerminator::CleanupEnter {
                scope_id,
                next_state,
            } => Self::CleanupEnter {
                scope_id: *scope_id,
                next_segment: *next_state,
            },
            StateTerminator::ReturnHandle => Self::ReturnHandle,
            StateTerminator::ReturnFromFunction => Self::ReturnFromFunction,
            StateTerminator::ArmExit(exit) => Self::ArmExit { exit: *exit },
        }
    }

    fn outgoing_edges(&self, from: HandleSegmentId) -> Vec<HandleSegmentEdge> {
        match self {
            Self::Goto { next_segment } => vec![HandleSegmentEdge {
                from,
                to: *next_segment,
                kind: HandleSegmentEdgeKind::Goto,
            }],
            Self::Branch {
                then_segment,
                else_segment,
                ..
            } => vec![
                HandleSegmentEdge {
                    from,
                    to: *then_segment,
                    kind: HandleSegmentEdgeKind::BranchThen,
                },
                HandleSegmentEdge {
                    from,
                    to: *else_segment,
                    kind: HandleSegmentEdgeKind::BranchElse,
                },
            ],
            Self::Suspend { resume_segment, .. } => vec![HandleSegmentEdge {
                from,
                to: *resume_segment,
                kind: HandleSegmentEdgeKind::SuspendResume,
            }],
            Self::CleanupEnter { next_segment, .. } => vec![HandleSegmentEdge {
                from,
                to: *next_segment,
                kind: HandleSegmentEdgeKind::CleanupEnter,
            }],
            Self::ReturnHandle | Self::ReturnFromFunction | Self::ArmExit { .. } => Vec::new(),
        }
    }

    fn structural_signature(&self) -> usize {
        match self {
            Self::Goto { next_segment } => *next_segment as usize,
            Self::Branch {
                condition,
                then_segment,
                else_segment,
                merge_segment,
            } => {
                condition.structural_signature()
                    ^ (*then_segment as usize)
                    ^ ((*else_segment as usize) << 1)
                    ^ ((*merge_segment as usize) << 2)
            }
            Self::Suspend {
                site_id,
                resume_segment,
            } => 0x1000 ^ (*site_id as usize) ^ ((*resume_segment as usize) << 1),
            Self::CleanupEnter {
                scope_id,
                next_segment,
            } => 0x2000 ^ (*scope_id as usize) ^ ((*next_segment as usize) << 1),
            Self::ReturnHandle => 0x3000,
            Self::ReturnFromFunction => 0x4000,
            Self::ArmExit { exit } => 0x5000 ^ exit.structural_signature(),
        }
    }

    fn label(&self) -> String {
        match self {
            Self::Goto { next_segment } => format!("goto seg{next_segment}"),
            Self::Branch {
                condition,
                then_segment,
                else_segment,
                merge_segment,
            } => format!(
                "branch cond={} then=seg{then_segment} else=seg{else_segment} merge=seg{merge_segment}",
                condition.label()
            ),
            Self::Suspend {
                site_id,
                resume_segment,
            } => format!("suspend site{site_id} -> seg{resume_segment}"),
            Self::CleanupEnter {
                scope_id,
                next_segment,
            } => format!("cleanup scope{scope_id} -> seg{next_segment}"),
            Self::ReturnHandle => "return handle".to_string(),
            Self::ReturnFromFunction => "return function".to_string(),
            Self::ArmExit { exit } => format!("arm-exit {}", exit.label()),
        }
    }

    fn to_plan(&self) -> StateTerminator {
        match self {
            Self::Goto { next_segment } => StateTerminator::Goto(*next_segment),
            Self::Branch {
                condition,
                then_segment,
                else_segment,
                merge_segment,
            } => StateTerminator::Branch {
                condition: condition.clone(),
                then_state: *then_segment,
                else_state: *else_segment,
                merge_state: *merge_segment,
            },
            Self::Suspend { site_id, .. } => StateTerminator::Suspend { site_id: *site_id },
            Self::CleanupEnter {
                scope_id,
                next_segment,
            } => StateTerminator::CleanupEnter {
                scope_id: *scope_id,
                next_state: *next_segment,
            },
            Self::ReturnHandle => StateTerminator::ReturnHandle,
            Self::ReturnFromFunction => StateTerminator::ReturnFromFunction,
            Self::ArmExit { exit } => StateTerminator::ArmExit(*exit),
        }
    }
}

impl HandleSegmentEdgeKind {
    fn structural_signature(self) -> usize {
        match self {
            Self::Goto => 1,
            Self::BranchThen => 2,
            Self::BranchElse => 3,
            Self::SuspendResume => 4,
            Self::CleanupEnter => 5,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Goto => "goto",
            Self::BranchThen => "branch-then",
            Self::BranchElse => "branch-else",
            Self::SuspendResume => "suspend-resume",
            Self::CleanupEnter => "cleanup-enter",
        }
    }
}

impl HandleSegmentEdge {
    fn structural_signature(&self) -> usize {
        self.from as usize ^ ((self.to as usize) << 1) ^ (self.kind.structural_signature() << 2)
    }
}

impl HandleSegmentDispatchEntry {
    fn to_plan(&self) -> DispatchEntry {
        DispatchEntry {
            op_fqn: self.op_fqn.clone(),
            arm_ids: self.arm_ids.clone(),
        }
    }

    fn structural_signature(&self) -> usize {
        let mut acc = self.op_fqn.len();
        for arm_id in &self.arm_ids {
            acc ^= *arm_id as usize;
        }
        for target in &self.targets {
            acc ^= target.structural_signature();
        }
        acc
    }
}

impl HandleSegmentDispatchTarget {
    fn structural_signature(&self) -> usize {
        self.arm_id as usize ^ ((self.entry_segment as usize) << 1)
    }

    #[cfg(test)]
    fn label(&self) -> String {
        format!("arm{}(entry=seg{})", self.arm_id, self.entry_segment)
    }
}

impl HandleSegmentArmBody {
    fn to_plan(&self, frame_slots: &HashMap<hir::SymbolId, FrameSlot>) -> Result<ArmPlan, String> {
        let binder_slots = self
            .binder_slots
            .iter()
            .map(|slot_id| {
                frame_slots.get(slot_id).cloned().ok_or_else(|| {
                    format!(
                        "segment builder input missing slot metadata for arm{} binder local#{}",
                        self.arm_id,
                        slot_id.as_u32()
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(ArmPlan {
            id: self.arm_id,
            op_fqn: self.op_fqn.clone(),
            effect_ty: self.effect_ty,
            binder_slots,
            capture_locals: self.capture_locals.clone(),
            body_entry_state: self.body_entry_segment,
            body_may_suspend_outward: self.body_may_suspend_outward,
        })
    }

    fn structural_signature(&self) -> usize {
        let mut acc = self.arm_id as usize
            ^ self.op_fqn.len()
            ^ self.effect_ty.as_u32() as usize
            ^ (self.body_entry_segment as usize)
            ^ (usize::from(self.body_may_suspend_outward) << 1);
        for segment_id in &self.body_segments {
            acc ^= (*segment_id as usize) << 3;
        }
        for slot_id in &self.binder_slots {
            acc ^= (slot_id.as_u32() as usize) << 3;
        }
        for local_id in &self.capture_locals {
            acc ^= (local_id.as_u32() as usize) << 4;
        }
        for scope_id in &self.cleanup_scope_stack {
            acc ^= (*scope_id as usize) << 5;
        }
        acc
    }
}

impl HandleSegmentSuspendSite {
    fn from_plan(site: &SuspendSitePlan) -> Self {
        Self {
            id: site.id,
            span: site.span,
            kind: site.kind.clone(),
            owner_segment: site.owner_state,
            resume_segment: site.resume_target,
            escape_resume_segment: site.escape_resume_target,
            matching_arms: site.matching_arms.clone(),
            available_locals: site.available_locals.clone(),
            capture_locals: site.capture_locals.clone(),
            source_path: site.source_path.clone(),
            resume_path: site.resume_path.clone(),
            continuation_escape: site.continuation_escape,
        }
    }

    fn structural_signature(&self) -> usize {
        let mut acc = self.id as usize
            ^ self.span.start
            ^ self.span.end
            ^ ((self.owner_segment as usize) << 1)
            ^ self.resume_segment as usize
            ^ self.kind.structural_signature();
        if let Some(escape_resume_segment) = self.escape_resume_segment {
            acc ^= (escape_resume_segment as usize) << 2;
        }
        for arm_id in &self.matching_arms {
            acc ^= *arm_id as usize;
        }
        for id in &self.available_locals {
            acc ^= id.as_u32() as usize;
        }
        for id in &self.capture_locals {
            acc ^= (id.as_u32() as usize) << 1;
        }
        if let Some(source_path) = &self.source_path {
            acc ^= source_path.structural_signature();
        }
        if let Some(resume_path) = &self.resume_path {
            acc ^= resume_path.structural_signature();
        }
        acc ^= self.continuation_escape.structural_signature() << 3;
        acc
    }

    fn to_plan(&self) -> SuspendSitePlan {
        SuspendSitePlan {
            id: self.id,
            span: self.span,
            kind: self.kind.clone(),
            owner_state: self.owner_segment,
            resume_target: self.resume_segment,
            escape_resume_target: self.escape_resume_segment,
            matching_arms: self.matching_arms.clone(),
            available_locals: self.available_locals.clone(),
            capture_locals: self.capture_locals.clone(),
            source_path: self.source_path.clone(),
            resume_path: self.resume_path.clone(),
            continuation_escape: self.continuation_escape,
        }
    }
}

impl HandleSegmentCleanupScope {
    fn from_plan(scope: &CleanupScopePlan) -> Self {
        Self {
            id: scope.id,
            kind: scope.kind,
            entry_segment: scope.entry_state,
            exit_segment: scope.exit_state,
            note: scope.note.clone(),
        }
    }

    fn structural_signature(&self) -> usize {
        self.id as usize
            ^ self.kind.structural_signature()
            ^ self.entry_segment as usize
            ^ self.exit_segment as usize
            ^ self.note.len()
    }

    fn to_plan(&self) -> CleanupScopePlan {
        CleanupScopePlan {
            id: self.id,
            kind: self.kind,
            entry_state: self.entry_segment,
            exit_state: self.exit_segment,
            note: self.note.clone(),
        }
    }
}

fn render_segment_symbol_ids(
    ids: &[hir::SymbolId],
    slots_by_id: &HashMap<hir::SymbolId, &FrameSlot>,
) -> String {
    let mut labels = ids
        .iter()
        .map(|id| (id.as_u32(), describe_segment_local(*id, slots_by_id)))
        .collect::<Vec<_>>();
    labels.sort_by_key(|(id, _)| *id);
    labels
        .into_iter()
        .map(|(_, label)| label)
        .collect::<Vec<_>>()
        .join(", ")
}

fn render_segment_cleanup_scope_ids(ids: &[CleanupScopeId]) -> String {
    ids.iter()
        .map(|id| format!("cleanup{id}"))
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
fn render_segment_ids(ids: &[HandleSegmentId]) -> String {
    ids.iter()
        .map(|id| format!("seg{id}"))
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
fn render_segment_slot_refs(
    slot_ids: &[hir::SymbolId],
    slots_by_id: &HashMap<hir::SymbolId, &FrameSlot>,
    types: &TypeStore,
) -> String {
    slot_ids
        .iter()
        .map(|slot_id| {
            slots_by_id.get(slot_id).map_or_else(
                || format!("unknown#{}", slot_id.as_u32()),
                |slot| format!("{}:{}", slot.display_name(), types.display(slot.ty)),
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn describe_segment_local(
    id: hir::SymbolId,
    slots_by_id: &HashMap<hir::SymbolId, &FrameSlot>,
) -> String {
    slots_by_id.get(&id).map_or_else(
        || format!("local#{}", id.as_u32()),
        |slot| slot.display_name(),
    )
}

fn describe_suspend_site_kind(kind: &SuspendSiteKind) -> &'static str {
    match kind {
        SuspendSiteKind::Perform { .. } => "perform",
        SuspendSiteKind::CallMaySuspend { .. } => "call-may-suspend",
        SuspendSiteKind::CallStateMachineCallee { .. } => "call-state-machine-callee",
        SuspendSiteKind::RuntimeRaise { .. } => "runtime-raise",
        SuspendSiteKind::ObjectInitAccess { .. } => "object-init-access",
        SuspendSiteKind::TopLevelValueInitAccess { .. } => "top-level-val-init-access",
        SuspendSiteKind::ClassCtorInit { .. } => "class-ctor-init",
        SuspendSiteKind::NestedHandleBoundary { .. } => "nested-handle-boundary",
    }
}

fn build_segment_frame_slots(frame_layout: &FrameLayoutPlan) -> Vec<FrameSlot> {
    let mut frame_slots = frame_layout.slots.values().cloned().collect::<Vec<_>>();
    frame_slots.sort_by_key(|slot| slot.id.as_u32());
    frame_slots
}

fn build_segment_successor_map(
    states: &[PlanState],
    suspend_sites: &[SuspendSitePlan],
) -> HashMap<PlanStateId, Vec<PlanStateId>> {
    let resume_targets = suspend_sites
        .iter()
        .map(|site| (site.id, site.resume_target))
        .collect::<HashMap<_, _>>();
    states
        .iter()
        .map(|state| {
            let successors = match &state.terminator {
                StateTerminator::Goto(next_state) => vec![*next_state],
                StateTerminator::Branch {
                    then_state,
                    else_state,
                    ..
                } => vec![*then_state, *else_state],
                StateTerminator::Suspend { site_id } => vec![
                    *resume_targets
                        .get(site_id)
                        .expect("segment successor map missing suspend resume target"),
                ],
                StateTerminator::CleanupEnter { next_state, .. } => vec![*next_state],
                StateTerminator::ReturnHandle
                | StateTerminator::ReturnFromFunction
                | StateTerminator::ArmExit(_) => Vec::new(),
            };
            (state.id, successors)
        })
        .collect()
}

fn collect_state_region<F>(
    start_state: PlanStateId,
    states_by_id: &HashMap<PlanStateId, &PlanState>,
    successors: &HashMap<PlanStateId, Vec<PlanStateId>>,
    stop_at: F,
) -> Vec<PlanStateId>
where
    F: Fn(&PlanState) -> bool,
{
    let mut seen = HashSet::new();
    let mut stack = vec![start_state];
    while let Some(state_id) = stack.pop() {
        if !seen.insert(state_id) {
            continue;
        }
        let state = states_by_id
            .get(&state_id)
            .expect("segment region should only visit known states");
        if stop_at(state) {
            continue;
        }
        if let Some(next_states) = successors.get(&state_id) {
            stack.extend(next_states.iter().copied());
        }
    }

    let mut region = seen.into_iter().collect::<Vec<_>>();
    region.sort_unstable();
    region
}

fn build_state_cleanup_execution_scopes(
    cleanup_scopes: &[CleanupScopePlan],
    states: &[PlanState],
    successors: &HashMap<PlanStateId, Vec<PlanStateId>>,
) -> HashMap<PlanStateId, Vec<CleanupScopeId>> {
    let states_by_id = states
        .iter()
        .map(|state| (state.id, state))
        .collect::<HashMap<_, _>>();
    let mut cleanup_scopes_by_state = HashMap::<PlanStateId, Vec<CleanupScopeId>>::new();

    for scope in cleanup_scopes {
        let region = collect_state_region(scope.entry_state, &states_by_id, successors, |state| {
            state.id == scope.exit_state
        });
        for state_id in region {
            cleanup_scopes_by_state
                .entry(state_id)
                .or_default()
                .push(scope.id);
        }
    }

    for scope_ids in cleanup_scopes_by_state.values_mut() {
        scope_ids.sort_unstable();
        scope_ids.dedup();
    }

    cleanup_scopes_by_state
}

fn build_state_cleanup_scope_stacks(
    cleanup_scopes: &[CleanupScopePlan],
    states: &[PlanState],
    state_cleanup_execution_scopes: &HashMap<PlanStateId, Vec<CleanupScopeId>>,
) -> HashMap<PlanStateId, Vec<CleanupScopeId>> {
    let mut all_scope_ids = cleanup_scopes
        .iter()
        .map(|scope| scope.id)
        .collect::<Vec<_>>();
    all_scope_ids.sort_unstable();

    states
        .iter()
        .map(|state| {
            let executing_scopes = state_cleanup_execution_scopes
                .get(&state.id)
                .cloned()
                .unwrap_or_default();
            let cleanup_scope_stack = all_scope_ids
                .iter()
                .copied()
                .filter(|scope_id| !executing_scopes.contains(scope_id))
                .collect::<Vec<_>>();
            (state.id, cleanup_scope_stack)
        })
        .collect()
}

fn build_segment_arm_bodies(
    arm_plans: &[ArmPlan],
    states: &[PlanState],
    successors: &HashMap<PlanStateId, Vec<PlanStateId>>,
    state_cleanup_scope_stacks: &HashMap<PlanStateId, Vec<CleanupScopeId>>,
) -> Vec<HandleSegmentArmBody> {
    let states_by_id = states
        .iter()
        .map(|state| (state.id, state))
        .collect::<HashMap<_, _>>();

    let mut arm_bodies = arm_plans
        .iter()
        .map(|arm| {
            let body_segments =
                collect_state_region(arm.body_entry_state, &states_by_id, successors, |state| {
                    matches!(state.terminator, StateTerminator::ArmExit(_))
                });
            let cleanup_scope_stack = state_cleanup_scope_stacks
                .get(&arm.body_entry_state)
                .cloned()
                .unwrap_or_default();
            debug_assert!(
                body_segments.iter().all(|segment_id| {
                    state_cleanup_scope_stacks
                        .get(segment_id)
                        .cloned()
                        .unwrap_or_default()
                        == cleanup_scope_stack
                }),
                "arm body should stay inside one cleanup context",
            );

            HandleSegmentArmBody {
                arm_id: arm.id,
                op_fqn: arm.op_fqn.clone(),
                effect_ty: arm.effect_ty,
                body_entry_segment: arm.body_entry_state,
                body_segments,
                binder_slots: arm.binder_slots.iter().map(|slot| slot.id).collect(),
                capture_locals: arm.capture_locals.clone(),
                body_may_suspend_outward: arm.body_may_suspend_outward,
                cleanup_scope_stack,
            }
        })
        .collect::<Vec<_>>();
    arm_bodies.sort_by_key(|arm| arm.arm_id);
    arm_bodies
}

fn build_state_dispatch_contexts(
    states: &[PlanState],
    cleanup_scopes: &[CleanupScopePlan],
    state_cleanup_execution_scopes: &HashMap<PlanStateId, Vec<CleanupScopeId>>,
    arm_bodies: &[HandleSegmentArmBody],
) -> HashMap<PlanStateId, HandleSegmentDispatchContext> {
    let cleanup_kinds = cleanup_scopes
        .iter()
        .map(|scope| (scope.id, scope.kind))
        .collect::<HashMap<_, _>>();
    let mut arm_by_state = HashMap::<PlanStateId, ArmPlanId>::new();
    for arm in arm_bodies {
        for segment_id in &arm.body_segments {
            arm_by_state.insert(*segment_id, arm.arm_id);
        }
    }

    states
        .iter()
        .map(|state| {
            let context = if let Some(arm_id) = arm_by_state.get(&state.id).copied() {
                debug_assert!(
                    !state_cleanup_execution_scopes.contains_key(&state.id),
                    "arm body should not overlap cleanup execution states",
                );
                HandleSegmentDispatchContext::Arm { arm_id }
            } else if let Some(scope_id) = state_cleanup_execution_scopes
                .get(&state.id)
                .and_then(|scope_ids| scope_ids.last())
                .copied()
            {
                HandleSegmentDispatchContext::Cleanup {
                    scope_id,
                    kind: *cleanup_kinds
                        .get(&scope_id)
                        .expect("segment dispatch context missing cleanup kind"),
                }
            } else {
                HandleSegmentDispatchContext::Main
            };
            (state.id, context)
        })
        .collect()
}

fn build_segment_dispatch_entries(
    dispatch_plan: &DispatchPlan,
    arm_bodies: &[HandleSegmentArmBody],
) -> Vec<HandleSegmentDispatchEntry> {
    let arm_bodies_by_id = arm_bodies
        .iter()
        .map(|arm| (arm.arm_id, arm))
        .collect::<HashMap<_, _>>();
    dispatch_plan
        .entries
        .iter()
        .map(|entry| HandleSegmentDispatchEntry {
            op_fqn: entry.op_fqn.clone(),
            arm_ids: entry.arm_ids.clone(),
            targets: entry
                .arm_ids
                .iter()
                .map(|arm_id| {
                    let arm = arm_bodies_by_id
                        .get(arm_id)
                        .expect("segment dispatch entry missing arm body metadata");
                    HandleSegmentDispatchTarget {
                        arm_id: *arm_id,
                        entry_segment: arm.body_entry_segment,
                    }
                })
                .collect(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::ast;
    use crate::hir;
    use crate::parser::parse_file;
    use crate::resolve::Index;
    use crate::session::{EffectPipelineMode, Session, SessionOptions};
    use crate::source::SourceFile;
    use crate::ty::TypeStore;
    use crate::typecheck;

    use super::*;

    #[test]
    fn segment_dump_covers_direct_branch_loop_and_finally() {
        let dump = build_segment_dump(
            r#"
package a

import scoop.core.*

effect Yield {
    fun next(): Int
}

fun demo(flag: Bool): Int {
    val result: Int = handle {
        var sum: Int = 0
        if (flag) {
            val x: Int = Yield.next()
            sum = x
        } else {
            sum = 1
        }
        while (sum < 3) {
            sum = sum + 1
        }
        sum
    } with {
        Yield.next() , k -> {
            k.resume(41)
        }
    } finally {
        println("cleanup")
    }
    result
}
"#,
        );

        assert!(dump.contains("handle-segments span="), "{dump}");
        assert!(dump.contains("cleanup0 kind=finally"), "{dump}");
        assert!(dump.contains("site0 kind=perform"), "{dump}");
        assert!(dump.contains("branch-then"), "{dump}");
        assert!(dump.contains("branch-else"), "{dump}");
        assert!(dump.contains("suspend-resume"), "{dump}");
        assert!(dump.contains("loop re-entry -> s"), "{dump}");
    }

    #[test]
    fn segment_dump_distinguishes_state_machine_callee_and_indirect_call_sites() {
        let dump = build_segment_dump(
            r#"
package a

import scoop.core.*

effect Ask {
    fun ask(seed: Int): Int
}

fun fetch(seed: Int): Int / (Ask) {
    Ask.ask(seed)
}

fun demo(thunk: () -> Int / (Ask)): Int {
    val result: Int = handle {
        val a: Int = fetch(1)
        val b: Int = thunk()
        a + b
    } with {
        Ask.ask(seed) , k -> {
            k.resume(seed + 10)
        }
    }
    result
}
"#,
        );

        assert!(dump.contains("kind=call-state-machine-callee"), "{dump}");
        assert!(dump.contains("detail=a.fetch"), "{dump}");
        assert!(dump.contains("kind=call-may-suspend"), "{dump}");
        assert!(dump.contains("path=top[0]"), "{dump}");
        assert!(dump.contains("path=top[1]"), "{dump}");
    }

    #[test]
    fn segment_dump_classifies_hidden_suspend_helper_as_state_machine_callee() {
        let dump = build_segment_dump(
            r#"
package a

import scoop.core.*

object BoomObject {
    init {
        Raise.raise(RuntimeError.NullAssertionFailed)
    }

    val x: Int = 1
}

fun helper(): Int {
    BoomObject.x
}

fun demo(): Int {
    val result: Int = handle {
        helper()
    } with {
        Raise.raise(err: RuntimeError) -> 10
    }
    result
}
"#,
        );

        assert!(dump.contains("kind=call-state-machine-callee"), "{dump}");
        assert!(dump.contains("detail=a.helper"), "{dump}");
    }

    #[test]
    fn segment_dump_classifies_hidden_suspend_member_helper_as_state_machine_callee() {
        let dump = build_segment_dump(
            r#"
package a

import scoop.core.*

object BoomObject {
    init {
        Raise.raise(RuntimeError.NullAssertionFailed)
    }

    val x: Int = 1
}

object Helper {
    fun run(): Int {
        BoomObject.x
    }
}

fun demo(): Int {
    val result: Int = handle {
        Helper.run()
    } with {
        Raise.raise(err: RuntimeError) -> 10
    }
    result
}
"#,
        );

        assert!(dump.contains("kind=call-state-machine-callee"), "{dump}");
        assert!(dump.contains("detail=a.Helper.run"), "{dump}");
    }

    #[test]
    fn segment_dump_classifies_hidden_suspend_local_closure_call_as_call_may_suspend() {
        let dump = build_segment_dump(
            r#"
package a

import scoop.core.*

object BoomObject {
    init {
        Raise.raise(RuntimeError.NullAssertionFailed)
    }

    val x: Int = 1
}

fun helper(): Int {
    BoomObject.x
}

fun demo(): Int {
    val thunk: () -> Int = {
        helper()
    }
    val result: Int = handle {
        thunk()
    } with {
        Raise.raise(err: RuntimeError) -> 10
    }
    result
}
"#,
        );

        assert!(dump.contains("kind=call-may-suspend"), "{dump}");
    }

    #[test]
    fn segment_dump_classifies_effectful_wrapper_member_function_value_call_as_call_may_suspend() {
        let dump = build_segment_dump(
            r#"
package a

import scoop.core.*

effect Ask {
    fun ask(seed: Int): Int
}

struct Wrapper(val f: () -> Int / (Ask))

fun demo(wrapper: Wrapper): Int {
    val result: Int = handle {
        wrapper.f()
    } with {
        Ask.ask(seed), k -> {
            k.resume(seed + 1)
        }
    }
    result
}
"#,
        );

        assert!(dump.contains("kind=call-may-suspend"), "{dump}");
        assert!(dump.contains("path=top[0]"), "{dump}");
    }

    #[test]
    fn segment_dump_classifies_higher_order_returned_function_value_call_as_call_may_suspend() {
        let dump = build_segment_dump(
            r#"
package a

import scoop.core.*

effect Ask {
    fun ask(seed: Int): Int
}

enum Mode {
    Pure,
    Effectful(val seed: Int),
}

fun choose(mode: Mode): () -> Int / (Ask) {
    when (mode) {
        Pure -> {
            val thunk: () -> Int / (Ask) = { 7 }
            thunk
        }
        Effectful(seed) -> {
            val thunk: () -> Int / (Ask) = { Ask.ask(seed) }
            thunk
        }
    }
}

fun demo(mode: Mode): Int {
    val result: Int = handle {
        choose(mode)()
    } with {
        Ask.ask(seed), k -> {
            k.resume(seed + 1)
        }
    }
    result
}
"#,
        );

        assert!(dump.contains("kind=call-may-suspend"), "{dump}");
        assert!(dump.contains("path=top[0]"), "{dump}");
    }

    #[test]
    fn segment_dump_skips_locally_handled_helper_and_uncalled_effectful_higher_order_param() {
        let dump = build_segment_dump(
            r#"
package a

import scoop.core.*

effect Ask {
    fun ask(seed: Int): Int
}

fun hidden(): Int / (Ask) {
    return handle {
        Ask.ask(1)
    } with {
        Ask.ask(seed) -> seed + 1
    }
}

fun latent(thunk: () -> Int / (Ask)): Int / (Ask) {
    7
}

fun demo(): Int {
    val result: Int = handle {
        hidden() + latent({ Ask.ask(2) })
    } with {
        Ask.ask(seed), k -> {
            k.resume(seed + 10)
        }
    }
    result
}
"#,
        );

        assert!(!dump.contains("detail=a.hidden"), "{dump}");
        assert!(!dump.contains("detail=a.latent"), "{dump}");
        assert!(!dump.contains("kind=call-state-machine-callee"), "{dump}");
        assert!(!dump.contains("kind=call-may-suspend"), "{dump}");
    }

    #[test]
    fn segment_dump_records_nested_while_source_path() {
        let dump = build_segment_dump(
            r#"
package a

import scoop.core.*

effect Yield {
    fun next(): Int
}

fun demo(limit: Int): Int {
    val result: Int = handle {
        var outer: Int = 0
        while (outer < limit) {
            var inner: Int = 0
            while (inner < 1) {
                val x: Int = Yield.next()
                inner = inner + x
            }
            outer = outer + 1
        }
        outer
    } with {
        Yield.next() , k -> {
            k.resume(1)
        }
    }
    result
}
"#,
        );

        assert!(dump.contains("kind=perform"), "{dump}");
        assert!(
            dump.contains("path=top[1] -> while-body[1] -> while-body[0]"),
            "{dump}"
        );
        assert!(dump.contains("loop re-entry -> s"), "{dump}");
    }

    #[test]
    fn segment_dump_records_when_arm_source_path() {
        let dump = build_segment_dump(
            r#"
package a

import scoop.core.*

effect Yield {
    fun next(): Int
}

fun demo(flag: Bool): Int {
    val result: Int = handle {
        when (flag) {
            true -> {
                val x: Int = Yield.next()
                x
            }
            false -> 0
        }
    } with {
        Yield.next() , k -> {
            k.resume(1)
        }
    }
    result
}
"#,
        );

        assert!(dump.contains("kind=perform"), "{dump}");
        assert!(dump.contains("path=top[0] -> when-arm#0[0]"), "{dump}");
    }

    #[test]
    fn segment_dump_records_self_contained_nested_handle_boundary_in_outer_machine() {
        let dump = build_segment_dump(
            r#"
package a

import scoop.core.*

effect Yield {
    fun next(): Int
}

effect Ask {
    fun current(): Int
}

effect Boom {
    fun boom(code: Int): Nothing
}

fun demo(mode: Int): Int {
    val result: Int = handle {
        val inner: Int = handle {
            val x: Int = Yield.next()
            x + mode
        } with {
            Yield.next() , k -> {
                k.resume(10)
            }
        }
        if (mode == 0) {
            val y: Int = Ask.current()
            inner + y
        } else {
            Boom.boom(mode)
            0
        }
    } with {
        Ask.current(), k -> 7
        Boom.boom(code: Int) -> 0
    }
    result
}
"#,
        );

        assert!(dump.contains("kind=nested-handle-boundary"), "{dump}");
        assert!(dump.contains("nested-handles:\n  nested#0"), "{dump}");
        assert!(dump.contains("a.Yield.next => [arm"), "{dump}");
        assert!(dump.contains("site0 kind=perform"), "{dump}");
        assert!(
            dump.contains("dispatch:\n  a.Ask.current => [arm0(entry=seg"),
            "{dump}"
        );
        assert!(dump.contains("a.Boom.boom => [arm1(entry=seg"), "{dump}");
        assert!(
            dump.contains("arm-bodies:\n  arm0 op=a.Ask.current entry=seg"),
            "{dump}"
        );
        assert!(dump.contains("arm1 op=a.Boom.boom entry=seg"), "{dump}");
        assert!(dump.contains("context=arm-body arm0"), "{dump}");
        assert!(dump.contains("context=arm-body arm1"), "{dump}");
        assert!(
            dump.contains("terminator=arm-exit materialize-continuation"),
            "{dump}"
        );
        assert!(dump.contains("terminator=arm-exit return-handle"), "{dump}");
        assert!(dump.contains("mode="), "{dump}");
    }

    #[test]
    fn segment_dump_records_mixed_arm_cleanup_context_and_dispatch_context() {
        let dump = build_segment_dump(
            r#"
package a

import scoop.core.*

effect Yield {
    fun next(): Int
}

effect Log {
    fun current(seed: Int): Int
}

fun demo(): Int {
    val result: Int = handle {
        val x: Int = Yield.next()
        val y: Int = Log.current(x)
        x + y
    } with {
        Yield.next() , k -> {
            k.resume(10)
        }
        Log.current(seed: Int) -> seed + 1
    } finally {
        println("cleanup")
    }
    result
}
"#,
        );

        assert!(
            dump.contains("dispatch:\n  a.Log.current => [arm1(entry=seg"),
            "{dump}"
        );
        assert!(dump.contains("a.Yield.next => [arm0(entry=seg"), "{dump}");
        assert!(dump.contains("arm0 op=a.Yield.next entry=seg"), "{dump}");
        assert!(dump.contains("arm1 op=a.Log.current entry=seg"), "{dump}");
        assert!(dump.contains("context=arm-body arm0"), "{dump}");
        assert!(dump.contains("context=arm-body arm1"), "{dump}");
        assert!(
            dump.contains("terminator=arm-exit materialize-continuation"),
            "{dump}"
        );
        assert!(dump.contains("terminator=arm-exit return-handle"), "{dump}");
        assert!(
            dump.contains("context=cleanup-body cleanup0 kind=finally"),
            "{dump}"
        );
        assert!(dump.contains("cleanup-stack=[cleanup0]"), "{dump}");
        assert!(dump.contains("mode="), "{dump}");
    }

    #[test]
    fn segment_dump_covers_richer_mixed_while_direct_and_indirect_sites() {
        let dump = build_segment_dump(
            r#"
package a

import scoop.core.*

effect Yield {
    fun next(): Int
}

effect Ask {
    fun ask(seed: Int): Int
}

fun demo(limit: Int, thunk: (Int) -> Int / (Ask)): Int {
    val result: Int = handle {
        val base: Int = Yield.next()
        var i: Int = 0
        while (i < limit) {
            val direct: Int = Ask.ask(base + i)
            val indirect: Int = thunk(direct)
            println(indirect)
            i = i + 1
        }
        base + i
    } with {
        Yield.next() , k -> {
            k.resume(10)
        }
        Ask.ask(seed: Int), k -> seed + 2
    }
    result
}
"#,
        );

        assert!(
            dump.contains("dispatch:\n  a.Ask.ask => [arm1(entry=seg"),
            "{dump}"
        );
        assert!(dump.contains("a.Yield.next => [arm0(entry=seg"), "{dump}");
        assert!(dump.contains("arm0 op=a.Yield.next entry=seg"), "{dump}");
        assert!(dump.contains("arm1 op=a.Ask.ask entry=seg"), "{dump}");
        assert!(dump.contains("kind=perform"), "{dump}");
        assert!(dump.contains("kind=call-may-suspend"), "{dump}");
        assert!(dump.contains("path=top[2] -> while-body[0]"), "{dump}");
        assert!(dump.contains("path=top[2] -> while-body[1]"), "{dump}");
        assert!(
            dump.contains("terminator=arm-exit materialize-continuation"),
            "{dump}"
        );
        assert!(
            dump.contains("terminator=arm-exit materialize-continuation"),
            "{dump}"
        );
        assert!(dump.contains("mode="), "{dump}");
    }

    #[test]
    fn segment_dump_records_suspend_inside_cleanup_context() {
        let dump = build_segment_dump(
            r#"
package a

import scoop.core.*

effect Ask {
    fun ask(): Int
}

fun demo(): Int / (Ask) {
    val result: Int = handle {
        1
    } with {
        Ask.ask() -> 7
    } finally {
        val cleanup: Int = Ask.ask()
        println(cleanup)
    }
    result
}
"#,
        );

        assert!(dump.contains("cleanup0 kind=finally"), "{dump}");
        assert!(dump.contains("site0 kind=perform"), "{dump}");
        assert!(
            dump.contains("dispatch:\n  a.Ask.ask => [arm0(entry=seg"),
            "{dump}"
        );
        assert!(
            dump.contains("context=cleanup-body cleanup0 kind=finally"),
            "{dump}"
        );
    }

    #[test]
    fn segment_dump_marks_pure_continuation_resume_as_runtime_raise_site() {
        let dump = build_segment_dump(
            r#"
package a

import scoop.core.*

fun demo(k: Continuation<Int, Int>): Int {
    val result: Int = try {
        k.resume(1)
        11
    } catch (e: RuntimeError) {
        22
    }
    result
}
"#,
        );

        assert!(dump.contains("kind=runtime-raise"), "{dump}");
        assert!(dump.contains("detail=Continuation.resume"), "{dump}");
    }

    #[test]
    fn continuation_resume_hidden_suspend_classification_requires_typechecked_call_site_marker() {
        let lowered = lower_typed_single_source(
            r#"
package a

import scoop.core.*

fun demo(k: Continuation<Int, Int>): Int {
    val ignored: Int = try {
        k.resume(1)
        0
    } catch (e: RuntimeError) {
        1
    }
    val result: Int = handle {
        0
    } with {
        Raise.raise(err: RuntimeError) -> 1
    }
    result
}
"#,
        );

        let (fun, handle) =
            first_handle_in_file(&lowered.file).expect("expected a handle expression");
        let context = collect_plan_context(&lowered, fun);
        let builder = HandlePlanBuilder::new(&lowered.types, handle, &context);
        let resume_call_site = lowered
            .continuation_resume_call_sites
            .iter()
            .next()
            .cloned()
            .expect("expected a typechecked Continuation.resume call site");
        assert!(matches!(
            builder.classify_builtin_suspend_call(resume_call_site.span),
            Some(SuspendSiteKind::RuntimeRaise { reason }) if reason == "Continuation.resume"
        ));
        assert!(
            builder
                .classify_builtin_suspend_call(handle.body.span)
                .is_none(),
            "unmarked call sites must not be inferred as builtin resume sites"
        );
    }

    #[test]
    fn non_pure_continuation_resume_classifies_as_call_suspend_site() {
        let lowered = lower_typed_single_source(
            r#"
package a

import scoop.core.*

effect Boom {
    fun next(): Int
}

fun demo(k: Continuation<Int, Int, eff Boom>): Int / (Boom + Raise<RuntimeError>) {
    k.resume(1)
    val result: Int = handle {
        0
    } with {
        Raise.raise(err: RuntimeError) -> 1
    }
    result
}
"#,
        );

        let (fun, handle) =
            first_handle_in_file(&lowered.file).expect("expected a handle expression");
        let context = collect_plan_context(&lowered, fun);
        let builder = HandlePlanBuilder::new(&lowered.types, handle, &context);
        let resume_call_site = lowered
            .non_pure_continuation_resume_call_sites
            .iter()
            .next()
            .cloned()
            .expect("expected a non-pure Continuation.resume call site");
        assert!(matches!(
            builder.classify_builtin_suspend_call(resume_call_site.span),
            Some(SuspendSiteKind::CallMaySuspend { callee }) if callee == "Continuation.resume"
        ));
    }

    #[test]
    fn segment_dump_marks_class_ctor_init_as_hidden_suspend_site() {
        let dump = build_segment_dump(
            r#"
package a

import scoop.core.*

class Boom() {
    init {
        Raise.raise(RuntimeError.NullAssertionFailed)
    }
}

fun demo(): Int {
    val result: Int = try {
        val _boom: Boom = Boom()
        1
    } catch (e: RuntimeError) {
        0
    }
    result
}
"#,
        );

        assert!(dump.contains("kind=class-ctor-init"), "{dump}");
        assert!(dump.contains("detail=a.Boom"), "{dump}");
    }

    #[test]
    fn segment_dump_marks_object_init_access_as_hidden_suspend_site() {
        let dump = build_segment_dump(
            r#"
package a

import scoop.core.*

object BoomObject {
    init {
        Raise.raise(RuntimeError.NullAssertionFailed)
    }

    val x: Int = 1
}

fun demo(): Int {
    val result: Int = try {
        BoomObject.x
    } catch (e: RuntimeError) {
        0
    }
    result
}
"#,
        );

        assert!(dump.contains("kind=object-init-access"), "{dump}");
        assert!(dump.contains("detail=a.BoomObject.x"), "{dump}");
    }

    #[test]
    fn segment_dump_records_frame_slot_metadata_for_outer_locals_and_binders_when_nested_handle_is_self_contained()
     {
        let dump = build_segment_dump(
            r#"
package a

import scoop.core.*

effect Yield {
    fun next(seed: Int): Int
}

effect Ask {
    fun current(): Int
}

fun demo(seed: Int): Int {
    val base: Int = seed + 1
    val result: Int = handle {
        val local: Int = base + 1
        val inner: Int = handle {
            val asked: Int = Ask.current()
            asked + local + seed
        } with {
            Ask.current() , k -> {
                k.resume(base)
            }
        }
        val x: Int = Yield.next(local)
        x + inner + local
    } with {
        Yield.next(arg: Int) , k -> {
            k.resume(arg + base)
        }
    }
    result
}
"#,
        );

        assert!(dump.contains("frame-slots:"), "{dump}");
        assert!(dump.contains("base#"), "{dump}");
        assert!(dump.contains("local#"), "{dump}");
        assert!(dump.contains("arg#"), "{dump}");
        assert!(dump.contains("owner=handle-body"), "{dump}");
        assert!(dump.contains("owner=arm0"), "{dump}");
        assert!(dump.contains("lifted=yes"), "{dump}");
        assert!(dump.contains("nested#0"), "{dump}");
    }

    #[test]
    fn segment_builder_contract_rejects_missing_lifted_local_metadata() {
        let mut segment_list = build_segment_list(
            r#"
package a

import scoop.core.*

effect Yield {
    fun next(seed: Int): Int
}

fun demo(seed: Int): Int {
    val base: Int = seed + 1
    val result: Int = handle {
        val local: Int = base + 1
        val x: Int = Yield.next(local)
        x + local
    } with {
        Yield.next(arg: Int) , k -> {
            k.resume(arg + base)
        }
    }
    result
}
"#,
        );
        let base_id = segment_slot_id_named(&segment_list, "base");
        segment_list.lifted_locals.retain(|id| *id != base_id);

        let err = segment_list
            .validate_builder_contract()
            .expect_err("missing lifted-local metadata should fail");
        assert!(err.contains("lifted_locals[] is missing"), "{err}");
        assert!(err.contains("base#"), "{err}");
    }

    #[test]
    fn segment_builder_contract_rejects_dangling_capture_local_ref() {
        let mut segment_list = build_segment_list(
            r#"
package a

import scoop.core.*

effect Yield {
    fun next(seed: Int): Int
}

fun demo(seed: Int): Int {
    val base: Int = seed + 1
    val result: Int = handle {
        val local: Int = base + 1
        val x: Int = Yield.next(local)
        x + local
    } with {
        Yield.next(arg: Int) , k -> {
            k.resume(arg + base)
        }
    }
    result
}
"#,
        );
        let base_id = segment_slot_id_named(&segment_list, "base");
        segment_list.lifted_locals.retain(|id| *id != base_id);
        segment_list.frame_slots.retain(|slot| slot.id != base_id);

        let err = segment_list
            .validate_builder_contract()
            .expect_err("dangling capture local reference should fail");
        assert!(
            err.contains("arm0 capture metadata references missing slot"),
            "{err}"
        );
    }

    #[test]
    fn segment_builder_contract_rejects_mismatched_direct_perform_matching_arm() {
        let mut segment_list = build_segment_list(
            r#"
package a

import scoop.core.*

effect Yield {
    fun next(): Int
}

effect Log {
    fun current(seed: Int): Int
}

fun demo(): Int {
    val result: Int = handle {
        val x: Int = Yield.next()
        val y: Int = Log.current(x)
        x + y
    } with {
        Yield.next() , k -> {
            k.resume(10)
        }
        Log.current(seed: Int) -> seed + 1
    }
    result
}
"#,
        );
        let yield_site = segment_list
            .suspend_sites
            .iter_mut()
            .find(|site| {
                matches!(
                    &site.kind,
                        SuspendSiteKind::Perform { op_fqn } if op_fqn == "a.Yield.next"
                )
            })
            .expect("expected direct perform site for a.Yield.next");
        yield_site.matching_arms = vec![1];

        let err = segment_list
            .validate_builder_contract()
            .expect_err("mismatched perform matching arm should fail");
        assert!(err.contains("matches arm1 for a.Log.current"), "{err}");
    }

    #[test]
    fn segment_builder_contract_rejects_matching_arms_on_indirect_site() {
        let mut segment_list = build_segment_list(
            r#"
package a

import scoop.core.*

effect Ask {
    fun ask(seed: Int): Int
}

fun fetch(seed: Int): Int / (Ask) {
    Ask.ask(seed)
}

fun demo(): Int {
    val result: Int = handle {
        val value: Int = fetch(1)
        value
    } with {
        Ask.ask(seed: Int) , k -> {
            k.resume(seed + 10)
        }
    }
    result
}
"#,
        );
        let indirect_site = segment_list
            .suspend_sites
            .iter_mut()
            .find(|site| matches!(&site.kind, SuspendSiteKind::CallStateMachineCallee { .. }))
            .expect("expected call-state-machine-callee site");
        indirect_site.matching_arms = vec![0];

        let err = segment_list
            .validate_builder_contract()
            .expect_err("indirect call site must not list matching arms");
        assert!(err.contains("must not list matching arms"), "{err}");
        assert!(err.contains("call-state-machine-callee"), "{err}");
    }

    #[test]
    fn plan_round_trip_from_segments_preserves_direct_branch_loop_finally_dump() {
        let source = r#"
package a

import scoop.core.*

effect Yield {
    fun next(): Int
}

fun demo(flag: Bool): Int {
    val result: Int = handle {
        var sum: Int = 0
        if (flag) {
            val x: Int = Yield.next()
            sum = x
        } else {
            sum = 1
        }
        while (sum < 3) {
            sum = sum + 1
        }
        sum
    } with {
        Yield.next() , k -> {
            k.resume(41)
        }
    } finally {
        println("cleanup")
    }
    result
}
"#;

        assert_eq!(
            build_round_tripped_plan_dump(source),
            build_plan_dump(source)
        );
    }

    #[test]
    fn plan_and_segments_support_return_inside_handle_body_block_expression() {
        let source = r#"
package a

import scoop.core.*

effect Yield {
    fun next(): Int
}

fun demo(): Int {
    handle {
        if (true) {
            return 1
        }
        val x: Int = Yield.next()
        x
    } with {
        Yield.next() , k -> {
            k.resume(2)
        }
    }
    0
}
"#;

        let source_plan = build_source_plan(source);
        assert!(
            source_plan
                .states
                .iter()
                .any(|state| matches!(state.terminator, StateTerminator::ReturnFromFunction)),
            "plan should contain return-from-function terminator"
        );

        let segment_list = source_plan.build_segment_list();
        segment_list
            .validate_builder_contract()
            .expect("segment builder contract should hold");
        assert!(
            segment_list.segments.iter().any(|segment| matches!(
                segment.terminator,
                HandleSegmentTerminator::ReturnFromFunction
            )),
            "segment list should contain return-from-function terminator"
        );

        let rebuilt_plan = HandleStateMachinePlan::build_from_segments(&segment_list)
            .expect("segment-only builder should reconstruct plan with early return");
        assert!(
            rebuilt_plan
                .states
                .iter()
                .any(|state| matches!(state.terminator, StateTerminator::ReturnFromFunction)),
            "rebuilt plan should preserve return-from-function terminator"
        );

        assert_eq!(
            build_round_tripped_plan_dump(source),
            build_plan_dump(source)
        );
    }

    #[test]
    fn plan_round_trip_from_segments_preserves_indirect_suspend_dump() {
        let source = r#"
package a

import scoop.core.*

effect Ask {
    fun ask(seed: Int): Int
}

fun fetch(seed: Int): Int / (Ask) {
    Ask.ask(seed)
}

fun demo(thunk: () -> Int / (Ask)): Int {
    val result: Int = handle {
        val a: Int = fetch(1)
        val b: Int = thunk()
        a + b
    } with {
        Ask.ask(seed) , k -> {
            k.resume(seed + 10)
        }
    }
    result
}
"#;

        assert_eq!(
            build_round_tripped_plan_dump(source),
            build_plan_dump(source)
        );
    }

    #[test]
    fn plan_round_trip_from_segments_preserves_nested_handle_multi_arm_dump() {
        let source = r#"
package a

import scoop.core.*

effect Yield {
    fun next(): Int
}

effect Ask {
    fun current(): Int
}

effect Boom {
    fun boom(code: Int): Nothing
}

fun demo(mode: Int): Int {
    val result: Int = handle {
        val inner: Int = handle {
            val x: Int = Yield.next()
            x + mode
        } with {
            Yield.next() , k -> {
                k.resume(10)
            }
        }
        if (mode == 0) {
            val y: Int = Ask.current()
            inner + y
        } else {
            Boom.boom(mode)
            0
        }
    } with {
        Ask.current(), k -> 7
        Boom.boom(code: Int) -> 0
    }
    result
}
"#;

        assert_eq!(
            build_round_tripped_plan_dump(source),
            build_plan_dump(source)
        );
    }

    #[test]
    fn plan_round_trip_from_segments_preserves_hidden_suspend_site_dump() {
        let source = r#"
package a

import scoop.core.*

object BoomObject {
    init {
        Raise.raise(RuntimeError.NullAssertionFailed)
    }

    val x: Int = 1
}

fun demo(): Int {
    val result: Int = try {
        BoomObject.x
    } catch (e: RuntimeError) {
        0
    }
    result
}
"#;

        assert_eq!(
            build_round_tripped_plan_dump(source),
            build_plan_dump(source)
        );
    }

    #[test]
    fn segment_round_trip_preserves_typed_emit_ops_and_branch_metadata() {
        let source = r#"
package a

import scoop.core.*

effect Yield {
    fun next(): Int
}

fun demo(flag: Bool): Int {
    val result: Int = handle {
        var sum: Int = 0
        if (flag) {
            val x: Int = Yield.next()
            sum = x
        } else {
            sum = 1
        }
        while (sum < 3) {
            sum = sum + 1
        }
        sum
    } with {
        Yield.next() , k -> {
            k.resume(41)
        }
    } finally {
        println("cleanup")
    }
    result
}
"#;

        let source_plan = build_source_plan(source);
        let segment_list = source_plan.build_segment_list();
        segment_list
            .validate_builder_contract()
            .expect("segment builder contract should hold");
        let rebuilt_plan = HandleStateMachinePlan::build_from_segments(&segment_list)
            .expect("segment-only builder should reconstruct full plan");

        assert_eq!(
            collect_plan_exec_signature(&source_plan),
            collect_plan_exec_signature(&rebuilt_plan)
        );

        let while_cond_state = source_plan
            .states
            .iter()
            .find(|state| state.label == "while.cond")
            .expect("expected while.cond state");
        assert!(matches!(
            while_cond_state.actions.first(),
            Some(&HandleStateOp::WhileCondHeader { .. })
        ));
        assert!(matches!(
            &while_cond_state.terminator,
            StateTerminator::Branch {
                condition: HandleBranchCondition::WhileCond { .. },
                ..
            }
        ));

        let has_if_branch_segment = segment_list.segments.iter().any(|segment| {
            matches!(
                &segment.terminator,
                HandleSegmentTerminator::Branch {
                    condition: HandleBranchCondition::IfCond { .. },
                    ..
                }
            )
        });
        assert!(
            has_if_branch_segment,
            "expected an if-branch segment terminator"
        );

        let has_bind_local = segment_list
            .segments
            .iter()
            .flat_map(|segment| segment.ops.iter())
            .any(|op| matches!(op, &HandleStateOp::BindLocal { .. }));
        assert!(
            has_bind_local,
            "expected typed bind-local op in segment list"
        );
    }

    #[test]
    fn source_plan_skips_pure_initializer_fragment_ops_in_consumer_positions() {
        let source = r#"
package a

import scoop.core.*

effect Yield {
    fun next(): Int
}

fun demo(): Int? {
    val result: Int? = handle {
        val some: Int? = Some(7)
        some
    } with {
        Yield.next() , k -> {
            k.resume(0)
        }
    }
    result
}
"#;

        let source_plan = build_source_plan(source);
        let (init_span, callee_span, arg_span) = source_plan
            .states
            .iter()
            .flat_map(|state| state.actions.iter())
            .find_map(|op| match op {
                HandleStateOp::BindLocal { decl, .. } if decl.name.as_deref() == Some("some") => {
                    let init = decl
                        .init
                        .as_ref()
                        .expect("`some` should have an initializer");
                    let hir::ExprKind::Call { callee, args } = &init.kind else {
                        panic!("expected `some` initializer to stay as a call expression");
                    };
                    let [hir::CallArg::Positional(arg_expr)] = args.as_slice() else {
                        panic!("expected `Some(7)` to have one positional argument");
                    };
                    Some((init.span, callee.span, arg_expr.span))
                }
                _ => None,
            })
            .expect("expected bind-local op for `some`");

        assert!(
            !source_plan
                .states
                .iter()
                .flat_map(|state| state.actions.iter())
                .any(|op| { matches!(op, HandleStateOp::Call { expr } if expr.span == init_span) }),
            "pure initializer should be evaluated only by BindLocal, not by a preheated Call op"
        );
        assert!(
            !source_plan
                .states
                .iter()
                .flat_map(|state| state.actions.iter())
                .any(|op| {
                    matches!(op, HandleStateOp::VarRef { expr } if expr.span == callee_span)
                }),
            "pure initializer must not emit callee VarRef fragment op"
        );
        assert!(
            !source_plan
                .states
                .iter()
                .flat_map(|state| state.actions.iter())
                .any(|op| {
                    matches!(op, HandleStateOp::Literal { expr } if expr.span == arg_span)
                }),
            "pure initializer must not emit argument Literal fragment op"
        );
    }

    #[test]
    fn source_plan_keeps_only_whole_call_for_pure_statement_args_and_pure_if_condition() {
        let source = r#"
package a

import scoop.core.*

effect Yield {
    fun next(): Int
}

fun demo(seed: Int): Int {
    val result: Int = handle {
        println(seed + 1)
        if (seed > 0) {
            1
        } else {
            0
        }
    } with {
        Yield.next() , k -> {
            k.resume(0)
        }
    }
    result
}
"#;

        let source_plan = build_source_plan(source);
        let (statement_callee_span, statement_arg_span) = source_plan
            .states
            .iter()
            .flat_map(|state| state.actions.iter())
            .find_map(|op| match op {
                HandleStateOp::Call { expr } => {
                    let hir::ExprKind::Call { callee, args } = &expr.kind else {
                        return None;
                    };
                    let [hir::CallArg::Positional(arg_expr)] = args.as_slice() else {
                        return None;
                    };
                    Some((callee.span, arg_expr.span))
                }
                _ => None,
            })
            .expect("expected whole call op for println statement");
        let if_cond_span = source_plan
            .states
            .iter()
            .find_map(|state| match &state.terminator {
                StateTerminator::Branch {
                    condition: HandleBranchCondition::IfCond { condition },
                    ..
                } => Some(condition.span),
                _ => None,
            })
            .expect("expected if-branch terminator");

        assert!(
            !source_plan.states.iter().flat_map(|state| state.actions.iter()).any(|op| {
                matches!(op, HandleStateOp::VarRef { expr } if expr.span == statement_callee_span)
            }),
            "pure statement callees should stay inside the whole Call op instead of emitting VarRef fragments"
        );
        assert!(
            !source_plan.states.iter().flat_map(|state| state.actions.iter()).any(|op| {
                matches!(op, HandleStateOp::BinaryExpr { expr } if expr.span == statement_arg_span)
            }),
            "pure call arguments should stay inside the whole Call op instead of emitting BinaryExpr fragments"
        );
        assert!(
            !source_plan
                .states
                .iter()
                .flat_map(|state| state.actions.iter())
                .any(|op| {
                    matches!(op, HandleStateOp::BinaryExpr { expr } if expr.span == if_cond_span)
                }),
            "pure if conditions should be evaluated only by the Branch terminator"
        );
    }

    #[test]
    fn segment_dump_records_resume_path_for_nested_call_arg_site() {
        let dump = build_segment_dump(
            r#"
package a

import scoop.core.*

effect Yield {
    fun next(): Int
}

fun add(a: Int, b: Int): Int {
    a + b
}

fun demo(): Int {
    val result: Int = handle {
        val y: Int = add(Yield.next() + 1, 2)
        y
    } with {
        Yield.next() , k -> {
            k.resume(41)
        }
    }
    result
}
"#,
        );

        assert!(
            dump.contains("resume-path=val-init -> call-arg#0 -> binary-lhs"),
            "{dump}"
        );
    }

    #[test]
    fn source_plan_rewrites_direct_resume_consumer_to_synthetic_local() {
        let source_plan = build_source_plan(
            r#"
package a

import scoop.core.*

effect Yield {
    fun next(): Int
}

fun demo(): Int {
    val result: Int = handle {
        val y: Int = Yield.next()
        y
    } with {
        Yield.next() , k -> {
            k.resume(41)
        }
    }
    result
}
"#,
        );

        let resume_state = source_plan
            .states
            .iter()
            .find(|state| {
                state
                    .actions
                    .iter()
                    .any(|op| matches!(op, HandleStateOp::ResumeAfterSite { .. }))
            })
            .expect("expected a resume state");
        let resume_slot_name = resume_state
            .actions
            .iter()
            .find_map(|op| match op {
                HandleStateOp::ResumeAfterSite {
                    resume_slot: Some(slot),
                    ..
                } => Some(slot.name.clone()),
                _ => None,
            })
            .expect("resume state should allocate a synthetic resume slot");
        let bind_init = resume_state
            .actions
            .iter()
            .find_map(|op| match op {
                HandleStateOp::BindLocal { decl, .. } => decl.init.as_ref(),
                _ => None,
            })
            .expect("resume state should still bind the original user local");

        match &bind_init.kind {
            hir::ExprKind::VarRef(hir::ValueRef::Local { name, .. }) => {
                assert_eq!(name, &resume_slot_name);
                assert!(name.starts_with("__resume_site"));
            }
            other => panic!("expected rewritten local var ref, got {other:?}"),
        }
    }

    #[test]
    fn source_plan_rewrites_try_resume_direct_local_binding_to_synthetic_local() {
        let source_plan = build_source_plan(
            r#"
package a

import scoop.core.*

effect Boom {
    fun next(): Int
}

fun demo(k: Continuation<Int, Int, eff Boom>): Int / (Boom + Raise<RuntimeError>) {
    val resumed: Int = try {
        val step: Int = k.resume(1)
        step + 1
    } catch (e: RuntimeError) {
        0
    }
    resumed
}
"#,
        );

        let resume_state = source_plan
            .states
            .iter()
            .find(|state| {
                state
                    .actions
                    .iter()
                    .any(|op| matches!(op, HandleStateOp::ResumeAfterSite { .. }))
            })
            .expect("expected a resume state");
        let resume_slot_name = resume_state
            .actions
            .iter()
            .find_map(|op| match op {
                HandleStateOp::ResumeAfterSite {
                    reason: ResumeAfterSiteReason::Call,
                    resume_slot: Some(slot),
                    ..
                } => Some(slot.name.clone()),
                _ => None,
            })
            .expect("resume state should allocate a synthetic resume slot for resume call");
        let bind_init = resume_state
            .actions
            .iter()
            .find_map(|op| match op {
                HandleStateOp::BindLocal { decl, .. } if decl.name.as_deref() == Some("step") => {
                    decl.init.as_ref()
                }
                _ => None,
            })
            .expect("resume state should still bind the direct step local");

        match &bind_init.kind {
            hir::ExprKind::VarRef(hir::ValueRef::Local { name, .. }) => {
                assert_eq!(name, &resume_slot_name);
                assert!(name.starts_with("__resume_site"));
            }
            other => panic!("expected rewritten direct resume binding, got {other:?}"),
        }
    }

    #[test]
    fn source_plan_rewrites_task_drive_waiting_style_resume_binding() {
        let source_plan = build_source_plan(
            r#"
package a

import scoop.core.*

effect Boom {
    fun next(): (Int, Any)
}

fun apply(step: __TaskStepResult<Int>): Int {
    when (step) {
        Ready(_) -> 1
        Pending(_, _) -> 2
    }
}

fun demo(
    k: Continuation<(Int, Any), __TaskStepResult<Int>, eff Boom>,
    value: (Int, Any),
): Int / (Boom + Raise<RuntimeError>) {
    val resumed: Int = try {
        val step: __TaskStepResult<Int> = k.resume(value)
        apply(step)
    } catch (e: RuntimeError) {
        0
    }
    resumed
}
"#,
        );

        let resume_state = source_plan
            .states
            .iter()
            .find(|state| {
                state
                    .actions
                    .iter()
                    .any(|op| matches!(op, HandleStateOp::ResumeAfterSite { .. }))
            })
            .expect("expected a resume state");
        let resume_slot_name = resume_state
            .actions
            .iter()
            .find_map(|op| match op {
                HandleStateOp::ResumeAfterSite {
                    reason: ResumeAfterSiteReason::Call,
                    resume_slot: Some(slot),
                    ..
                } => Some(slot.name.clone()),
                _ => None,
            })
            .expect("resume state should allocate a synthetic resume slot");

        let step_bind = resume_state
            .actions
            .iter()
            .find_map(|op| match op {
                HandleStateOp::BindLocal { decl, .. } if decl.name.as_deref() == Some("step") => {
                    Some(decl)
                }
                _ => None,
            })
            .expect("resume state should bind the direct step local");
        let bind_init = step_bind
            .init
            .as_ref()
            .expect("step local should keep an initializer");

        match &bind_init.kind {
            hir::ExprKind::VarRef(hir::ValueRef::Local { name, .. }) => {
                assert_eq!(name, &resume_slot_name);
                assert!(name.starts_with("__resume_site"));
            }
            other => panic!("expected rewritten task-drive step binding, got {other:?}"),
        }
    }

    #[test]
    fn round_tripped_plan_preserves_task_drive_waiting_style_resume_binding() {
        let plan = build_round_tripped_plan(
            r#"
package a

import scoop.core.*

effect Boom {
    fun next(): (Int, Any)
}

fun apply(step: __TaskStepResult<Int>): Int {
    when (step) {
        Ready(_) -> 1
        Pending(_, _) -> 2
    }
}

fun demo(
    k: Continuation<(Int, Any), __TaskStepResult<Int>, eff Boom>,
    value: (Int, Any),
): Int / (Boom + Raise<RuntimeError>) {
    val resumed: Int = try {
        val step: __TaskStepResult<Int> = k.resume(value)
        apply(step)
    } catch (e: RuntimeError) {
        0
    }
    resumed
}
"#,
        );

        let resume_state = plan
            .states
            .iter()
            .find(|state| {
                state
                    .actions
                    .iter()
                    .any(|op| matches!(op, HandleStateOp::ResumeAfterSite { .. }))
            })
            .expect("expected a resume state");
        let resume_slot_name = resume_state
            .actions
            .iter()
            .find_map(|op| match op {
                HandleStateOp::ResumeAfterSite {
                    reason: ResumeAfterSiteReason::Call,
                    resume_slot: Some(slot),
                    ..
                } => Some(slot.name.clone()),
                _ => None,
            })
            .expect("resume state should allocate a synthetic resume slot");

        let step_bind = resume_state
            .actions
            .iter()
            .find_map(|op| match op {
                HandleStateOp::BindLocal { decl, .. } if decl.name.as_deref() == Some("step") => {
                    Some(decl)
                }
                _ => None,
            })
            .expect("round-tripped resume state should bind the direct step local");
        let bind_init = step_bind
            .init
            .as_ref()
            .expect("step local should keep an initializer");

        match &bind_init.kind {
            hir::ExprKind::VarRef(hir::ValueRef::Local { name, .. }) => {
                assert_eq!(name, &resume_slot_name);
                assert!(name.starts_with("__resume_site"));
            }
            other => {
                panic!("expected round-tripped rewritten task-drive step binding, got {other:?}")
            }
        }
    }

    #[test]
    fn source_plan_elides_enclosing_when_expr_after_when_arm_resume() {
        let source_plan = build_source_plan(
            r#"
package a

import scoop.core.*

effect Ask {
    fun ask(): String
}

fun demo(): Unit {
    handle {
        when (1) {
            1 -> {
                println("before_ask")
                val msg: String = Ask.ask()
                println("after_ask")
                println(msg)
            }
            else -> println("else_arm")
        }
        println("after_when")
    } with {
        Ask.ask(), k -> {
            println("ask_arm")
        }
    }
}
"#,
        );

        let resume_state = source_plan
            .states
            .iter()
            .find(|state| {
                state
                    .actions
                    .iter()
                    .any(|op| matches!(op, HandleStateOp::ResumeAfterSite { .. }))
            })
            .expect("expected a resume state");

        assert!(
            resume_state
                .actions
                .iter()
                .any(|op| matches!(op, HandleStateOp::BindLocal { .. })),
            "resume state should still bind the resumed arm local"
        );
        assert!(
            !resume_state
                .actions
                .iter()
                .any(|op| matches!(op, HandleStateOp::WhenExpr { .. })),
            "resume state should not keep the enclosing when-expr once arm tail is materialized"
        );
    }

    #[test]
    fn source_plan_rewrites_nested_when_consumer_to_materialized_arm_tail_block() {
        let source_plan = build_source_plan(
            r#"
package a

import scoop.core.*

effect Ask {
    fun ask(): String
}

fun demo(): Unit {
    handle {
        println(when (1) {
            1 -> {
                println("before_ask")
                val msg: String = Ask.ask()
                println("after_ask")
                msg
            }
            else -> "else_arm"
        })
        println("after_when")
    } with {
        Ask.ask(), k -> {
            println("ask_arm")
        }
    }
}
"#,
        );

        let resume_state = source_plan
            .states
            .iter()
            .find(|state| {
                state
                    .actions
                    .iter()
                    .any(|op| matches!(op, HandleStateOp::ResumeAfterSite { .. }))
            })
            .expect("expected a resume state");
        let resume_slot_name = resume_state
            .actions
            .iter()
            .find_map(|op| match op {
                HandleStateOp::ResumeAfterSite {
                    resume_slot: Some(slot),
                    ..
                } => Some(slot.name.clone()),
                _ => None,
            })
            .expect("resume state should allocate a synthetic resume slot");
        let outer_call = resume_state
            .actions
            .iter()
            .find_map(|op| match op {
                HandleStateOp::Call { expr } => Some(expr.as_ref()),
                _ => None,
            })
            .expect("resume state should keep the outer consumer call");

        assert!(
            !resume_state
                .actions
                .iter()
                .any(|op| matches!(op, HandleStateOp::BindLocal { .. })),
            "resume state should not keep standalone arm-tail bindings once the enclosing consumer owns the rebuilt tail"
        );
        assert!(
            !resume_state
                .actions
                .iter()
                .any(|op| matches!(op, HandleStateOp::WhenExpr { .. })),
            "resume state should not keep the enclosing when-expr once a later consumer is rewritten"
        );

        let hir::ExprKind::Call { args, .. } = &outer_call.kind else {
            panic!("expected outer consumer call, got {:?}", outer_call.kind);
        };
        let Some(hir::CallArg::Positional(arg_expr)) = args.first() else {
            panic!("expected positional call arg");
        };
        let hir::ExprKind::Block(block) = &arg_expr.kind else {
            panic!(
                "expected rewritten call arg to be a materialized block, got {:?}",
                arg_expr.kind
            );
        };
        let Some(hir::Stmt {
            kind: hir::StmtKind::Val(decl),
            ..
        }) = block.stmts.first()
        else {
            panic!("expected rewritten tail block to start with a synthetic val init");
        };
        let Some(init) = decl.init.as_ref() else {
            panic!("expected synthetic val init to keep the resume payload");
        };
        match &init.kind {
            hir::ExprKind::VarRef(hir::ValueRef::Local { name, .. }) => {
                assert_eq!(name, &resume_slot_name);
                assert!(name.starts_with("__resume_site"));
            }
            other => panic!(
                "expected rewritten tail init to read the synthetic resume slot, got {other:?}"
            ),
        }
    }

    #[test]
    fn source_plan_rewrites_nested_call_arg_resume_tail_to_synthetic_local() {
        let source_plan = build_source_plan(
            r#"
package a

import scoop.core.*

effect Yield {
    fun next(): Int
}

fun add(a: Int, b: Int): Int {
    a + b
}

fun demo(): Int {
    val result: Int = handle {
        val y: Int = add(Yield.next() + 1, 2)
        y
    } with {
        Yield.next() , k -> {
            k.resume(41)
        }
    }
    result
}
"#,
        );

        let resume_state = source_plan
            .states
            .iter()
            .find(|state| {
                state
                    .actions
                    .iter()
                    .any(|op| matches!(op, HandleStateOp::ResumeAfterSite { .. }))
            })
            .expect("expected a resume state");
        let resume_slot_name = resume_state
            .actions
            .iter()
            .find_map(|op| match op {
                HandleStateOp::ResumeAfterSite {
                    resume_slot: Some(slot),
                    ..
                } => Some(slot.name.clone()),
                _ => None,
            })
            .expect("resume state should allocate a synthetic resume slot");

        let binary_expr = resume_state
            .actions
            .iter()
            .find_map(|op| match op {
                HandleStateOp::BinaryExpr { expr } => Some(expr.as_ref()),
                _ => None,
            })
            .expect("expected binary tail in resume state");
        let call_expr = resume_state
            .actions
            .iter()
            .find_map(|op| match op {
                HandleStateOp::Call { expr } => Some(expr.as_ref()),
                _ => None,
            })
            .expect("expected call tail in resume state");

        match &binary_expr.kind {
            hir::ExprKind::Binary { lhs, .. } => match &lhs.kind {
                hir::ExprKind::VarRef(hir::ValueRef::Local { name, .. }) => {
                    assert_eq!(name, &resume_slot_name);
                }
                other => panic!("expected rewritten binary lhs, got {other:?}"),
            },
            other => panic!("expected binary expr, got {other:?}"),
        }

        match &call_expr.kind {
            hir::ExprKind::Call { args, .. } => match args.first() {
                Some(hir::CallArg::Positional(arg0)) => match &arg0.kind {
                    hir::ExprKind::Binary { lhs, .. } => match &lhs.kind {
                        hir::ExprKind::VarRef(hir::ValueRef::Local { name, .. }) => {
                            assert_eq!(name, &resume_slot_name);
                        }
                        other => panic!("expected rewritten call-arg lhs, got {other:?}"),
                    },
                    other => panic!("expected binary call arg, got {other:?}"),
                },
                other => panic!("expected first positional arg, got {other:?}"),
            },
            other => panic!("expected call expr, got {other:?}"),
        }
    }

    #[test]
    fn source_plan_clones_if_expr_merge_consumer_for_resume_path() {
        let source_plan = build_source_plan(
            r#"
package a

import scoop.core.*

effect Yield {
    fun next(): Int
}

fun demo(): Int {
    val result: Int = handle {
        val x: Int = if (true) Yield.next() else 0
        x
    } with {
        Yield.next() , k -> {
            k.resume(41)
        }
    }
    result
}
"#,
        );

        let resume_state = source_plan
            .states
            .iter()
            .find(|state| {
                state
                    .actions
                    .iter()
                    .any(|op| matches!(op, HandleStateOp::ResumeAfterSite { .. }))
            })
            .expect("expected a resume state");
        let resume_slot_name = resume_state
            .actions
            .iter()
            .find_map(|op| match op {
                HandleStateOp::ResumeAfterSite {
                    resume_slot: Some(slot),
                    ..
                } => Some(slot.name.clone()),
                _ => None,
            })
            .expect("resume state should allocate a synthetic resume slot");

        let StateTerminator::Goto(cloned_merge_id) = resume_state.terminator else {
            panic!("resume state should jump into a cloned merge consumer state");
        };
        let cloned_merge = source_plan
            .states
            .iter()
            .find(|state| state.id == cloned_merge_id)
            .expect("cloned merge state should exist");
        assert!(
            cloned_merge.label.contains("resume.site"),
            "resume path should jump to a synthetic cloned state, got {}",
            cloned_merge.label
        );

        let else_state = source_plan
            .states
            .iter()
            .find(|state| state.label == "if.else")
            .expect("expected original else state");
        let StateTerminator::Goto(original_merge_id) = else_state.terminator else {
            panic!("original else state should still jump to the shared merge state");
        };
        assert_ne!(
            cloned_merge_id, original_merge_id,
            "resume path must not reuse the original shared merge state"
        );

        let original_merge = source_plan
            .states
            .iter()
            .find(|state| state.id == original_merge_id)
            .expect("original merge state should exist");

        let original_init = original_merge
            .actions
            .iter()
            .find_map(|op| match op {
                HandleStateOp::BindLocal { decl, .. } if decl.name.as_deref() == Some("x") => {
                    decl.init.as_ref()
                }
                _ => None,
            })
            .expect("original merge state should still bind `x`");
        match &original_init.kind {
            hir::ExprKind::If { then_branch, .. } => {
                assert!(
                    matches!(then_branch.kind, hir::ExprKind::Perform { .. }),
                    "shared merge state must keep the original if expr for the non-resume path"
                );
            }
            other => panic!("expected original init to stay as an if expr, got {other:?}"),
        }

        let cloned_init = cloned_merge
            .actions
            .iter()
            .find_map(|op| match op {
                HandleStateOp::BindLocal { decl, .. } if decl.name.as_deref() == Some("x") => {
                    decl.init.as_ref()
                }
                _ => None,
            })
            .expect("cloned merge state should still bind `x`");
        match &cloned_init.kind {
            hir::ExprKind::If { then_branch, .. } => match &then_branch.kind {
                hir::ExprKind::VarRef(hir::ValueRef::Local { name, .. }) => {
                    assert_eq!(name, &resume_slot_name);
                }
                other => panic!(
                    "expected cloned then-branch to read the synthetic resume slot, got {other:?}"
                ),
            },
            other => panic!("expected cloned init to stay as an if expr, got {other:?}"),
        }
    }

    #[test]
    fn source_plan_preserves_same_statement_escape_replay_prefix_for_nested_block_call_site() {
        let source_plan = build_source_plan(
            r#"
package a

import scoop.core.*

effect Ask {
    fun ask(seed: Int): String
}

fun fetch(seed: Int): String / (Ask) {
    val msg: String = Ask.ask(seed + 1)
    println(msg)
    msg
}

fun demo(): Int {
    handle {
        do {
            val first: String = Ask.ask(10)
            println(first)
            val second: String = fetch(20)
            println(second)
        }
        0
    } with {
        Ask.ask(seed), k -> {
            println(seed)
            7
        }
    }
}
"#,
        );

        let call_site = source_plan
            .suspend_sites
            .iter()
            .find(|site| {
                matches!(
                    site.kind,
                    SuspendSiteKind::CallMaySuspend { .. }
                        | SuspendSiteKind::CallStateMachineCallee { .. }
                ) && site.escape_resume_target.is_some()
            })
            .expect("expected indirect call site to gain an escape replay target");

        let owner_state = source_plan
            .states
            .iter()
            .find(|state| state.id == call_site.owner_state)
            .expect("call-site owner state should exist");
        assert!(
            matches!(
                owner_state.actions.first(),
                Some(HandleStateOp::ResumeAfterSite { .. })
            ),
            "owner state should still begin with the preceding ResumeAfterSite marker"
        );

        let replay_state = source_plan
            .states
            .iter()
            .find(|state| Some(state.id) == call_site.escape_resume_target)
            .expect("escape replay state should exist");
        assert_ne!(
            replay_state.id, owner_state.id,
            "escape replay target must be a distinct synthetic state"
        );
        assert!(
            !matches!(
                replay_state.actions.first(),
                Some(HandleStateOp::ResumeAfterSite { .. })
            ),
            "escape replay state must skip the stale ResumeAfterSite marker so the new resume payload survives until the later call boundary"
        );
        assert!(
            matches!(
                replay_state.actions.first(),
                Some(HandleStateOp::BindLocal { .. })
            ),
            "same-statement escape replay should keep the already-executed nested-block prefix before the indirect site"
        );
        assert!(
            matches!(
                replay_state.terminator,
                StateTerminator::Suspend { site_id } if site_id == call_site.id
            ),
            "escape replay state must still suspend at the same indirect call site"
        );
    }

    #[test]
    fn source_plan_trims_escape_replay_to_current_top_level_statement() {
        let source_plan = build_source_plan(
            r#"
package a

import scoop.core.*

effect Ask {
    fun ask(seed: Int): String
}

fun fetch(seed: Int): String / (Ask) {
    val msg: String = Ask.ask(seed + 1)
    println(msg)
    msg
}

fun demo(): Int {
    handle {
        val first: String = Ask.ask(10)
        println(first)
        val second: String = fetch(20)
        println(second)
        0
    } with {
        Ask.ask(seed), k -> {
            println(seed)
            7
        }
    }
}
"#,
        );

        let call_site = source_plan
            .suspend_sites
            .iter()
            .find(|site| {
                matches!(
                    site.kind,
                    SuspendSiteKind::CallMaySuspend { .. }
                        | SuspendSiteKind::CallStateMachineCallee { .. }
                ) && site.escape_resume_target.is_some()
            })
            .expect("expected indirect call site to gain an escape replay target");

        let owner_state = source_plan
            .states
            .iter()
            .find(|state| state.id == call_site.owner_state)
            .expect("call-site owner state should exist");
        assert!(
            matches!(
                owner_state.actions.first(),
                Some(HandleStateOp::ResumeAfterSite { .. })
            ),
            "owner state should still begin with the preceding ResumeAfterSite marker"
        );

        let replay_state = source_plan
            .states
            .iter()
            .find(|state| Some(state.id) == call_site.escape_resume_target)
            .expect("escape replay state should exist");
        assert_eq!(
            replay_state.actions.len(),
            1,
            "top-level replay should start at the current statement boundary instead of replaying earlier completed statements"
        );
        assert!(
            matches!(
                replay_state.actions.first(),
                Some(HandleStateOp::SuspendCall { site_id, .. }) if *site_id == call_site.id
            ),
            "top-level replay should only keep the current indirect call boundary before re-suspending"
        );
        assert!(
            matches!(
                replay_state.terminator,
                StateTerminator::Suspend { site_id } if site_id == call_site.id
            ),
            "escape replay state must still suspend at the same indirect call site"
        );
    }

    #[test]
    fn source_plan_does_not_assign_escape_replay_target_for_later_perform_site() {
        let source_plan = build_source_plan(
            r#"
package a

import scoop.core.*

effect Yield {
    fun next(): Int
}

class Cell(var k: Continuation<Int, Unit>?)

fun demo(): Int {
    val none_k: Continuation<Int, Unit>? = None()
    val cell: Cell = Cell(none_k)

    val _: Unit = handle {
        val first: Int = Yield.next()
        println(first)
        val second: Int = Yield.next()
        println(second)
    } with {
        Yield.next(), k -> {
            cell.k = Some(k)
        }
    }

    0
}
"#,
        );

        let mut perform_sites = source_plan
            .suspend_sites
            .iter()
            .filter(|site| matches!(site.kind, SuspendSiteKind::Perform { .. }))
            .collect::<Vec<_>>();
        perform_sites.sort_by_key(|site| site.id);
        assert_eq!(
            perform_sites.len(),
            2,
            "two direct perform sites should be present in the handle body"
        );

        let second_site = perform_sites[1];
        let owner_state = source_plan
            .states
            .iter()
            .find(|state| state.id == second_site.owner_state)
            .expect("later perform-site owner state should exist");
        assert!(
            matches!(
                owner_state.actions.first(),
                Some(HandleStateOp::ResumeAfterSite { .. })
            ),
            "the later perform site should still live in a state that starts from an earlier resume marker"
        );
        assert_eq!(
            second_site.escape_resume_target, None,
            "direct perform continuations must resume at their dedicated post-perform state instead of replaying the earlier owner-state path"
        );
    }

    #[test]
    fn source_plan_preserves_outer_slot_types_and_while_cond_reads_for_resume_capture() {
        let source = r#"
package a

import scoop.core.*

effect Yield {
    fun next(): Int
}

fun demo(limit: Int): Int {
    val result: Int = handle {
        var i: Int = 0
        while (i < limit) {
            val x: Int = Yield.next()
            if (x == 1) {
                println("x_ok")
            } else {
                println("x_bad")
            }
            i = i + 1
        }
        i
    } with {
        Yield.next() , k -> {
            k.resume(1)
        }
    }
    result
}
"#;
        let lowered = lower_typed_single_source(source);
        let (fun, _) = first_handle_in_file(&lowered.file).expect("expected a handle expression");
        let source_plan = build_source_plan(source);
        let limit_param = &fun.params[0];
        let limit_slot = source_plan
            .frame_layout
            .slots
            .get(&limit_param.id)
            .expect("expected frame slot for outer param `limit`");
        assert_eq!(limit_slot.ty, limit_param.ty);

        let i_slot = source_plan
            .frame_layout
            .slots
            .values()
            .find(|slot| slot.name == "i")
            .expect("expected frame slot for loop local `i`");
        let perform_site = source_plan
            .suspend_sites
            .iter()
            .find(|site| matches!(site.kind, SuspendSiteKind::Perform { .. }))
            .expect("expected perform suspend site");
        assert!(
            perform_site.capture_locals.contains(&limit_param.id),
            "while condition outer param must be captured across resume"
        );
        assert!(
            perform_site.capture_locals.contains(&i_slot.id),
            "loop local used by the next while condition must stay captured"
        );
    }

    fn build_plan_dump(source_text: &str) -> String {
        let lowered = lower_typed_single_source(source_text);
        let (fun, handle) =
            first_handle_in_file(&lowered.file).expect("expected a handle expression");
        let context = collect_plan_context(&lowered, fun);
        HandleStateMachinePlan::build_with_context(&lowered.types, handle, &context)
            .pretty_dump(&lowered.types)
    }

    fn build_source_plan(source_text: &str) -> HandleStateMachinePlan {
        let lowered = lower_typed_single_source(source_text);
        let (fun, handle) =
            first_handle_in_file(&lowered.file).expect("expected a handle expression");
        let context = collect_plan_context(&lowered, fun);
        HandleStateMachinePlan::build_with_context(&lowered.types, handle, &context)
    }

    fn build_segment_list(source_text: &str) -> HandleSegmentList {
        build_source_plan(source_text).build_segment_list()
    }

    fn build_segment_dump(source_text: &str) -> String {
        let lowered = lower_typed_single_source(source_text);
        let segment_list = build_segment_list(source_text);
        segment_list
            .validate_builder_contract()
            .expect("segment builder contract should hold");
        segment_list.pretty_dump(&lowered.types)
    }

    fn build_round_tripped_plan_dump(source_text: &str) -> String {
        let lowered = lower_typed_single_source(source_text);
        let plan = build_round_tripped_plan(source_text);
        plan.pretty_dump(&lowered.types)
    }

    fn build_round_tripped_plan(source_text: &str) -> HandleStateMachinePlan {
        let source_plan = build_source_plan(source_text);
        let segment_list = source_plan.build_segment_list();
        segment_list
            .validate_builder_contract()
            .expect("segment builder contract should hold");
        HandleStateMachinePlan::build_from_segments(&segment_list)
            .expect("segment-only builder should reconstruct full plan")
    }

    fn collect_plan_exec_signature(
        plan: &HandleStateMachinePlan,
    ) -> Vec<(String, Vec<usize>, Option<usize>)> {
        plan.states
            .iter()
            .map(|state| {
                let branch_sig = match &state.terminator {
                    StateTerminator::Branch { condition, .. } => {
                        Some(condition.structural_signature())
                    }
                    _ => None,
                };
                (
                    state.label.clone(),
                    state
                        .actions
                        .iter()
                        .map(HandleStateOp::structural_signature)
                        .collect(),
                    branch_sig,
                )
            })
            .collect()
    }

    fn segment_slot_id_named(segment_list: &HandleSegmentList, name: &str) -> hir::SymbolId {
        segment_list
            .frame_slots
            .iter()
            .find(|slot| slot.name == name)
            .map(|slot| slot.id)
            .unwrap_or_else(|| panic!("expected frame slot named {name}"))
    }

    fn lower_typed_single_source(source_text: &str) -> hir::LoweredHir {
        lower_typed_single_source_with_source(source_text).1
    }

    fn lower_typed_single_source_with_source(source_text: &str) -> (SourceFile, hir::LoweredHir) {
        let session = legacy_session();
        let source = SourceFile::new_virtual("<mem>", source_text);
        let mut ast = parse_file(&source).unwrap();

        let index = {
            let mut pairs: Vec<(&SourceFile, &ast::File)> = Vec::new();
            for file in &session.sysroot().files {
                pairs.push((&file.source, &file.ast));
            }
            pairs.push((&source, &ast));
            Index::build(&pairs).unwrap()
        };

        let headers = crate::resolve::check_file_headers(&source, &ast, &index).unwrap();
        crate::resolve::check_file_bodies(&source, &mut ast, &index, &headers).unwrap();

        let mut env = typecheck::TypeEnv::from_sysroot(session.sysroot(), &index).unwrap();
        env.extend_from_file(&source, &ast, &index).unwrap();

        let mut typecheck_types = TypeStore::new();
        let builtins = typecheck_types.intern_builtins();
        typecheck::check_file_annotations(
            &source,
            &ast,
            &index,
            &headers.imports,
            &env,
            &mut typecheck_types,
            builtins,
        )
        .unwrap();
        typecheck::check_file_type_refs(
            &source,
            &ast,
            &index,
            &headers.imports,
            &env,
            &mut typecheck_types,
            builtins,
        )
        .unwrap();
        typecheck::check_file_exprs(
            &source,
            &ast,
            &index,
            &headers.imports,
            &env,
            &mut typecheck_types,
            builtins,
        )
        .unwrap();

        let mut unit: Vec<(&SourceFile, &ast::File)> = Vec::new();
        for file in &session.sysroot().files {
            unit.push((&file.source, &file.ast));
        }
        unit.push((&source, &ast));

        let lowered = hir::lower_for_compilation_unit_multi_files(
            &source,
            &index,
            &unit,
            &[(&source, &ast)],
            &[],
            &typecheck_types,
        )
        .unwrap();

        (source, lowered)
    }

    fn legacy_session() -> Session {
        Session::with_options(SessionOptions::new(EffectPipelineMode::Legacy)).unwrap()
    }

    fn first_handle_in_file(file: &hir::File) -> Option<(&hir::FunDecl, &hir::HandleExpr)> {
        for item in &file.items {
            if let hir::Item::Fun(fun) = item
                && let Some(body) = &fun.body
                && let Some(handle) = first_handle_in_block(body)
            {
                return Some((fun, handle));
            }
        }
        None
    }

    fn first_handle_in_block(block: &hir::Block) -> Option<&hir::HandleExpr> {
        for stmt in &block.stmts {
            if let Some(handle) = first_handle_in_stmt(stmt) {
                return Some(handle);
            }
        }
        None
    }

    fn first_handle_in_stmt(stmt: &hir::Stmt) -> Option<&hir::HandleExpr> {
        match &stmt.kind {
            hir::StmtKind::Expr(expr) => first_handle_in_expr(expr),
            hir::StmtKind::Val(decl) => decl.init.as_ref().and_then(first_handle_in_expr),
            hir::StmtKind::Assign { lhs, rhs, .. } => {
                first_handle_in_expr(lhs).or_else(|| first_handle_in_expr(rhs))
            }
            hir::StmtKind::While { cond, body } => {
                first_handle_in_expr(cond).or_else(|| first_handle_in_block(body))
            }
            hir::StmtKind::Return { value } => value.as_ref().and_then(first_handle_in_expr),
            hir::StmtKind::Empty
            | hir::StmtKind::Break { .. }
            | hir::StmtKind::Continue { .. }
            | hir::StmtKind::Todo(_) => None,
        }
    }

    fn first_handle_in_expr(expr: &hir::Expr) -> Option<&hir::HandleExpr> {
        match &expr.kind {
            hir::ExprKind::Handle(handle) => Some(handle),
            hir::ExprKind::Block(block) => first_handle_in_block(block),
            hir::ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => first_handle_in_expr(cond)
                .or_else(|| first_handle_in_expr(then_branch))
                .or_else(|| else_branch.as_deref().and_then(first_handle_in_expr)),
            hir::ExprKind::Call { callee, args } => first_handle_in_expr(callee).or_else(|| {
                args.iter().find_map(|arg| match arg {
                    hir::CallArg::Positional(expr) => first_handle_in_expr(expr),
                    hir::CallArg::Named { value, .. } => first_handle_in_expr(value),
                })
            }),
            hir::ExprKind::StructLit { fields, .. } => fields
                .iter()
                .find_map(|field| first_handle_in_expr(&field.value)),
            hir::ExprKind::TupleLit { elements } => elements.iter().find_map(first_handle_in_expr),
            hir::ExprKind::InterpolatedString { parts, .. } => parts.iter().find_map(|part| {
                if let hir::InterpolatedStringPart::Expr { expr } = part {
                    first_handle_in_expr(expr)
                } else {
                    None
                }
            }),
            hir::ExprKind::Unary { expr: inner, .. }
            | hir::ExprKind::Cast { expr: inner, .. }
            | hir::ExprKind::TypeCheck { expr: inner, .. }
            | hir::ExprKind::MemberAccess {
                receiver: inner, ..
            } => first_handle_in_expr(inner),
            hir::ExprKind::Binary { lhs, rhs, .. } => {
                first_handle_in_expr(lhs).or_else(|| first_handle_in_expr(rhs))
            }
            hir::ExprKind::When { subject, arms } => first_handle_in_expr(subject).or_else(|| {
                arms.iter().find_map(|arm| {
                    arm.guard
                        .as_ref()
                        .and_then(first_handle_in_expr)
                        .or_else(|| first_handle_in_expr(&arm.body))
                })
            }),
            hir::ExprKind::Closure(closure) => first_handle_in_expr(&closure.body),
            hir::ExprKind::Perform { args, .. } => args.iter().find_map(|arg| match arg {
                hir::CallArg::Positional(expr) => first_handle_in_expr(expr),
                hir::CallArg::Named { value, .. } => first_handle_in_expr(value),
            }),
            hir::ExprKind::Missing
            | hir::ExprKind::Literal(_)
            | hir::ExprKind::VarRef(_)
            | hir::ExprKind::UnresolvedIdent { .. }
            | hir::ExprKind::ClassLiteral(_)
            | hir::ExprKind::Todo(_) => None,
        }
    }

    fn collect_plan_context(
        lowered: &hir::LoweredHir,
        owner_fun: &hir::FunDecl,
    ) -> HandlePlanContext {
        collect_effect_analysis_context_for_fun(lowered, owner_fun)
    }
}
