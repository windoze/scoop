type HandleSegmentId = PlanStateId;

#[derive(Debug, Clone)]
pub(super) struct HandleSegmentList {
    handle_span: Span,
    result_ty: TypeId,
    entry_segment: HandleSegmentId,
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
    resume_mode: ArmResumeMode,
    body_exit: ArmBodyExit,
}

#[derive(Debug, Clone)]
struct HandleSegmentArmBody {
    arm_id: ArmPlanId,
    op_fqn: String,
    resume_mode: ArmResumeMode,
    body_entry_segment: HandleSegmentId,
    body_segments: Vec<HandleSegmentId>,
    body_exit: ArmBodyExit,
    binder_slots: Vec<FrameSlot>,
    capture_locals: Vec<hir::SymbolId>,
    detach_policy: String,
    cleanup_scope_stack: Vec<CleanupScopeId>,
}

#[derive(Debug, Clone)]
struct HandleSegment {
    id: HandleSegmentId,
    label: String,
    source_span: Option<Span>,
    dispatch_context: HandleSegmentDispatchContext,
    cleanup_scope_stack: Vec<CleanupScopeId>,
    ops: Vec<String>,
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
        resume_mode: ArmResumeMode,
    },
}

#[derive(Debug, Clone)]
enum HandleSegmentTerminator {
    Goto {
        next_segment: HandleSegmentId,
    },
    Branch {
        condition: String,
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

#[derive(Debug, Clone, Copy)]
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
    matching_arms: Vec<ArmPlanId>,
    available_locals: Vec<hir::SymbolId>,
    capture_locals: Vec<hir::SymbolId>,
    source_path: Option<SuspendSourcePath>,
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
}

impl HandleSegmentList {
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
            &plan.arm_plans,
            &plan.cleanup_scopes,
            &state_cleanup_execution_scopes,
            &arm_bodies,
        );
        let dispatch_entries = build_segment_dispatch_entries(&plan.dispatch_plan, &arm_bodies);
        let suspend_owners = build_suspend_owner_segments(&plan.states);
        let suspend_sites = plan
            .suspend_sites
            .iter()
            .map(|site| {
                HandleSegmentSuspendSite::from_plan(
                    site,
                    *suspend_owners
                        .get(&site.id)
                        .expect("segment projection missing suspend owner state"),
                )
            })
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

    #[cfg(test)]
    pub(super) fn pretty_dump(&self, types: &TypeStore) -> String {
        let mut out = String::new();
        self.write_pretty_dump(types, 0, &mut out);
        out
    }

    #[cfg(test)]
    fn write_pretty_dump(&self, types: &TypeStore, indent: usize, out: &mut String) {
        let pad = " ".repeat(indent);
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
                out.push_str(&format!(
                    "{pad}  {} => [{}]\n",
                    entry.op_fqn,
                    targets
                ));
            }
        }

        out.push_str(&format!("{pad}arm-bodies:\n"));
        if self.arm_bodies.is_empty() {
            out.push_str(&format!("{pad}  []\n"));
        } else {
            for arm in &self.arm_bodies {
                out.push_str(&format!(
                    "{pad}  arm{} op={} mode={} entry=seg{} segments=[{}] exit={} detach={}\n",
                    arm.arm_id,
                    arm.op_fqn,
                    arm.resume_mode.label(),
                    arm.body_entry_segment,
                    render_segment_ids(&arm.body_segments),
                    arm.body_exit.label(),
                    arm.detach_policy
                ));
                out.push_str(&format!(
                    "{pad}    binders=[{}]\n",
                    render_segment_frame_slots(&arm.binder_slots, types)
                ));
                out.push_str(&format!(
                    "{pad}    captures=[{}]\n",
                    render_segment_symbol_ids(&arm.capture_locals)
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
                let available = render_segment_symbol_ids(&site.available_locals);
                let captures = render_segment_symbol_ids(&site.capture_locals);
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
                if let Some(detail) = site.kind.detail() {
                    out.push_str(&format!("{pad}    detail={detail}\n"));
                }
                if let Some(source_path) = &site.source_path {
                    out.push_str(&format!("{pad}    path={}\n", source_path.label()));
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
        for segment in &self.segments {
            let source_span = segment
                .source_span
                .map_or_else(|| "none".to_string(), |span| format!("{span:?}"));
            out.push_str(&format!(
                "{pad}  seg{} {} span={source_span}:\n",
                segment.id,
                segment.label
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
                out.push_str(&format!("{pad}    {op}\n"));
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
            acc ^= op.len();
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
            Self::Arm {
                arm_id,
                resume_mode,
            } => 3 ^ (arm_id as usize) ^ (resume_mode.structural_signature() << 1),
        }
    }

    #[cfg(test)]
    fn label(self) -> String {
        match self {
            Self::Main => "handle-body".to_string(),
            Self::Cleanup { scope_id, kind } => {
                format!("cleanup-body cleanup{scope_id} kind={}", kind.label())
            }
            Self::Arm {
                arm_id,
                resume_mode,
            } => format!(
                "arm-body arm{arm_id} mode={}",
                resume_mode.label()
            ),
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
            Self::CleanupEnter {
                next_segment, ..
            } => vec![HandleSegmentEdge {
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
                condition.len()
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

    #[cfg(test)]
    fn label(&self) -> String {
        match self {
            Self::Goto { next_segment } => format!("goto seg{next_segment}"),
            Self::Branch {
                condition,
                then_segment,
                else_segment,
                merge_segment,
            } => format!(
                "branch cond={condition} then=seg{then_segment} else=seg{else_segment} merge=seg{merge_segment}"
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

    #[cfg(test)]
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
        self.from as usize
            ^ ((self.to as usize) << 1)
            ^ (self.kind.structural_signature() << 2)
    }
}

impl HandleSegmentDispatchEntry {
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
        self.arm_id as usize
            ^ ((self.entry_segment as usize) << 1)
            ^ (self.resume_mode.structural_signature() << 2)
            ^ (self.body_exit.structural_signature() << 3)
    }

    #[cfg(test)]
    fn label(&self) -> String {
        format!(
            "arm{}(entry=seg{} exit={} mode={})",
            self.arm_id,
            self.entry_segment,
            self.body_exit.label(),
            self.resume_mode.label()
        )
    }
}

impl HandleSegmentArmBody {
    fn structural_signature(&self) -> usize {
        let mut acc = self.arm_id as usize
            ^ self.op_fqn.len()
            ^ (self.resume_mode.structural_signature() << 1)
            ^ (self.body_entry_segment as usize)
            ^ (self.body_exit.structural_signature() << 2)
            ^ self.detach_policy.len();
        for segment_id in &self.body_segments {
            acc ^= (*segment_id as usize) << 3;
        }
        for slot in &self.binder_slots {
            acc ^= slot.structural_signature();
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
    fn from_plan(site: &SuspendSitePlan, owner_segment: HandleSegmentId) -> Self {
        Self {
            id: site.id,
            span: site.span,
            kind: site.kind.clone(),
            owner_segment,
            resume_segment: site.resume_target,
            matching_arms: site.matching_arms.clone(),
            available_locals: site.available_locals.clone(),
            capture_locals: site.capture_locals.clone(),
            source_path: site.source_path.clone(),
        }
    }

    fn structural_signature(&self) -> usize {
        let mut acc = self.id as usize
            ^ self.span.start
            ^ self.span.end
            ^ ((self.owner_segment as usize) << 1)
            ^ self.resume_segment as usize
            ^ self.kind.structural_signature();
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
        acc
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
}

#[cfg(test)]
fn render_segment_symbol_ids(ids: &[hir::SymbolId]) -> String {
    let mut labels = ids
        .iter()
        .map(|id| format!("local#{}", id.as_u32()))
        .collect::<Vec<_>>();
    labels.sort();
    labels.join(", ")
}

#[cfg(test)]
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
fn render_segment_frame_slots(slots: &[FrameSlot], types: &TypeStore) -> String {
    slots.iter()
        .map(|slot| format!("{}:{}", slot.display_name(), types.display(slot.ty)))
        .collect::<Vec<_>>()
        .join(", ")
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
                StateTerminator::Suspend { site_id } => vec![*resume_targets
                    .get(site_id)
                    .expect("segment successor map missing suspend resume target")],
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
        let region = collect_state_region(
            scope.entry_state,
            &states_by_id,
            successors,
            |state| state.id == scope.exit_state,
        );
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
    let mut all_scope_ids = cleanup_scopes.iter().map(|scope| scope.id).collect::<Vec<_>>();
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
            let body_segments = collect_state_region(
                arm.body_entry_state,
                &states_by_id,
                successors,
                |state| matches!(state.terminator, StateTerminator::ArmExit(_)),
            );
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
                resume_mode: arm.resume_mode,
                body_entry_segment: arm.body_entry_state,
                body_segments,
                body_exit: arm.body_exit,
                binder_slots: arm.binder_slots.clone(),
                capture_locals: arm.capture_locals.clone(),
                detach_policy: arm.detach_policy.clone(),
                cleanup_scope_stack,
            }
        })
        .collect::<Vec<_>>();
    arm_bodies.sort_by_key(|arm| arm.arm_id);
    arm_bodies
}

fn build_state_dispatch_contexts(
    states: &[PlanState],
    arm_plans: &[ArmPlan],
    cleanup_scopes: &[CleanupScopePlan],
    state_cleanup_execution_scopes: &HashMap<PlanStateId, Vec<CleanupScopeId>>,
    arm_bodies: &[HandleSegmentArmBody],
) -> HashMap<PlanStateId, HandleSegmentDispatchContext> {
    let arm_modes = arm_plans
        .iter()
        .map(|arm| (arm.id, arm.resume_mode))
        .collect::<HashMap<_, _>>();
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
                HandleSegmentDispatchContext::Arm {
                    arm_id,
                    resume_mode: *arm_modes
                        .get(&arm_id)
                        .expect("segment dispatch context missing arm mode"),
                }
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
                        resume_mode: arm.resume_mode,
                        body_exit: arm.body_exit,
                    }
                })
                .collect(),
        })
        .collect()
}

fn build_suspend_owner_segments(states: &[PlanState]) -> HashMap<SuspendSiteId, HandleSegmentId> {
    states
        .iter()
        .filter_map(|state| match state.terminator {
            StateTerminator::Suspend { site_id } => Some((site_id, state.id)),
            _ => None,
        })
        .collect()
}
