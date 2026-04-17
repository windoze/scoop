type UnifiedStateId = HandleSegmentId;

/// Production-facing lowering contract: the single structured input that
/// a downstream LLVM emitter receives for an effect `handle` expression.
///
/// Constructed exclusively via `MainCodegen::build_unified_lowering_contract`.
/// All emitter-visible metadata is accessed through the enclosed
/// `UnifiedHandleStateMachine` and its typed accessors.
#[derive(Debug, Clone)]
pub(crate) struct UnifiedHandleLoweringContract {
    machine: UnifiedHandleStateMachine,
}

impl UnifiedHandleLoweringContract {
    /// Return the full unified state machine backing this contract.
    pub(crate) fn machine(&self) -> &UnifiedHandleStateMachine {
        &self.machine
    }

    // Convenience delegates — keep the emitter call-sites concise while
    // preserving the "all data flows through the contract" invariant.

    pub(crate) fn handle_span(&self) -> Span {
        self.machine.handle_span()
    }

    pub(crate) fn result_ty(&self) -> TypeId {
        self.machine.result_ty()
    }

    pub(crate) fn entry_state(&self) -> UnifiedStateId {
        self.machine.entry_state()
    }

    pub(crate) fn states(&self) -> &[UnifiedState] {
        self.machine.states()
    }

    pub(crate) fn dispatch_entries(&self) -> &[UnifiedDispatchEntry] {
        self.machine.dispatch_entries()
    }

    pub(crate) fn arms(&self) -> &[UnifiedArm] {
        self.machine.arms()
    }

    pub(crate) fn suspend_sites(&self) -> &[UnifiedSuspendSite] {
        self.machine.suspend_sites()
    }

    pub(crate) fn cleanup_scopes(&self) -> &[UnifiedCleanupScope] {
        self.machine.cleanup_scopes()
    }

    pub(crate) fn frame(&self) -> &UnifiedFrameSchema {
        self.machine.frame()
    }

    pub(crate) fn nested_handles(&self) -> &[UnifiedHandleStateMachine] {
        self.machine.nested_handles()
    }
}

#[derive(Debug, Clone)]
pub(crate) struct UnifiedHandleStateMachine {
    handle_span: Span,
    result_ty: TypeId,
    storage: UnifiedStateMachineStorage,
    entry_state: UnifiedStateId,
    frame: UnifiedFrameSchema,
    dispatch_entries: Vec<UnifiedDispatchEntry>,
    arms: Vec<UnifiedArm>,
    states: Vec<UnifiedState>,
    suspend_sites: Vec<UnifiedSuspendSite>,
    cleanup_scopes: Vec<UnifiedCleanupScope>,
    nested_handles: Vec<UnifiedHandleStateMachine>,
}

// ---- pub(crate) read-only accessors for downstream emitter consumption ----
impl UnifiedHandleStateMachine {
    pub(crate) fn handle_span(&self) -> Span {
        self.handle_span
    }

    pub(crate) fn result_ty(&self) -> TypeId {
        self.result_ty
    }

    pub(crate) fn storage(&self) -> UnifiedStateMachineStorage {
        self.storage
    }

    pub(crate) fn entry_state(&self) -> UnifiedStateId {
        self.entry_state
    }

    pub(crate) fn frame(&self) -> &UnifiedFrameSchema {
        &self.frame
    }

    pub(crate) fn dispatch_entries(&self) -> &[UnifiedDispatchEntry] {
        &self.dispatch_entries
    }

    pub(crate) fn arms(&self) -> &[UnifiedArm] {
        &self.arms
    }

    pub(crate) fn states(&self) -> &[UnifiedState] {
        &self.states
    }

    pub(crate) fn suspend_sites(&self) -> &[UnifiedSuspendSite] {
        &self.suspend_sites
    }

    pub(crate) fn cleanup_scopes(&self) -> &[UnifiedCleanupScope] {
        &self.cleanup_scopes
    }

    pub(crate) fn nested_handles(&self) -> &[UnifiedHandleStateMachine] {
        &self.nested_handles
    }

    /// Look up a state by id. Returns `None` if the id is not in the machine.
    pub(crate) fn get_state(&self, id: UnifiedStateId) -> Option<&UnifiedState> {
        self.states.iter().find(|s| s.id == id)
    }

    /// Look up a dispatch entry by operation FQN.
    pub(crate) fn get_dispatch_entry(&self, op_fqn: &str) -> Option<&UnifiedDispatchEntry> {
        self.dispatch_entries
            .iter()
            .find(|e| e.op_fqn == op_fqn)
    }

    /// Look up a suspend site by id.
    pub(crate) fn get_suspend_site(&self, id: SuspendSiteId) -> Option<&UnifiedSuspendSite> {
        self.suspend_sites.iter().find(|s| s.id == id)
    }

    /// Look up a cleanup scope by id.
    pub(crate) fn get_cleanup_scope(&self, id: CleanupScopeId) -> Option<&UnifiedCleanupScope> {
        self.cleanup_scopes.iter().find(|s| s.id == id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UnifiedStateMachineStorage {
    Heap,
}

#[derive(Debug, Clone)]
pub(crate) struct UnifiedFrameSchema {
    fields: Vec<UnifiedFrameField>,
    slots: Vec<UnifiedFrameSlot>,
}

impl UnifiedFrameSchema {
    pub(crate) fn fields(&self) -> &[UnifiedFrameField] {
        &self.fields
    }

    pub(crate) fn slots(&self) -> &[UnifiedFrameSlot] {
        &self.slots
    }

    /// Return the field index for a given slot id, if present.
    pub(crate) fn get_slot_field_index(&self, slot_id: hir::SymbolId) -> Option<usize> {
        self.slots
            .iter()
            .find(|slot| slot.slot.id == slot_id)
            .map(|slot| slot.field_index)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UnifiedFrameField {
    System(UnifiedFrameSystemField),
    Slot {
        slot_id: hir::SymbolId,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UnifiedFrameSystemField {
    StateTag,
    ResumeWord,
    ResumeGcRef,
    CleanupFlag,
    OneShotFlag,
}

#[derive(Debug, Clone)]
pub(crate) struct UnifiedFrameSlot {
    slot: FrameSlot,
    source: UnifiedFrameSlotSource,
    field_index: usize,
    listed_as_lifted_local: bool,
}

impl UnifiedFrameSlot {
    pub(crate) fn slot(&self) -> &FrameSlot {
        &self.slot
    }

    pub(crate) fn source(&self) -> UnifiedFrameSlotSource {
        self.source
    }

    pub(crate) fn field_index(&self) -> usize {
        self.field_index
    }

    pub(crate) fn listed_as_lifted_local(&self) -> bool {
        self.listed_as_lifted_local
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UnifiedFrameSlotSource {
    HandleBody,
    ArmBinder {
        arm_id: ArmPlanId,
    },
}

#[derive(Debug, Clone)]
pub(crate) struct UnifiedDispatchEntry {
    op_fqn: String,
    arms: Vec<UnifiedDispatchArm>,
}

impl UnifiedDispatchEntry {
    pub(crate) fn op_fqn(&self) -> &str {
        &self.op_fqn
    }

    pub(crate) fn arms(&self) -> &[UnifiedDispatchArm] {
        &self.arms
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct UnifiedDispatchArm {
    arm_id: ArmPlanId,
    entry_state: UnifiedStateId,
}

impl UnifiedDispatchArm {
    pub(crate) fn arm_id(&self) -> ArmPlanId {
        self.arm_id
    }

    pub(crate) fn entry_state(&self) -> UnifiedStateId {
        self.entry_state
    }
}

#[derive(Debug, Clone)]
pub(crate) struct UnifiedArm {
    arm_id: ArmPlanId,
    op_fqn: String,
    entry_state: UnifiedStateId,
    body_states: Vec<UnifiedStateId>,
    binder_slots: Vec<hir::SymbolId>,
    capture_locals: Vec<hir::SymbolId>,
    cleanup_scope_stack: Vec<CleanupScopeId>,
}

impl UnifiedArm {
    pub(crate) fn arm_id(&self) -> ArmPlanId {
        self.arm_id
    }

    pub(crate) fn op_fqn(&self) -> &str {
        &self.op_fqn
    }

    pub(crate) fn entry_state(&self) -> UnifiedStateId {
        self.entry_state
    }

    pub(crate) fn body_states(&self) -> &[UnifiedStateId] {
        &self.body_states
    }

    pub(crate) fn binder_slots(&self) -> &[hir::SymbolId] {
        &self.binder_slots
    }

    pub(crate) fn capture_locals(&self) -> &[hir::SymbolId] {
        &self.capture_locals
    }

    pub(crate) fn cleanup_scope_stack(&self) -> &[CleanupScopeId] {
        &self.cleanup_scope_stack
    }
}

#[derive(Debug, Clone)]
pub(crate) struct UnifiedState {
    id: UnifiedStateId,
    label: String,
    source_span: Option<Span>,
    context: UnifiedStateContext,
    cleanup_scope_stack: Vec<CleanupScopeId>,
    ops: Vec<HandleStateOp>,
    terminator: UnifiedStateTerminator,
    outgoing_edges: Vec<UnifiedStateEdge>,
}

impl UnifiedState {
    pub(crate) fn id(&self) -> UnifiedStateId {
        self.id
    }

    pub(crate) fn label(&self) -> &str {
        &self.label
    }

    pub(crate) fn source_span(&self) -> Option<Span> {
        self.source_span
    }

    pub(crate) fn context(&self) -> UnifiedStateContext {
        self.context
    }

    pub(crate) fn cleanup_scope_stack(&self) -> &[CleanupScopeId] {
        &self.cleanup_scope_stack
    }

    pub(crate) fn ops(&self) -> &[HandleStateOp] {
        &self.ops
    }

    pub(crate) fn terminator(&self) -> &UnifiedStateTerminator {
        &self.terminator
    }

    pub(crate) fn outgoing_edges(&self) -> &[UnifiedStateEdge] {
        &self.outgoing_edges
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UnifiedStateContext {
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
pub(crate) enum UnifiedStateTerminator {
    Goto {
        next_state: UnifiedStateId,
    },
    Branch {
        condition: HandleBranchCondition,
        then_state: UnifiedStateId,
        else_state: UnifiedStateId,
        merge_state: UnifiedStateId,
    },
    Suspend {
        site_id: SuspendSiteId,
        resume_state: UnifiedStateId,
    },
    CleanupEnter {
        scope_id: CleanupScopeId,
        next_state: UnifiedStateId,
    },
    ReturnHandle,
    ReturnFromFunction,
    ArmReturnHandle,
    ArmResumeMatchedSite,
    ArmMaterializeContinuation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct UnifiedStateEdge {
    to_state: UnifiedStateId,
    kind: UnifiedStateEdgeKind,
}

impl UnifiedStateEdge {
    pub(crate) fn target_state(&self) -> UnifiedStateId {
        self.to_state
    }

    pub(crate) fn kind(&self) -> UnifiedStateEdgeKind {
        self.kind
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UnifiedStateEdgeKind {
    Goto,
    BranchThen,
    BranchElse,
    SuspendResume,
    CleanupEnter,
}

#[derive(Debug, Clone)]
pub(crate) struct UnifiedSuspendSite {
    id: SuspendSiteId,
    span: Span,
    kind: SuspendSiteKind,
    owner_state: UnifiedStateId,
    resume_state: UnifiedStateId,
    matching_arms: Vec<ArmPlanId>,
    available_locals: Vec<hir::SymbolId>,
    capture_locals: Vec<hir::SymbolId>,
    source_path: Option<SuspendSourcePath>,
    resume_path: Option<SuspendResumePath>,
}

impl UnifiedSuspendSite {
    pub(crate) fn id(&self) -> SuspendSiteId {
        self.id
    }

    pub(crate) fn span(&self) -> Span {
        self.span
    }

    pub(crate) fn kind(&self) -> &SuspendSiteKind {
        &self.kind
    }

    pub(crate) fn owner_state(&self) -> UnifiedStateId {
        self.owner_state
    }

    pub(crate) fn resume_state(&self) -> UnifiedStateId {
        self.resume_state
    }

    pub(crate) fn matching_arms(&self) -> &[ArmPlanId] {
        &self.matching_arms
    }

    pub(crate) fn available_locals(&self) -> &[hir::SymbolId] {
        &self.available_locals
    }

    pub(crate) fn capture_locals(&self) -> &[hir::SymbolId] {
        &self.capture_locals
    }
}

#[derive(Debug, Clone)]
pub(crate) struct UnifiedCleanupScope {
    id: CleanupScopeId,
    kind: CleanupScopeKind,
    entry_state: UnifiedStateId,
    exit_state: UnifiedStateId,
    note: String,
}

impl UnifiedCleanupScope {
    pub(crate) fn id(&self) -> CleanupScopeId {
        self.id
    }

    pub(crate) fn kind(&self) -> CleanupScopeKind {
        self.kind
    }

    pub(crate) fn entry_state(&self) -> UnifiedStateId {
        self.entry_state
    }

    pub(crate) fn exit_state(&self) -> UnifiedStateId {
        self.exit_state
    }

    pub(crate) fn note(&self) -> &str {
        &self.note
    }
}

impl HandleSegmentList {
    fn build_unified_state_machine(&self) -> Result<UnifiedHandleStateMachine, String> {
        UnifiedHandleStateMachine::build_from_segments(self)
    }
}

impl UnifiedHandleStateMachine {
    // 当前阶段的 unified transformation 只允许把已经冻结的 segment contract
    // 重新投影成 canonical full machine。这里不读取 HIR，不回看上游源码结构，
    // 也不根据任何旧分流标签做额外推断。
    fn build_from_segments(segments: &HandleSegmentList) -> Result<Self, String> {
        segments.validate_builder_contract()?;

        let mut outgoing_edges_by_state = build_unified_outgoing_edges(&segments.edges);
        let states = segments
            .segments
            .iter()
            .map(|segment| {
                UnifiedState::from_segment(
                    segment,
                    outgoing_edges_by_state.remove(&segment.id).unwrap_or_default(),
                )
            })
            .collect::<Vec<_>>();

        let dispatch_entries = segments
            .dispatch_entries
            .iter()
            .map(UnifiedDispatchEntry::from_segment)
            .collect::<Vec<_>>();
        let arms = segments
            .arm_bodies
            .iter()
            .map(UnifiedArm::from_segment)
            .collect::<Vec<_>>();
        let suspend_sites = segments
            .suspend_sites
            .iter()
            .map(UnifiedSuspendSite::from_segment)
            .collect::<Vec<_>>();
        let cleanup_scopes = segments
            .cleanup_scopes
            .iter()
            .map(UnifiedCleanupScope::from_segment)
            .collect::<Vec<_>>();
        let nested_handles = segments
            .nested_handles
            .iter()
            .map(Self::build_from_segments)
            .collect::<Result<Vec<_>, _>>()?;

        let machine = Self {
            handle_span: segments.handle_span,
            result_ty: segments.result_ty,
            storage: UnifiedStateMachineStorage::Heap,
            entry_state: segments.entry_segment,
            frame: UnifiedFrameSchema::from_segments(&segments.frame_slots, &segments.lifted_locals),
            dispatch_entries,
            arms,
            states,
            suspend_sites,
            cleanup_scopes,
            nested_handles,
        };
        machine.validate_full_machine_contract()?;
        Ok(machine)
    }

    fn state(&self, id: UnifiedStateId) -> Option<&UnifiedState> {
        self.get_state(id)
    }

    fn validate_full_machine_contract(&self) -> Result<(), String> {
        self.validate_full_machine_contract_with_path("root")
    }

    fn validate_full_machine_contract_with_path(&self, path: &str) -> Result<(), String> {
        if !matches!(self.storage, UnifiedStateMachineStorage::Heap) {
            return Err(format!(
                "{path}: storage must be {} in this phase, got {}",
                UnifiedStateMachineStorage::Heap.label(),
                self.storage.label()
            ));
        }

        let mut arm_by_id = HashMap::<ArmPlanId, &UnifiedArm>::new();
        let mut previous_arm_id = None::<ArmPlanId>;
        for arm in &self.arms {
            if let Some(prev_id) = previous_arm_id
                && prev_id >= arm.arm_id
            {
                return Err(format!(
                    "{path}: arms[] is not strictly sorted by arm id at arm{}",
                    arm.arm_id
                ));
            }
            previous_arm_id = Some(arm.arm_id);
            if arm_by_id.insert(arm.arm_id, arm).is_some() {
                return Err(format!("{path}: duplicate arm arm{}", arm.arm_id));
            }
        }

        let frame_slots_by_id = self.frame.validate_contract(path, &arm_by_id)?;

        let mut state_by_id = HashMap::<UnifiedStateId, &UnifiedState>::new();
        let mut previous_state_id = None::<UnifiedStateId>;
        for state in &self.states {
            if let Some(prev_id) = previous_state_id
                && prev_id >= state.id
            {
                return Err(format!(
                    "{path}: states[] is not strictly sorted by state id at s{}",
                    state.id
                ));
            }
            previous_state_id = Some(state.id);
            if state_by_id.insert(state.id, state).is_some() {
                return Err(format!("{path}: duplicate state s{}", state.id));
            }
        }
        if !state_by_id.contains_key(&self.entry_state) {
            return Err(format!(
                "{path}: entry_state s{} is missing from states[]",
                self.entry_state
            ));
        }

        let mut cleanup_scope_by_id = HashMap::<CleanupScopeId, &UnifiedCleanupScope>::new();
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
            if cleanup_scope_by_id.insert(scope.id, scope).is_some() {
                return Err(format!("{path}: duplicate cleanup scope cleanup{}", scope.id));
            }
            if !state_by_id.contains_key(&scope.entry_state) {
                return Err(format!(
                    "{path}: cleanup{} entry state s{} is missing from states[]",
                    scope.id,
                    scope.entry_state
                ));
            }
            if !state_by_id.contains_key(&scope.exit_state) {
                return Err(format!(
                    "{path}: cleanup{} exit state s{} is missing from states[]",
                    scope.id,
                    scope.exit_state
                ));
            }
        }

        let mut suspend_site_by_id = HashMap::<SuspendSiteId, &UnifiedSuspendSite>::new();
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
            if suspend_site_by_id.insert(site.id, site).is_some() {
                return Err(format!("{path}: duplicate suspend site site{}", site.id));
            }
        }

        for state in &self.states {
            let mut previous_cleanup_stack_id = None::<CleanupScopeId>;
            for scope_id in &state.cleanup_scope_stack {
                if let Some(prev_id) = previous_cleanup_stack_id
                    && prev_id >= *scope_id
                {
                    return Err(format!(
                        "{path}: state s{} cleanup stack is not strictly sorted at cleanup{}",
                        state.id,
                        scope_id
                    ));
                }
                previous_cleanup_stack_id = Some(*scope_id);
                if !cleanup_scope_by_id.contains_key(scope_id) {
                    return Err(format!(
                        "{path}: state s{} references missing cleanup{} in cleanup stack",
                        state.id,
                        scope_id
                    ));
                }
            }

            match state.context {
                UnifiedStateContext::Main => {}
                UnifiedStateContext::Cleanup { scope_id, kind } => {
                    let scope = cleanup_scope_by_id.get(&scope_id).ok_or_else(|| {
                        format!(
                            "{path}: state s{} cleanup context references missing cleanup{}",
                            state.id, scope_id
                        )
                    })?;
                    if scope.kind != kind {
                        return Err(format!(
                            "{path}: state s{} cleanup context kind does not match cleanup{}",
                            state.id, scope_id
                        ));
                    }
                }
                UnifiedStateContext::Arm { arm_id } => {
                    if !arm_by_id.contains_key(&arm_id) {
                        return Err(format!(
                            "{path}: state s{} arm context references missing arm{}",
                            state.id, arm_id
                        ));
                    }
                }
            }

            match state.terminator {
                UnifiedStateTerminator::Goto { next_state } => {
                    validate_state_target_exists(path, state.id, "goto", next_state, &state_by_id)?;
                    validate_expected_outgoing_edge(
                        path,
                        state,
                        UnifiedStateEdge {
                            to_state: next_state,
                            kind: UnifiedStateEdgeKind::Goto,
                        },
                    )?;
                }
                UnifiedStateTerminator::Branch {
                    then_state,
                    else_state,
                    merge_state,
                    ..
                } => {
                    validate_state_target_exists(
                        path,
                        state.id,
                        "branch-then",
                        then_state,
                        &state_by_id,
                    )?;
                    validate_state_target_exists(
                        path,
                        state.id,
                        "branch-else",
                        else_state,
                        &state_by_id,
                    )?;
                    validate_state_target_exists(
                        path,
                        state.id,
                        "branch-merge",
                        merge_state,
                        &state_by_id,
                    )?;
                    let expected = vec![
                        UnifiedStateEdge {
                            to_state: then_state,
                            kind: UnifiedStateEdgeKind::BranchThen,
                        },
                        UnifiedStateEdge {
                            to_state: else_state,
                            kind: UnifiedStateEdgeKind::BranchElse,
                        },
                    ];
                    validate_outgoing_edges(path, state, &expected)?;
                }
                UnifiedStateTerminator::Suspend {
                    site_id,
                    resume_state,
                } => {
                    validate_state_target_exists(
                        path,
                        state.id,
                        "suspend-resume",
                        resume_state,
                        &state_by_id,
                    )?;
                    if !suspend_site_by_id.contains_key(&site_id) {
                        return Err(format!(
                            "{path}: state s{} suspend terminator references missing site{}",
                            state.id, site_id
                        ));
                    }
                    validate_expected_outgoing_edge(
                        path,
                        state,
                        UnifiedStateEdge {
                            to_state: resume_state,
                            kind: UnifiedStateEdgeKind::SuspendResume,
                        },
                    )?;
                }
                UnifiedStateTerminator::CleanupEnter {
                    scope_id,
                    next_state,
                } => {
                    if !cleanup_scope_by_id.contains_key(&scope_id) {
                        return Err(format!(
                            "{path}: state s{} cleanup-enter references missing cleanup{}",
                            state.id, scope_id
                        ));
                    }
                    validate_state_target_exists(
                        path,
                        state.id,
                        "cleanup-enter",
                        next_state,
                        &state_by_id,
                    )?;
                    validate_expected_outgoing_edge(
                        path,
                        state,
                        UnifiedStateEdge {
                            to_state: next_state,
                            kind: UnifiedStateEdgeKind::CleanupEnter,
                        },
                    )?;
                }
                UnifiedStateTerminator::ReturnHandle
                | UnifiedStateTerminator::ReturnFromFunction
                | UnifiedStateTerminator::ArmReturnHandle
                | UnifiedStateTerminator::ArmResumeMatchedSite
                | UnifiedStateTerminator::ArmMaterializeContinuation => {
                    if !state.outgoing_edges.is_empty() {
                        return Err(format!(
                            "{path}: terminal state s{} must not have outgoing edges",
                            state.id
                        ));
                    }
                }
            }
        }

        for arm in &self.arms {
            if !state_by_id.contains_key(&arm.entry_state) {
                return Err(format!(
                    "{path}: arm{} entry state s{} is missing from states[]",
                    arm.arm_id, arm.entry_state
                ));
            }
            if !arm.body_states.contains(&arm.entry_state) {
                return Err(format!(
                    "{path}: arm{} body does not include entry state s{}",
                    arm.arm_id, arm.entry_state
                ));
            }

            let mut previous_state_id = None::<UnifiedStateId>;
            let mut body_state_ids = HashSet::<UnifiedStateId>::new();
            for state_id in &arm.body_states {
                if let Some(prev_id) = previous_state_id
                    && prev_id >= *state_id
                {
                    return Err(format!(
                        "{path}: arm{} body_states[] is not strictly sorted at s{}",
                        arm.arm_id, state_id
                    ));
                }
                previous_state_id = Some(*state_id);
                if !body_state_ids.insert(*state_id) {
                    return Err(format!(
                        "{path}: arm{} body_states[] repeats s{}",
                        arm.arm_id, state_id
                    ));
                }

                let state = state_by_id.get(state_id).ok_or_else(|| {
                    format!(
                        "{path}: arm{} body references missing state s{}",
                        arm.arm_id, state_id
                    )
                })?;
                match state.context {
                    UnifiedStateContext::Arm { arm_id } if arm_id == arm.arm_id => {}
                    _ => {
                        return Err(format!(
                            "{path}: arm{} body state s{} has mismatched context",
                            arm.arm_id, state_id
                        ));
                    }
                }
                if state.cleanup_scope_stack != arm.cleanup_scope_stack {
                    return Err(format!(
                        "{path}: arm{} body state s{} has cleanup stack [{}] but arm expects [{}]",
                        arm.arm_id,
                        state_id,
                        render_segment_cleanup_scope_ids(&state.cleanup_scope_stack),
                        render_segment_cleanup_scope_ids(&arm.cleanup_scope_stack)
                    ));
                }
            }

            validate_sorted_symbol_list(
                path,
                &format!("arm{} binder_slots[]", arm.arm_id),
                &arm.binder_slots,
            )?;
            validate_sorted_symbol_list(
                path,
                &format!("arm{} capture_locals[]", arm.arm_id),
                &arm.capture_locals,
            )?;

            for local_id in &arm.binder_slots {
                let slot = frame_slots_by_id.get(local_id).ok_or_else(|| {
                    format!(
                        "{path}: arm{} binder {} is missing from frame slots",
                        arm.arm_id,
                        describe_segment_local(*local_id, &frame_slots_by_id)
                    )
                })?;
                if slot.owner_arm != Some(arm.arm_id) {
                    return Err(format!(
                        "{path}: arm{} binder {} is not owned by arm{}",
                        arm.arm_id,
                        slot.display_name(),
                        arm.arm_id
                    ));
                }
            }

            for local_id in &arm.capture_locals {
                if !frame_slots_by_id.contains_key(local_id) {
                    return Err(format!(
                        "{path}: arm{} capture {} is missing from frame slots",
                        arm.arm_id,
                        describe_segment_local(*local_id, &frame_slots_by_id)
                    ));
                }
            }
        }

        let mut dispatched_arm_ids = HashSet::<ArmPlanId>::new();
        let mut previous_dispatch_op = None::<&str>;
        let mut dispatch_ops = HashSet::<&str>::new();
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

            let mut previous_arm_id = None::<ArmPlanId>;
            let mut arm_ids = HashSet::<ArmPlanId>::new();
            for arm in &entry.arms {
                if let Some(prev_id) = previous_arm_id
                    && prev_id >= arm.arm_id
                {
                    return Err(format!(
                        "{path}: dispatch entry {} arms[] is not strictly sorted at arm{}",
                        entry.op_fqn, arm.arm_id
                    ));
                }
                previous_arm_id = Some(arm.arm_id);
                if !arm_ids.insert(arm.arm_id) {
                    return Err(format!(
                        "{path}: dispatch entry {} repeats arm{}",
                        entry.op_fqn, arm.arm_id
                    ));
                }

                let unified_arm = arm_by_id.get(&arm.arm_id).ok_or_else(|| {
                    format!(
                        "{path}: dispatch entry {} references missing arm{}",
                        entry.op_fqn, arm.arm_id
                    )
                })?;
                if unified_arm.op_fqn != entry.op_fqn {
                    return Err(format!(
                        "{path}: dispatch entry {} points to arm{} for {}",
                        entry.op_fqn, arm.arm_id, unified_arm.op_fqn
                    ));
                }
                if unified_arm.entry_state != arm.entry_state {
                    return Err(format!(
                        "{path}: dispatch entry {} arm{} entry state s{} does not match arm body s{}",
                        entry.op_fqn,
                        arm.arm_id,
                        arm.entry_state,
                        unified_arm.entry_state
                    ));
                }
                dispatched_arm_ids.insert(arm.arm_id);
            }
        }

        if dispatched_arm_ids.len() != arm_by_id.len() {
            let mut missing = arm_by_id
                .keys()
                .copied()
                .filter(|arm_id| !dispatched_arm_ids.contains(arm_id))
                .map(|arm_id| format!("arm{arm_id}"))
                .collect::<Vec<_>>();
            missing.sort();
            return Err(format!(
                "{path}: dispatch metadata is missing [{}]",
                missing.join(", ")
            ));
        }

        for site in &self.suspend_sites {
            if !state_by_id.contains_key(&site.owner_state) {
                return Err(format!(
                    "{path}: site{} owner state s{} is missing from states[]",
                    site.id, site.owner_state
                ));
            }
            if !state_by_id.contains_key(&site.resume_state) {
                return Err(format!(
                    "{path}: site{} resume state s{} is missing from states[]",
                    site.id, site.resume_state
                ));
            }

            let owner_state = state_by_id
                .get(&site.owner_state)
                .expect("validated owner state should exist");
            match owner_state.terminator {
                UnifiedStateTerminator::Suspend {
                    site_id,
                    resume_state,
                } if site_id == site.id && resume_state == site.resume_state => {}
                _ => {
                    return Err(format!(
                        "{path}: site{} owner state s{} terminator does not point back to site",
                        site.id, site.owner_state
                    ));
                }
            }

            validate_sorted_symbol_list(
                path,
                &format!("site{} available_locals[]", site.id),
                &site.available_locals,
            )?;
            validate_sorted_symbol_list(
                path,
                &format!("site{} capture_locals[]", site.id),
                &site.capture_locals,
            )?;

            let available_locals = site.available_locals.iter().copied().collect::<HashSet<_>>();
            for local_id in &site.available_locals {
                if !frame_slots_by_id.contains_key(local_id) {
                    return Err(format!(
                        "{path}: site{} available local {} is missing from frame slots",
                        site.id,
                        describe_segment_local(*local_id, &frame_slots_by_id)
                    ));
                }
            }
            for local_id in &site.capture_locals {
                if !frame_slots_by_id.contains_key(local_id) {
                    return Err(format!(
                        "{path}: site{} capture local {} is missing from frame slots",
                        site.id,
                        describe_segment_local(*local_id, &frame_slots_by_id)
                    ));
                }
                if !available_locals.contains(local_id) {
                    return Err(format!(
                        "{path}: site{} capture {} is not listed in available_locals[]",
                        site.id,
                        describe_segment_local(*local_id, &frame_slots_by_id)
                    ));
                }
            }

            let mut previous_arm_id = None::<ArmPlanId>;
            let mut matching_arms = HashSet::<ArmPlanId>::new();
            for arm_id in &site.matching_arms {
                if let Some(prev_id) = previous_arm_id
                    && prev_id >= *arm_id
                {
                    return Err(format!(
                        "{path}: site{} matching_arms[] is not strictly sorted at arm{}",
                        site.id, arm_id
                    ));
                }
                previous_arm_id = Some(*arm_id);
                if !matching_arms.insert(*arm_id) {
                    return Err(format!(
                        "{path}: site{} matching_arms[] repeats arm{}",
                        site.id, arm_id
                    ));
                }
                if !arm_by_id.contains_key(arm_id) {
                    return Err(format!(
                        "{path}: site{} references missing arm{}",
                        site.id, arm_id
                    ));
                }
            }

            match &site.kind {
                SuspendSiteKind::Perform { op_fqn } => {
                    for arm_id in &site.matching_arms {
                        let arm = arm_by_id.get(arm_id).expect("validated arm should exist");
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
                SuspendSiteKind::RuntimeRaise { .. } => {
                    for arm_id in &site.matching_arms {
                        let arm = arm_by_id.get(arm_id).expect("validated arm should exist");
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
                    if site.resume_path.is_some() {
                        return Err(format!(
                            "{path}: site{} kind={} must not carry resume_path metadata",
                            site.id,
                            describe_suspend_site_kind(&site.kind)
                        ));
                    }
                }
                SuspendSiteKind::CallMaySuspend { .. }
                | SuspendSiteKind::CallStateMachineCallee { .. }
                | SuspendSiteKind::ObjectInitAccess { .. }
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
                            | SuspendSiteKind::NestedHandleBoundary { .. }
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
        }

        for (idx, nested) in self.nested_handles.iter().enumerate() {
            nested.validate_full_machine_contract_with_path(&format!("{path}/nested#{idx}"))?;
        }

        Ok(())
    }

    fn dispatch_entry(&self, op_fqn: &str) -> Option<&UnifiedDispatchEntry> {
        self.get_dispatch_entry(op_fqn)
    }

    fn suspend_site(&self, id: SuspendSiteId) -> Option<&UnifiedSuspendSite> {
        self.get_suspend_site(id)
    }

    fn cleanup_scope(&self, id: CleanupScopeId) -> Option<&UnifiedCleanupScope> {
        self.get_cleanup_scope(id)
    }

    #[cfg(test)]
    fn pretty_dump(&self, types: &TypeStore) -> String {
        let mut out = String::new();
        self.write_pretty_dump(types, 0, &mut out);
        out
    }

    #[cfg(test)]
    fn write_pretty_dump(&self, types: &TypeStore, indent: usize, out: &mut String) {
        let pad = " ".repeat(indent);
        out.push_str(&format!(
            "{pad}handle span={:?} result={} storage={} entry=s{}\n",
            self.handle_span,
            types.display(self.result_ty),
            self.storage.label(),
            self.entry_state
        ));

        out.push_str(&format!("{pad}frame:\n"));
        for (idx, field) in self.frame.fields.iter().enumerate() {
            match field {
                UnifiedFrameField::System(kind) => {
                    out.push_str(&format!("{pad}  field#{idx} system {}\n", kind.label()));
                }
                UnifiedFrameField::Slot { slot_id } => {
                    let slot = self
                        .frame
                        .slots
                        .iter()
                        .find(|slot| slot.slot.id == *slot_id)
                        .expect("frame slot metadata should exist for every slot field");
                    out.push_str(&format!(
                        "{pad}  field#{idx} slot {}:{} owner={} lifted={}\n",
                        slot.slot.display_name(),
                        types.display(slot.slot.ty),
                        slot.source.label(),
                        yes_no(slot.listed_as_lifted_local)
                    ));
                }
            }
        }

        out.push_str(&format!("{pad}dispatch:\n"));
        if self.dispatch_entries.is_empty() {
            out.push_str(&format!("{pad}  []\n"));
        } else {
            for entry in &self.dispatch_entries {
                let arms = entry
                    .arms
                    .iter()
                    .map(UnifiedDispatchArm::label)
                    .collect::<Vec<_>>()
                    .join(", ");
                out.push_str(&format!("{pad}  {} => [{}]\n", entry.op_fqn, arms));
            }
        }

        let frame_slots_owned_by_id = self
            .frame
            .slots
            .iter()
            .map(|slot| (slot.slot.id, slot.slot.clone()))
            .collect::<HashMap<_, _>>();
        let frame_slots_by_id = frame_slots_owned_by_id
            .iter()
            .map(|(id, slot)| (*id, slot))
            .collect::<HashMap<_, _>>();

        out.push_str(&format!("{pad}arms:\n"));
        if self.arms.is_empty() {
            out.push_str(&format!("{pad}  []\n"));
        } else {
            for arm in &self.arms {
                let binders = render_segment_symbol_ids(&arm.binder_slots, &frame_slots_by_id);
                let captures = render_segment_symbol_ids(&arm.capture_locals, &frame_slots_by_id);
                out.push_str(&format!(
                    "{pad}  arm{} op={} entry=s{} body=[{}]\n",
                    arm.arm_id,
                    arm.op_fqn,
                    arm.entry_state,
                    render_segment_ids(&arm.body_states)
                ));
                out.push_str(&format!("{pad}    binders=[{binders}]\n"));
                out.push_str(&format!("{pad}    captures=[{captures}]\n"));
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
                    "{pad}  cleanup{} kind={} entry=s{} exit=s{} note={}\n",
                    scope.id,
                    scope.kind.label(),
                    scope.entry_state,
                    scope.exit_state,
                    scope.note
                ));
            }
        }

        out.push_str(&format!("{pad}suspend-sites:\n"));
        if self.suspend_sites.is_empty() {
            out.push_str(&format!("{pad}  []\n"));
        } else {
            for site in &self.suspend_sites {
                let matching = site
                    .matching_arms
                    .iter()
                    .map(|arm_id| format!("arm{arm_id}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                let available = render_segment_symbol_ids(&site.available_locals, &frame_slots_by_id);
                let captures = render_segment_symbol_ids(&site.capture_locals, &frame_slots_by_id);
                out.push_str(&format!(
                    "{pad}  site{} kind={} owner=s{} resume=s{} arms=[{}]\n",
                    site.id,
                    site.kind.label(),
                    site.owner_state,
                    site.resume_state,
                    matching
                ));
                out.push_str(&format!("{pad}    available=[{available}]\n"));
                out.push_str(&format!("{pad}    captures=[{captures}]\n"));
                if let Some(detail) = site.kind.detail() {
                    out.push_str(&format!("{pad}    detail={detail}\n"));
                }
                if let Some(source_path) = &site.source_path {
                    out.push_str(&format!("{pad}    path={}\n", source_path.label()));
                }
                if let Some(resume_path) = &site.resume_path {
                    out.push_str(&format!(
                        "{pad}    resume-path={}\n",
                        resume_path.label()
                    ));
                }
            }
        }

        out.push_str(&format!("{pad}states:\n"));
        for state in &self.states {
            out.push_str(&format!("{pad}  s{} {}:\n", state.id, state.label));
            out.push_str(&format!("{pad}    context={}\n", state.context.label()));
            out.push_str(&format!(
                "{pad}    cleanup-stack=[{}]\n",
                render_segment_cleanup_scope_ids(&state.cleanup_scope_stack)
            ));
            for op in &state.ops {
                out.push_str(&format!(
                    "{pad}    op={}\n",
                    op.label(&frame_slots_owned_by_id, types)
                ));
            }
            for edge in &state.outgoing_edges {
                out.push_str(&format!(
                    "{pad}    edge={} -> s{}\n",
                    edge.kind.label(),
                    edge.to_state
                ));
            }
            out.push_str(&format!(
                "{pad}    terminator={}\n",
                state.terminator.label()
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

impl UnifiedFrameSchema {
    // frame schema 只由 segment contract 中已经显式给出的 frame slots / lifted locals
    // 决定；这里只做固定 ABI 头字段拼接与稳定顺序投影，不补推断信息。
    fn from_segments(frame_slots: &[FrameSlot], lifted_locals: &[hir::SymbolId]) -> Self {
        let lifted_local_ids = lifted_locals.iter().copied().collect::<HashSet<_>>();
        let mut fields = UnifiedFrameSystemField::ordered()
            .into_iter()
            .map(UnifiedFrameField::System)
            .collect::<Vec<_>>();
        let mut slots = Vec::with_capacity(frame_slots.len());

        for slot in frame_slots {
            let field_index = fields.len();
            fields.push(UnifiedFrameField::Slot { slot_id: slot.id });
            slots.push(UnifiedFrameSlot {
                slot: slot.clone(),
                source: match slot.owner_arm {
                    Some(arm_id) => UnifiedFrameSlotSource::ArmBinder { arm_id },
                    None => UnifiedFrameSlotSource::HandleBody,
                },
                field_index,
                listed_as_lifted_local: lifted_local_ids.contains(&slot.id),
            });
        }

        Self { fields, slots }
    }

    fn validate_contract<'a>(
        &'a self,
        path: &str,
        arm_by_id: &HashMap<ArmPlanId, &UnifiedArm>,
    ) -> Result<HashMap<hir::SymbolId, &'a FrameSlot>, String> {
        let expected_system_fields = UnifiedFrameSystemField::ordered();
        if self.fields.len() < expected_system_fields.len() {
            return Err(format!(
                "{path}: frame.fields[] has {} entries but at least {} are required for system fields",
                self.fields.len(),
                expected_system_fields.len()
            ));
        }
        for (idx, expected) in expected_system_fields.iter().enumerate() {
            match self.fields.get(idx) {
                Some(UnifiedFrameField::System(actual)) if actual == expected => {}
                Some(UnifiedFrameField::System(actual)) => {
                    return Err(format!(
                        "{path}: frame field#{idx} expected system {} but found {}",
                        expected.label(),
                        actual.label()
                    ));
                }
                Some(UnifiedFrameField::Slot { slot_id }) => {
                    return Err(format!(
                        "{path}: frame field#{idx} expected system {} but found slot {}",
                        expected.label(),
                        slot_id.as_u32()
                    ));
                }
                None => unreachable!("checked frame.fields length above"),
            }
        }

        if self.fields.len() != expected_system_fields.len() + self.slots.len() {
            return Err(format!(
                "{path}: frame.fields[] length {} does not match {} system fields + {} slot fields",
                self.fields.len(),
                expected_system_fields.len(),
                self.slots.len()
            ));
        }

        let mut slots_by_id = HashMap::<hir::SymbolId, &FrameSlot>::new();
        let mut previous_slot_id = None::<hir::SymbolId>;
        for (slot_idx, slot) in self.slots.iter().enumerate() {
            if let Some(prev_id) = previous_slot_id
                && prev_id.as_u32() >= slot.slot.id.as_u32()
            {
                return Err(format!(
                    "{path}: frame.slots[] is not strictly sorted by symbol id at {}",
                    slot.slot.display_name()
                ));
            }
            previous_slot_id = Some(slot.slot.id);

            if slots_by_id.insert(slot.slot.id, &slot.slot).is_some() {
                return Err(format!(
                    "{path}: frame.slots[] repeats {}",
                    slot.slot.display_name()
                ));
            }

            let expected_field_index = expected_system_fields.len() + slot_idx;
            if slot.field_index != expected_field_index {
                return Err(format!(
                    "{path}: frame slot {} expected field_index {} but found {}",
                    slot.slot.display_name(),
                    expected_field_index,
                    slot.field_index
                ));
            }

            match self.fields.get(slot.field_index) {
                Some(UnifiedFrameField::Slot { slot_id }) if *slot_id == slot.slot.id => {}
                Some(UnifiedFrameField::Slot { slot_id }) => {
                    return Err(format!(
                        "{path}: frame slot {} points to field slot {}",
                        slot.slot.display_name(),
                        slot_id.as_u32()
                    ));
                }
                Some(UnifiedFrameField::System(kind)) => {
                    return Err(format!(
                        "{path}: frame slot {} points to system field {}",
                        slot.slot.display_name(),
                        kind.label()
                    ));
                }
                None => {
                    return Err(format!(
                        "{path}: frame slot {} points past frame.fields[]",
                        slot.slot.display_name()
                    ));
                }
            }

            match (slot.slot.owner_arm, slot.source) {
                (None, UnifiedFrameSlotSource::HandleBody) => {}
                (Some(expected_arm_id), UnifiedFrameSlotSource::ArmBinder { arm_id })
                    if expected_arm_id == arm_id => {}
                (Some(expected_arm_id), UnifiedFrameSlotSource::ArmBinder { arm_id }) => {
                    return Err(format!(
                        "{path}: frame slot {} is owned by arm{} but source says arm{}",
                        slot.slot.display_name(),
                        expected_arm_id,
                        arm_id
                    ));
                }
                (None, UnifiedFrameSlotSource::ArmBinder { arm_id }) => {
                    return Err(format!(
                        "{path}: frame slot {} has handle-body ownership but source says arm{}",
                        slot.slot.display_name(),
                        arm_id
                    ));
                }
                (Some(expected_arm_id), UnifiedFrameSlotSource::HandleBody) => {
                    return Err(format!(
                        "{path}: frame slot {} is owned by arm{} but source says handle-body",
                        slot.slot.display_name(),
                        expected_arm_id
                    ));
                }
            }

            if let Some(owner_arm) = slot.slot.owner_arm
                && !arm_by_id.contains_key(&owner_arm)
            {
                return Err(format!(
                    "{path}: frame slot {} references missing arm{}",
                    slot.slot.display_name(),
                    owner_arm
                ));
            }

            if slot.slot.owner_arm.is_some() && slot.listed_as_lifted_local {
                return Err(format!(
                    "{path}: frame slot {} is an arm binder and must not be marked as lifted local",
                    slot.slot.display_name()
                ));
            }
        }

        Ok(slots_by_id)
    }

    fn slot_field_index(&self, slot_id: hir::SymbolId) -> Option<usize> {
        self.get_slot_field_index(slot_id)
    }
}

impl UnifiedDispatchEntry {
    fn from_segment(entry: &HandleSegmentDispatchEntry) -> Self {
        let mut arms = entry
            .targets
            .iter()
            .map(|target| UnifiedDispatchArm {
                arm_id: target.arm_id,
                entry_state: target.entry_segment,
            })
            .collect::<Vec<_>>();
        arms.sort_by_key(|arm| arm.arm_id);
        Self {
            op_fqn: entry.op_fqn.clone(),
            arms,
        }
    }
}

impl UnifiedArm {
    fn from_segment(arm: &HandleSegmentArmBody) -> Self {
        Self {
            arm_id: arm.arm_id,
            op_fqn: arm.op_fqn.clone(),
            entry_state: arm.body_entry_segment,
            body_states: sorted_segment_ids(&arm.body_segments),
            binder_slots: sorted_symbol_ids(&arm.binder_slots),
            capture_locals: sorted_symbol_ids(&arm.capture_locals),
            cleanup_scope_stack: arm.cleanup_scope_stack.clone(),
        }
    }
}

impl UnifiedState {
    fn from_segment(segment: &HandleSegment, mut outgoing_edges: Vec<UnifiedStateEdge>) -> Self {
        outgoing_edges.sort_by_key(|edge| (edge.kind.sort_key(), edge.to_state));
        Self {
            id: segment.id,
            label: segment.label.clone(),
            source_span: segment.source_span,
            context: UnifiedStateContext::from_segment(segment.dispatch_context),
            cleanup_scope_stack: segment.cleanup_scope_stack.clone(),
            ops: segment.ops.clone(),
            terminator: UnifiedStateTerminator::from_segment(&segment.terminator),
            outgoing_edges,
        }
    }
}

impl UnifiedStateContext {
    fn from_segment(context: HandleSegmentDispatchContext) -> Self {
        match context {
            HandleSegmentDispatchContext::Main => Self::Main,
            HandleSegmentDispatchContext::Cleanup { scope_id, kind } => {
                Self::Cleanup { scope_id, kind }
            }
            HandleSegmentDispatchContext::Arm { arm_id, .. } => Self::Arm { arm_id },
        }
    }

    #[cfg(test)]
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

impl UnifiedStateTerminator {
    fn from_segment(terminator: &HandleSegmentTerminator) -> Self {
        match terminator {
            HandleSegmentTerminator::Goto { next_segment } => Self::Goto {
                next_state: *next_segment,
            },
            HandleSegmentTerminator::Branch {
                condition,
                then_segment,
                else_segment,
                merge_segment,
            } => Self::Branch {
                condition: condition.clone(),
                then_state: *then_segment,
                else_state: *else_segment,
                merge_state: *merge_segment,
            },
            HandleSegmentTerminator::Suspend {
                site_id,
                resume_segment,
            } => Self::Suspend {
                site_id: *site_id,
                resume_state: *resume_segment,
            },
            HandleSegmentTerminator::CleanupEnter {
                scope_id,
                next_segment,
            } => Self::CleanupEnter {
                scope_id: *scope_id,
                next_state: *next_segment,
            },
            HandleSegmentTerminator::ReturnHandle => Self::ReturnHandle,
            HandleSegmentTerminator::ReturnFromFunction => Self::ReturnFromFunction,
            HandleSegmentTerminator::ArmExit { exit } => match exit {
                ArmBodyExit::ReturnHandle => Self::ArmReturnHandle,
                ArmBodyExit::ResumeMatchedSite => Self::ArmResumeMatchedSite,
                ArmBodyExit::MaterializeContinuation => Self::ArmMaterializeContinuation,
            },
        }
    }

    #[cfg(test)]
    fn label(&self) -> String {
        match self {
            Self::Goto { next_state } => format!("goto s{next_state}"),
            Self::Branch {
                condition,
                then_state,
                else_state,
                merge_state,
            } => format!(
                "branch cond={} then=s{then_state} else=s{else_state} merge=s{merge_state}",
                condition.label()
            ),
            Self::Suspend {
                site_id,
                resume_state,
            } => format!("suspend site{site_id} -> s{resume_state}"),
            Self::CleanupEnter {
                scope_id,
                next_state,
            } => format!("cleanup scope{scope_id} -> s{next_state}"),
            Self::ReturnHandle => "return handle".to_string(),
            Self::ReturnFromFunction => "return function".to_string(),
            Self::ArmReturnHandle => "arm-exit return-handle".to_string(),
            Self::ArmResumeMatchedSite => "arm-exit resume-matched-site".to_string(),
            Self::ArmMaterializeContinuation => {
                "arm-exit materialize-continuation".to_string()
            }
        }
    }
}

impl UnifiedStateEdgeKind {
    fn from_segment(kind: HandleSegmentEdgeKind) -> Self {
        match kind {
            HandleSegmentEdgeKind::Goto => Self::Goto,
            HandleSegmentEdgeKind::BranchThen => Self::BranchThen,
            HandleSegmentEdgeKind::BranchElse => Self::BranchElse,
            HandleSegmentEdgeKind::SuspendResume => Self::SuspendResume,
            HandleSegmentEdgeKind::CleanupEnter => Self::CleanupEnter,
        }
    }

    fn sort_key(self) -> u8 {
        match self {
            Self::Goto => 0,
            Self::BranchThen => 1,
            Self::BranchElse => 2,
            Self::SuspendResume => 3,
            Self::CleanupEnter => 4,
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

impl UnifiedSuspendSite {
    fn from_segment(site: &HandleSegmentSuspendSite) -> Self {
        Self {
            id: site.id,
            span: site.span,
            kind: site.kind.clone(),
            owner_state: site.owner_segment,
            resume_state: site.resume_segment,
            matching_arms: sorted_arm_ids(&site.matching_arms),
            available_locals: sorted_symbol_ids(&site.available_locals),
            capture_locals: sorted_symbol_ids(&site.capture_locals),
            source_path: site.source_path.clone(),
            resume_path: site.resume_path.clone(),
        }
    }
}

impl UnifiedCleanupScope {
    fn from_segment(scope: &HandleSegmentCleanupScope) -> Self {
        Self {
            id: scope.id,
            kind: scope.kind,
            entry_state: scope.entry_segment,
            exit_state: scope.exit_segment,
            note: scope.note.clone(),
        }
    }
}

impl UnifiedStateMachineStorage {
    fn label(self) -> &'static str {
        match self {
            Self::Heap => "heap",
        }
    }
}

impl UnifiedFrameSystemField {
    fn ordered() -> [Self; 5] {
        [
            Self::StateTag,
            Self::ResumeWord,
            Self::ResumeGcRef,
            Self::CleanupFlag,
            Self::OneShotFlag,
        ]
    }

    fn label(self) -> &'static str {
        match self {
            Self::StateTag => "state-tag",
            Self::ResumeWord => "resume-word",
            Self::ResumeGcRef => "resume-gc-ref",
            Self::CleanupFlag => "cleanup-flag",
            Self::OneShotFlag => "one-shot-flag",
        }
    }
}

impl UnifiedFrameSlotSource {
    #[cfg(test)]
    fn label(self) -> String {
        match self {
            Self::HandleBody => "handle-body".to_string(),
            Self::ArmBinder { arm_id } => format!("arm{arm_id}"),
        }
    }
}

impl UnifiedDispatchArm {
    #[cfg(test)]
    fn label(&self) -> String {
        format!("arm{}(entry=s{})", self.arm_id, self.entry_state)
    }
}

fn build_unified_outgoing_edges(
    edges: &[HandleSegmentEdge],
) -> HashMap<UnifiedStateId, Vec<UnifiedStateEdge>> {
    let mut edges_by_state = HashMap::<UnifiedStateId, Vec<UnifiedStateEdge>>::new();
    for edge in edges {
        edges_by_state
            .entry(edge.from)
            .or_default()
            .push(UnifiedStateEdge {
                to_state: edge.to,
                kind: UnifiedStateEdgeKind::from_segment(edge.kind),
            });
    }
    edges_by_state
}

fn sorted_arm_ids(ids: &[ArmPlanId]) -> Vec<ArmPlanId> {
    let mut ids = ids.to_vec();
    ids.sort_unstable();
    ids
}

fn sorted_segment_ids(ids: &[HandleSegmentId]) -> Vec<HandleSegmentId> {
    let mut ids = ids.to_vec();
    ids.sort_unstable();
    ids
}

fn sorted_symbol_ids(ids: &[hir::SymbolId]) -> Vec<hir::SymbolId> {
    let mut ids = ids.to_vec();
    ids.sort_by_key(|id| id.as_u32());
    ids
}

fn validate_state_target_exists(
    path: &str,
    from_state: UnifiedStateId,
    edge_kind: &str,
    to_state: UnifiedStateId,
    state_by_id: &HashMap<UnifiedStateId, &UnifiedState>,
) -> Result<(), String> {
    if !state_by_id.contains_key(&to_state) {
        return Err(format!(
            "{path}: state s{from_state} {edge_kind} target s{to_state} is missing from states[]"
        ));
    }
    Ok(())
}

fn validate_expected_outgoing_edge(
    path: &str,
    state: &UnifiedState,
    expected: UnifiedStateEdge,
) -> Result<(), String> {
    validate_outgoing_edges(path, state, &[expected])
}

fn validate_outgoing_edges(
    path: &str,
    state: &UnifiedState,
    expected: &[UnifiedStateEdge],
) -> Result<(), String> {
    if state.outgoing_edges != expected {
        let actual = state
            .outgoing_edges
            .iter()
            .map(render_unified_state_edge)
            .collect::<Vec<_>>()
            .join(", ");
        let expected = expected
            .iter()
            .map(render_unified_state_edge)
            .collect::<Vec<_>>()
            .join(", ");
        return Err(format!(
            "{path}: state s{} outgoing edges [{}] do not match expected [{}]",
            state.id, actual, expected
        ));
    }
    Ok(())
}

fn validate_sorted_symbol_list(
    path: &str,
    label: &str,
    ids: &[hir::SymbolId],
) -> Result<(), String> {
    let mut previous = None::<hir::SymbolId>;
    let mut seen = HashSet::<hir::SymbolId>::new();
    for id in ids {
        if let Some(prev_id) = previous
            && prev_id.as_u32() >= id.as_u32()
        {
            return Err(format!(
                "{path}: {label} is not strictly sorted at local#{}",
                id.as_u32()
            ));
        }
        previous = Some(*id);
        if !seen.insert(*id) {
            return Err(format!(
                "{path}: {label} repeats local#{}",
                id.as_u32()
            ));
        }
    }
    Ok(())
}

fn render_unified_state_edge(edge: &UnifiedStateEdge) -> String {
    format!("{}->s{}", edge.kind.label(), edge.to_state)
}

#[cfg(test)]
mod transform_tests {
    use std::collections::{HashMap, HashSet};

    use crate::ast;
    use crate::hir;
    use crate::parser::parse_file;
    use crate::resolve::Index;
    use crate::session::Session;
    use crate::source::SourceFile;
    use crate::ty::TypeStore;
    use crate::typecheck;

    use super::*;

    #[test]
    fn unified_state_machine_builds_heap_full_machine_from_segments() {
        let lowered = lower_typed_single_source(
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
        Yield.next() -> resume {
            resume(10)
        }
        Log.current(seed: Int) -> seed + 1
    } finally {
        println("cleanup")
    }
    result
}
"#,
        );
        let segment_list = build_segment_list_from_lowered(&lowered);
        let machine = segment_list
            .build_unified_state_machine()
            .expect("valid segment contract should transform");

        assert_eq!(machine.entry_state, segment_list.entry_segment);
        assert!(matches!(machine.storage, UnifiedStateMachineStorage::Heap));
        assert_eq!(machine.states.len(), segment_list.segments.len());
        assert_eq!(machine.dispatch_entries.len(), segment_list.dispatch_entries.len());
        assert_eq!(machine.cleanup_scopes.len(), segment_list.cleanup_scopes.len());

        let field_labels = machine
            .frame
            .fields
            .iter()
            .take(5)
            .map(|field| match field {
                UnifiedFrameField::System(kind) => kind.label(),
                UnifiedFrameField::Slot { .. } => "slot",
            })
            .collect::<Vec<_>>();
        assert_eq!(
            field_labels,
            vec![
                "state-tag",
                "resume-word",
                "resume-gc-ref",
                "cleanup-flag",
                "one-shot-flag"
            ]
        );

        let has_handle_body_slot = machine
            .frame
            .slots
            .iter()
            .any(|slot| matches!(slot.source, UnifiedFrameSlotSource::HandleBody));
        assert!(has_handle_body_slot, "expected handle-body frame slot");

        let has_arm_binder_slot = machine
            .frame
            .slots
            .iter()
            .any(|slot| matches!(slot.source, UnifiedFrameSlotSource::ArmBinder { .. }));
        assert!(has_arm_binder_slot, "expected arm binder frame slot");

        let has_cleanup_state = machine.states.iter().any(|state| {
            matches!(
                state.context,
                UnifiedStateContext::Cleanup {
                    scope_id: 0,
                    kind: CleanupScopeKind::Finally
                }
            )
        });
        assert!(has_cleanup_state, "expected cleanup-context state");

        let has_arm_state = machine.states.iter().any(|state| {
            matches!(
                state.context,
                UnifiedStateContext::Arm { arm_id: 0 }
            )
        });
        assert!(has_arm_state, "expected arm-context state");

        let dump = machine.pretty_dump(&lowered.types);
        assert!(dump.contains("storage=heap"), "{dump}");
        assert!(dump.contains("field#0 system state-tag"), "{dump}");
        assert!(dump.contains("context=cleanup-body cleanup0 kind=finally"), "{dump}");
    }

    #[test]
    fn unified_state_machine_recurses_nested_handles_and_dispatch_tables() {
        let lowered = lower_typed_single_source(
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
            Yield.next() -> resume {
                resume(10)
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
        let segment_list = build_segment_list_from_lowered(&lowered);
        let machine = segment_list
            .build_unified_state_machine()
            .expect("valid nested handle segments should transform");

        assert_eq!(machine.nested_handles.len(), 1);
        assert_eq!(machine.dispatch_entries.len(), 2);
        assert_eq!(
            machine
                .dispatch_entry("a.Ask.current")
                .expect("outer machine should keep Ask dispatch")
                .arms
                .len(),
            1
        );
        assert_eq!(
            machine
                .dispatch_entry("a.Boom.boom")
                .expect("outer machine should keep Boom dispatch")
                .arms
                .len(),
            1
        );

        let nested = &machine.nested_handles[0];
        assert!(matches!(nested.storage, UnifiedStateMachineStorage::Heap));
        assert_eq!(nested.dispatch_entries.len(), 1);
        assert!(
            nested.dispatch_entry("a.Yield.next").is_some(),
            "nested machine should keep Yield dispatch"
        );

        let has_nested_boundary_site = machine.suspend_sites.iter().any(|site| {
            matches!(
                &site.kind,
                SuspendSiteKind::NestedHandleBoundary { .. }
            )
        });
        assert!(has_nested_boundary_site, "expected nested handle boundary site");

        let dump = machine.pretty_dump(&lowered.types);
        assert!(dump.contains("nested-handles:\n  nested#0"), "{dump}");
        assert!(dump.contains("a.Ask.current => [arm0(entry=s"), "{dump}");
        assert!(dump.contains("a.Boom.boom => [arm1(entry=s"), "{dump}");
    }

    #[test]
    fn unified_state_machine_canonicalizes_unordered_segment_metadata() {
        let lowered = lower_typed_single_source(
            r#"
package a

import scoop.core.*

effect Yield {
    fun next(left: Int, right: Int): Int
}

fun demo(seed: Int): Int {
    val outer0: Int = seed + 1
    val outer1: Int = seed + 2
    val result: Int = handle {
        val local0: Int = outer0 + outer1
        val local1: Int = local0 + seed
        val x: Int = Yield.next(local0, local1)
        x + local0 + local1
    } with {
        Yield.next(left: Int, right: Int) -> resume {
            resume(left + right + outer0 + outer1)
        }
    }
    result
}
"#,
        );
        let mut segment_list = build_segment_list_from_lowered(&lowered);

        let arm = segment_list
            .arm_bodies
            .first_mut()
            .expect("expected one arm body");
        arm.body_segments.reverse();
        arm.binder_slots.reverse();
        arm.capture_locals.reverse();

        let site = segment_list
            .suspend_sites
            .iter_mut()
            .find(|site| matches!(&site.kind, SuspendSiteKind::Perform { .. }))
            .expect("expected perform suspend site");
        site.available_locals.reverse();
        site.capture_locals.reverse();

        let machine = segment_list
            .build_unified_state_machine()
            .expect("unordered set-like metadata should canonicalize");
        let arm = machine.arms.first().expect("expected one unified arm");
        let site = machine
            .suspend_sites
            .iter()
            .find(|site| matches!(&site.kind, SuspendSiteKind::Perform { .. }))
            .expect("expected unified perform site");

        assert!(is_sorted_segment_ids(&arm.body_states));
        assert!(is_sorted_symbol_ids(&arm.binder_slots));
        assert!(is_sorted_symbol_ids(&arm.capture_locals));
        assert!(is_sorted_symbol_ids(&site.available_locals));
        assert!(is_sorted_symbol_ids(&site.capture_locals));
    }

    #[test]
    fn unified_state_machine_requires_valid_segment_contract() {
        let lowered = lower_typed_single_source(
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
        Yield.next(arg: Int) -> resume {
            resume(arg + base)
        }
    }
    result
}
"#,
        );
        let mut segment_list = build_segment_list_from_lowered(&lowered);
        let base_id = segment_slot_id_named(&segment_list, "base");
        segment_list.lifted_locals.retain(|id| *id != base_id);

        let err = segment_list
            .build_unified_state_machine()
            .expect_err("invalid contract should be rejected before transform");
        assert!(err.contains("lifted_locals[] is missing"), "{err}");
        assert!(err.contains("base#"), "{err}");
    }

    #[test]
    fn unified_state_machine_validation_rejects_corrupted_frame_slot_flags() {
        let lowered = lower_typed_single_source(
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
        Yield.next(arg: Int) -> resume {
            resume(arg + base)
        }
    }
    result
}
"#,
        );
        let segment_list = build_segment_list_from_lowered(&lowered);
        let mut machine = segment_list
            .build_unified_state_machine()
            .expect("valid machine should build");

        let binder_slot = machine
            .frame
            .slots
            .iter_mut()
            .find(|slot| slot.slot.owner_arm.is_some())
            .expect("expected arm binder slot in unified frame");
        binder_slot.listed_as_lifted_local = true;

        let err = machine
            .validate_full_machine_contract()
            .expect_err("corrupted frame slot flags should fail validation");
        assert!(err.contains("must not be marked as lifted local"), "{err}");
    }

    #[test]
    fn unified_state_machine_validation_rejects_corrupted_outgoing_edges() {
        let lowered = lower_typed_single_source(
            r#"
package a

import scoop.core.*

effect Yield {
    fun next(): Int
}

fun demo(): Int {
    val result: Int = handle {
        val x: Int = Yield.next()
        x + 1
    } with {
        Yield.next() -> resume {
            resume(10)
        }
    }
    result
}
"#,
        );
        let segment_list = build_segment_list_from_lowered(&lowered);
        let mut machine = segment_list
            .build_unified_state_machine()
            .expect("valid machine should build");

        let broken_state = machine
            .states
            .iter_mut()
            .find(|state| !state.outgoing_edges.is_empty())
            .expect("expected state with outgoing edge");
        broken_state.outgoing_edges.clear();

        let err = machine
            .validate_full_machine_contract()
            .expect_err("corrupted outgoing edges should fail validation");
        assert!(err.contains("outgoing edges"), "{err}");
    }

    #[test]
    fn unified_state_machine_preserves_return_from_function_terminator() {
        let lowered = lower_typed_single_source(
            r#"
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
        Yield.next() -> resume {
            resume(2)
        }
    }
    0
}
"#,
        );
        let segment_list = build_segment_list_from_lowered(&lowered);
        assert!(
            segment_list.segments.iter().any(|segment| matches!(
                segment.terminator,
                HandleSegmentTerminator::ReturnFromFunction
            )),
            "segment list should contain return-from-function terminator"
        );

        let machine = segment_list
            .build_unified_state_machine()
            .expect("return-from-function segment should transform");
        assert!(
            machine.states.iter().any(|state| matches!(
                state.terminator,
                UnifiedStateTerminator::ReturnFromFunction
            )),
            "unified machine should preserve return-from-function terminator"
        );
    }

    #[test]
    fn unified_state_machine_preserves_all_arm_exit_variants() {
        let lowered = lower_typed_single_source(
            r#"
package a

import scoop.core.*

effect Yield {
    fun next(): Int
}

effect Log {
    fun current(seed: Int): Int
}

effect Ask {
    fun current(): Int
}

fun demo(): Int {
    val result: Int = handle {
        val x: Int = Yield.next()
        val y: Int = Log.current(x)
        val z: Int = Ask.current()
        x + y + z
    } with {
        Yield.next() -> resume {
            resume(10)
        }
        Log.current(seed: Int) -> seed + 1
        Ask.current(), k -> 7
    }
    result
}
"#,
        );
        let segment_list = build_segment_list_from_lowered(&lowered);
        let machine = segment_list
            .build_unified_state_machine()
            .expect("all arm exit variants should transform");

        assert!(
            machine.states.iter().any(|state| matches!(
                state.terminator,
                UnifiedStateTerminator::ArmReturnHandle
            )),
            "expected arm-return-handle terminator in unified machine"
        );
        assert!(
            machine.states.iter().any(|state| matches!(
                state.terminator,
                UnifiedStateTerminator::ArmResumeMatchedSite
            )),
            "expected arm-resume-matched-site terminator in unified machine"
        );
        assert!(
            machine.states.iter().any(|state| matches!(
                state.terminator,
                UnifiedStateTerminator::ArmMaterializeContinuation
            )),
            "expected arm-materialize-continuation terminator in unified machine"
        );
    }

    #[test]
    fn unified_state_machine_transforms_all_segment_kinds_from_feature_matrix() {
        let cases: &[(&str, &str, &[&str])] = &[
            (
                "unused_arm_without_suspend_site",
                r#"
package a

import scoop.core.*

effect Yield {
    fun next(): Int
}

fun demo(): Int {
    val result: Int = handle {
        1 + 2
    } with {
        Yield.next() -> resume {
            resume(10)
        }
    }
    result
}
"#,
                &[
                    "dispatch:\n  a.Yield.next => [arm0(entry=s",
                    "suspend-sites:\n  []",
                    "arms:\n  arm0 op=a.Yield.next entry=s",
                ],
            ),
            (
                "branch_loop_finally_perform",
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
        Yield.next() -> resume {
            resume(41)
        }
    } finally {
        println("cleanup")
    }
    result
}
"#,
                &[
                    "kind=perform",
                    "context=cleanup-body cleanup0 kind=finally",
                    "edge=branch-then",
                    "edge=branch-else",
                    "edge=suspend-resume",
                ],
            ),
            (
                "state_machine_callee_and_call_may_suspend",
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
        Ask.ask(seed) -> resume {
            resume(seed + 10)
        }
    }
    result
}
"#,
                &[
                    "kind=call-state-machine-callee",
                    "detail=a.fetch",
                    "kind=call-may-suspend",
                    "path=top[0]",
                    "path=top[1]",
                ],
            ),
            (
                "nested_while_source_path",
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
        Yield.next() -> resume {
            resume(1)
        }
    }
    result
}
"#,
                &[
                    "kind=perform",
                    "path=top[1] -> while-body[1] -> while-body[0]",
                    "edge=suspend-resume",
                ],
            ),
            (
                "when_arm_source_path",
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
        Yield.next() -> resume {
            resume(1)
        }
    }
    result
}
"#,
                &[
                    "kind=perform",
                    "path=top[0] -> when-arm#0[0]",
                ],
            ),
            (
                "nested_handle_boundary_and_nested_machine",
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
            Yield.next() -> resume {
                resume(10)
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
                &[
                    "kind=nested-handle-boundary",
                    "nested-handles:\n  nested#0",
                    "dispatch:\n  a.Ask.current => [arm0(entry=s",
                    "a.Boom.boom => [arm1(entry=s",
                ],
            ),
            (
                "mixed_arm_and_cleanup_contexts",
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
        Yield.next() -> resume {
            resume(10)
        }
        Log.current(seed: Int) -> seed + 1
    } finally {
        println("cleanup")
    }
    result
}
"#,
                &[
                    "context=arm-body arm0",
                    "context=arm-body arm1",
                    "context=cleanup-body cleanup0 kind=finally",
                    "cleanup-stack=[cleanup0]",
                ],
            ),
            (
                "perform_and_call_may_suspend_inside_loop",
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
        Yield.next() -> resume {
            resume(10)
        }
        Ask.ask(seed: Int), k -> seed + 2
    }
    result
}
"#,
                &[
                    "kind=perform",
                    "kind=call-may-suspend",
                    "path=top[2] -> while-body[0]",
                    "path=top[2] -> while-body[1]",
                ],
            ),
            (
                "suspend_inside_cleanup",
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
                &[
                    "kind=perform",
                    "context=cleanup-body cleanup0 kind=finally",
                    "dispatch:\n  a.Ask.ask => [arm0(entry=s",
                ],
            ),
            (
                "runtime_raise_hidden_site",
                r#"
package a

import scoop.core.*

fun demo(k: Continuation<Int>): Int {
    val result: Int = try {
        k.resume(1)
        11
    } catch (e: RuntimeError) {
        22
    }
    result
}
"#,
                &[
                    "kind=runtime-raise",
                    "detail=Continuation.resume",
                ],
            ),
            (
                "class_ctor_init_hidden_site",
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
                &[
                    "kind=class-ctor-init",
                    "detail=a.Boom",
                ],
            ),
            (
                "object_init_access_hidden_site",
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
                &[
                    "kind=object-init-access",
                    "detail=a.BoomObject.x",
                ],
            ),
            (
                "frame_slot_metadata_with_nested_handles",
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
            Ask.current() -> resume {
                resume(base)
            }
        }
        val x: Int = Yield.next(local)
        x + inner + local
    } with {
        Yield.next(arg: Int) -> resume {
            resume(arg + base)
        }
    }
    result
}
"#,
                &[
                    "slot base#",
                    "slot local#",
                    "slot arg#",
                    "owner=arm0",
                    "lifted=yes",
                    "nested-handles:\n  nested#0",
                ],
            ),
        ];

        for (name, source, markers) in cases {
            let lowered = lower_typed_single_source(source);
            let segment_list = build_segment_list_from_lowered(&lowered);
            segment_list
                .validate_builder_contract()
                .unwrap_or_else(|err| panic!("{name}: segment contract failed: {err}"));

            let machine = segment_list
                .build_unified_state_machine()
                .unwrap_or_else(|err| panic!("{name}: unified transform failed: {err}"));
            machine
                .validate_full_machine_contract()
                .unwrap_or_else(|err| panic!("{name}: unified machine contract failed: {err}"));

            let dump = machine.pretty_dump(&lowered.types);
            for marker in *markers {
                assert!(
                    dump.contains(marker),
                    "{name}: expected marker `{marker}` in unified machine dump\n{dump}"
                );
            }
        }
    }

    #[test]
    fn unified_state_machine_preserves_execution_payload_metadata() {
        let lowered = lower_typed_single_source(
            r#"
package a

import scoop.core.*

effect Yield {
    fun next(): Int
}

fun demo(seed: Int): Int {
    val result: Int = handle {
        var total: Int = 0
        while (seed > 0) {
            val current: Int = if (seed > 1) Yield.next() else 1
            total = total + current
            return total
        }
        total
    } with {
        Yield.next() -> resume {
            resume(10)
        }
    }
    result
}
"#,
        );

        let source_plan = build_source_plan_from_lowered(&lowered);
        let segment_list = source_plan.build_segment_list();
        let machine = segment_list
            .build_unified_state_machine()
            .expect("valid segment contract should transform");

        let plan_bind = source_plan
            .states
            .iter()
            .flat_map(|state| state.actions.iter())
            .find_map(|op| match op {
                HandleStateOp::BindLocal { decl, .. } if decl.name.as_deref() == Some("current") => {
                    Some((decl.span, decl.ty))
                }
                _ => None,
            })
            .expect("expected bind-local payload for `current` in source plan");
        let segment_bind = segment_list
            .segments
            .iter()
            .flat_map(|segment| segment.ops.iter())
            .find_map(|op| match op {
                HandleStateOp::BindLocal { decl, .. } if decl.name.as_deref() == Some("current") => {
                    Some((decl.span, decl.ty))
                }
                _ => None,
            })
            .expect("expected bind-local payload for `current` in segment list");
        let machine_bind = machine
            .states
            .iter()
            .flat_map(|state| state.ops.iter())
            .find_map(|op| match op {
                HandleStateOp::BindLocal { decl, .. } if decl.name.as_deref() == Some("current") => {
                    Some((decl.span, decl.ty))
                }
                _ => None,
            })
            .expect("expected bind-local payload for `current` in unified machine");
        assert_eq!(plan_bind, segment_bind);
        assert_eq!(plan_bind, machine_bind);

        let plan_while_stmt_span = source_plan
            .states
            .iter()
            .flat_map(|state| state.actions.iter())
            .find_map(|op| match op {
                HandleStateOp::WhileCondHeader { stmt } => Some(stmt.span),
                _ => None,
            })
            .expect("expected while-cond header payload in source plan");
        let segment_while_stmt_span = segment_list
            .segments
            .iter()
            .flat_map(|segment| segment.ops.iter())
            .find_map(|op| match op {
                HandleStateOp::WhileCondHeader { stmt } => Some(stmt.span),
                _ => None,
            })
            .expect("expected while-cond header payload in segment list");
        let machine_while_stmt_span = machine
            .states
            .iter()
            .flat_map(|state| state.ops.iter())
            .find_map(|op| match op {
                HandleStateOp::WhileCondHeader { stmt } => Some(stmt.span),
                _ => None,
            })
            .expect("expected while-cond header payload in unified machine");
        assert_eq!(plan_while_stmt_span, segment_while_stmt_span);
        assert_eq!(plan_while_stmt_span, machine_while_stmt_span);

        let plan_if_condition = source_plan
            .states
            .iter()
            .find_map(|state| match &state.terminator {
                StateTerminator::Branch {
                    condition: HandleBranchCondition::IfCond { condition },
                    ..
                } => Some((condition.span, condition.ty)),
                _ => None,
            })
            .expect("expected if-branch condition payload in source plan");
        let segment_if_condition = segment_list
            .segments
            .iter()
            .find_map(|segment| match &segment.terminator {
                HandleSegmentTerminator::Branch {
                    condition: HandleBranchCondition::IfCond { condition },
                    ..
                } => Some((condition.span, condition.ty)),
                _ => None,
            })
            .expect("expected if-branch condition payload in segment list");
        let machine_if_condition = machine
            .states
            .iter()
            .find_map(|state| match &state.terminator {
                UnifiedStateTerminator::Branch {
                    condition: HandleBranchCondition::IfCond { condition },
                    ..
                } => Some((condition.span, condition.ty)),
                _ => None,
            })
            .expect("expected if-branch condition payload in unified machine");
        assert_eq!(plan_if_condition, segment_if_condition);
        assert_eq!(plan_if_condition, machine_if_condition);

        let plan_perform = source_plan
            .states
            .iter()
            .flat_map(|state| state.actions.iter())
            .find_map(|op| match op {
                HandleStateOp::Perform { op_fqn, expr } => {
                    Some((op_fqn.clone(), expr.span, expr.ty))
                }
                _ => None,
            })
            .expect("expected perform payload in source plan");
        let segment_perform = segment_list
            .segments
            .iter()
            .flat_map(|segment| segment.ops.iter())
            .find_map(|op| match op {
                HandleStateOp::Perform { op_fqn, expr } => {
                    Some((op_fqn.clone(), expr.span, expr.ty))
                }
                _ => None,
            })
            .expect("expected perform payload in segment list");
        let machine_perform = machine
            .states
            .iter()
            .flat_map(|state| state.ops.iter())
            .find_map(|op| match op {
                HandleStateOp::Perform { op_fqn, expr } => {
                    Some((op_fqn.clone(), expr.span, expr.ty))
                }
                _ => None,
            })
            .expect("expected perform payload in unified machine");
        assert_eq!(plan_perform, segment_perform);
        assert_eq!(plan_perform, machine_perform);

        let plan_resume = source_plan
            .states
            .iter()
            .flat_map(|state| state.actions.iter())
            .find_map(|op| match op {
                HandleStateOp::ResumeAfterSite {
                    reason: ResumeAfterSiteReason::Perform,
                    source_span,
                    source_ty,
                    ..
                } => Some((*source_span, *source_ty)),
                _ => None,
            })
            .expect("expected resume-after-site payload in source plan");
        let segment_resume = segment_list
            .segments
            .iter()
            .flat_map(|segment| segment.ops.iter())
            .find_map(|op| match op {
                HandleStateOp::ResumeAfterSite {
                    reason: ResumeAfterSiteReason::Perform,
                    source_span,
                    source_ty,
                    ..
                } => Some((*source_span, *source_ty)),
                _ => None,
            })
            .expect("expected resume-after-site payload in segment list");
        let machine_resume = machine
            .states
            .iter()
            .flat_map(|state| state.ops.iter())
            .find_map(|op| match op {
                HandleStateOp::ResumeAfterSite {
                    reason: ResumeAfterSiteReason::Perform,
                    source_span,
                    source_ty,
                    ..
                } => Some((*source_span, *source_ty)),
                _ => None,
            })
            .expect("expected resume-after-site payload in unified machine");
        assert_eq!(plan_perform.1, plan_resume.0);
        assert_eq!(plan_resume, segment_resume);
        assert_eq!(plan_resume, machine_resume);

        let plan_arm = source_plan
            .states
            .iter()
            .flat_map(|state| state.actions.iter())
            .find_map(|op| match op {
                HandleStateOp::ExecuteArmBody { arm, .. } => Some((arm.span, arm.body.span, arm.kind)),
                _ => None,
            })
            .expect("expected execute-arm payload in source plan");
        let segment_arm = segment_list
            .segments
            .iter()
            .flat_map(|segment| segment.ops.iter())
            .find_map(|op| match op {
                HandleStateOp::ExecuteArmBody { arm, .. } => Some((arm.span, arm.body.span, arm.kind)),
                _ => None,
            })
            .expect("expected execute-arm payload in segment list");
        let machine_arm = machine
            .states
            .iter()
            .flat_map(|state| state.ops.iter())
            .find_map(|op| match op {
                HandleStateOp::ExecuteArmBody { arm, .. } => Some((arm.span, arm.body.span, arm.kind)),
                _ => None,
            })
            .expect("expected execute-arm payload in unified machine");
        assert!(matches!(
            plan_arm.2,
            hir::HandleArmKind::ImmediateResume { .. }
        ));
        assert_eq!(plan_arm, segment_arm);
        assert_eq!(plan_arm, machine_arm);
    }

    #[test]
    fn resume_path_is_preserved_from_plan_to_segments_to_unified_machine() {
        let lowered = lower_typed_single_source(
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
        Yield.next() -> resume {
            resume(41)
        }
    }
    result
}
"#,
        );
        let source_plan = build_source_plan_from_lowered(&lowered);
        let segment_list = source_plan.build_segment_list();
        let machine = segment_list
            .build_unified_state_machine()
            .expect("unified machine should build");

        let plan_resume_path = source_plan
            .suspend_sites
            .iter()
            .find_map(|site| site.resume_path.as_ref().map(SuspendResumePath::label))
            .expect("expected resume_path in source plan");
        let segment_resume_path = segment_list
            .suspend_sites
            .iter()
            .find_map(|site| site.resume_path.as_ref().map(SuspendResumePath::label))
            .expect("expected resume_path in segment list");
        let machine_resume_path = machine
            .suspend_sites
            .iter()
            .find_map(|site| site.resume_path.as_ref().map(SuspendResumePath::label))
            .expect("expected resume_path in unified machine");

        assert_eq!(plan_resume_path, "val-init -> call-arg#0 -> binary-lhs");
        assert_eq!(plan_resume_path, segment_resume_path);
        assert_eq!(plan_resume_path, machine_resume_path);
    }

    #[test]
    fn nested_handle_boundary_preserves_resume_path_and_slot() {
        let lowered = lower_typed_single_source(
            r#"
package a

import scoop.core.*

effect Yield {
    fun next(): Int
}

fun demo(): Int {
    val result: Int = handle {
        val y: Int = (handle {
            val inner: Int = Yield.next()
            inner + 1
        } with {
            Yield.next() -> resume {
                resume(41)
            }
        }) + 2
        y
    } with {
        Yield.next() -> 99
    }
    result
}
"#,
        );
        let source_plan = build_source_plan_from_lowered(&lowered);
        let segment_list = source_plan.build_segment_list();
        let machine = segment_list
            .build_unified_state_machine()
            .expect("unified machine should build for nested handle boundary");

        let plan_resume_path = source_plan
            .suspend_sites
            .iter()
            .find(|site| matches!(site.kind, SuspendSiteKind::NestedHandleBoundary { .. }))
            .and_then(|site| site.resume_path.as_ref().map(SuspendResumePath::label))
            .expect("expected nested handle boundary resume_path in source plan");
        let segment_resume_path = segment_list
            .suspend_sites
            .iter()
            .find(|site| matches!(site.kind, SuspendSiteKind::NestedHandleBoundary { .. }))
            .and_then(|site| site.resume_path.as_ref().map(SuspendResumePath::label))
            .expect("expected nested handle boundary resume_path in segment list");
        let machine_resume_path = machine
            .suspend_sites
            .iter()
            .find(|site| matches!(site.kind, SuspendSiteKind::NestedHandleBoundary { .. }))
            .and_then(|site| site.resume_path.as_ref().map(SuspendResumePath::label))
            .expect("expected nested handle boundary resume_path in unified machine");
        assert_eq!(plan_resume_path, "val-init -> binary-lhs");
        assert_eq!(plan_resume_path, segment_resume_path);
        assert_eq!(plan_resume_path, machine_resume_path);

        let plan_resume_has_slot = source_plan
            .states
            .iter()
            .flat_map(|state| state.actions.iter())
            .find_map(|op| match op {
                HandleStateOp::ResumeAfterSite {
                    reason: ResumeAfterSiteReason::NestedHandleBoundary,
                    resume_slot,
                    ..
                } => Some(resume_slot.is_some()),
                _ => None,
            })
            .expect("expected nested handle boundary ResumeAfterSite in source plan");
        let segment_resume_has_slot = segment_list
            .segments
            .iter()
            .flat_map(|segment| segment.ops.iter())
            .find_map(|op| match op {
                HandleStateOp::ResumeAfterSite {
                    reason: ResumeAfterSiteReason::NestedHandleBoundary,
                    resume_slot,
                    ..
                } => Some(resume_slot.is_some()),
                _ => None,
            })
            .expect("expected nested handle boundary ResumeAfterSite in segment list");
        let machine_resume_has_slot = machine
            .states
            .iter()
            .flat_map(|state| state.ops.iter())
            .find_map(|op| match op {
                HandleStateOp::ResumeAfterSite {
                    reason: ResumeAfterSiteReason::NestedHandleBoundary,
                    resume_slot,
                    ..
                } => Some(resume_slot.is_some()),
                _ => None,
            })
            .expect("expected nested handle boundary ResumeAfterSite in unified machine");
        assert!(plan_resume_has_slot);
        assert!(segment_resume_has_slot);
        assert!(machine_resume_has_slot);

        let resumed_binary = source_plan
            .states
            .iter()
            .find_map(|state| {
                let has_nested_resume_marker = state.actions.iter().any(|op| {
                    matches!(
                        op,
                        HandleStateOp::ResumeAfterSite {
                            reason: ResumeAfterSiteReason::NestedHandleBoundary,
                            ..
                        }
                    )
                });
                if !has_nested_resume_marker {
                    return None;
                }
                state.actions.iter().find_map(|op| match op {
                    HandleStateOp::BinaryExpr { expr } => Some(expr.as_ref()),
                    _ => None,
                })
            })
            .expect("expected post-resume binary expr for nested handle boundary");
        let hir::ExprKind::Binary { lhs, .. } = &resumed_binary.kind else {
            panic!("expected post-resume nested handle expression to stay a binary expr");
        };
        let hir::ExprKind::VarRef(hir::ValueRef::Local { name, .. }) = &lhs.kind else {
            panic!("expected nested handle lhs to rewrite to synthetic resume slot");
        };
        assert!(
            name.starts_with("__resume_site"),
            "expected nested handle lhs to read synthetic resume slot, got {name}"
        );

        let segment_resumed_binary = segment_list
            .segments
            .iter()
            .find_map(|segment| {
                let has_nested_resume_marker = segment.ops.iter().any(|op| {
                    matches!(
                        op,
                        HandleStateOp::ResumeAfterSite {
                            reason: ResumeAfterSiteReason::NestedHandleBoundary,
                            ..
                        }
                    )
                });
                if !has_nested_resume_marker {
                    return None;
                }
                segment.ops.iter().find_map(|op| match op {
                    HandleStateOp::BinaryExpr { expr } => Some(expr.as_ref()),
                    _ => None,
                })
            })
            .expect("expected post-resume binary expr in segment list");
        let hir::ExprKind::Binary {
            lhs: segment_lhs, ..
        } = &segment_resumed_binary.kind
        else {
            panic!("expected segment post-resume nested handle expression to stay a binary expr");
        };
        let hir::ExprKind::VarRef(hir::ValueRef::Local {
            name: segment_name, ..
        }) = &segment_lhs.kind
        else {
            panic!("expected segment nested handle lhs to rewrite to synthetic resume slot");
        };
        assert!(
            segment_name.starts_with("__resume_site"),
            "expected segment nested handle lhs to read synthetic resume slot, got {segment_name}"
        );

        let machine_resumed_binary = machine
            .states
            .iter()
            .find_map(|state| {
                let has_nested_resume_marker = state.ops.iter().any(|op| {
                    matches!(
                        op,
                        HandleStateOp::ResumeAfterSite {
                            reason: ResumeAfterSiteReason::NestedHandleBoundary,
                            ..
                        }
                    )
                });
                if !has_nested_resume_marker {
                    return None;
                }
                state.ops.iter().find_map(|op| match op {
                    HandleStateOp::BinaryExpr { expr } => Some(expr.as_ref()),
                    _ => None,
                })
            })
            .expect("expected post-resume binary expr in unified machine");
        let hir::ExprKind::Binary {
            lhs: machine_lhs, ..
        } = &machine_resumed_binary.kind
        else {
            panic!("expected unified post-resume nested handle expression to stay a binary expr");
        };
        let hir::ExprKind::VarRef(hir::ValueRef::Local {
            name: machine_name, ..
        }) = &machine_lhs.kind
        else {
            panic!("expected unified nested handle lhs to rewrite to synthetic resume slot");
        };
        assert!(
            machine_name.starts_with("__resume_site"),
            "expected unified nested handle lhs to read synthetic resume slot, got {machine_name}"
        );
    }

    #[test]
    fn unified_lowering_contract_provides_complete_read_access() {
        let lowered = lower_typed_single_source(
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
        Yield.next() -> resume {
            resume(10)
        }
        Log.current(seed: Int) -> seed + 1
    } finally {
        println("cleanup")
    }
    result
}
"#,
        );
        let (fun, handle) =
            first_handle_in_file(&lowered.file).expect("expected a handle expression");
        let context = collect_plan_context(&lowered, fun);

        // Build via the same pipeline that build_unified_lowering_contract uses.
        let source_plan =
            HandleStateMachinePlan::build_with_context(&lowered.types, handle, &context);
        let segment_list = source_plan.build_segment_list();
        let machine = segment_list
            .build_unified_state_machine()
            .expect("valid segment contract should transform");
        let contract = UnifiedHandleLoweringContract { machine };

        // -- Top-level contract accessors --
        // handle_span / result_ty are set by the plan builder from the outer Expr,
        // so just verify they are non-degenerate and that convenience delegates match.
        assert_ne!(contract.handle_span().start, contract.handle_span().end);
        assert_eq!(contract.handle_span(), contract.machine().handle_span());
        assert_eq!(contract.result_ty(), contract.machine().result_ty());
        assert_eq!(
            contract.entry_state(),
            contract.machine().entry_state()
        );
        assert!(
            matches!(contract.machine().storage(), UnifiedStateMachineStorage::Heap),
            "current phase must produce heap-allocated full machine"
        );

        // -- States --
        let states = contract.states();
        assert!(!states.is_empty());
        for state in states {
            // Verify accessor round-trip matches internal data.
            let looked_up = contract
                .machine()
                .get_state(state.id())
                .expect("get_state must find every state in states()");
            assert_eq!(looked_up.id(), state.id());
            assert_eq!(looked_up.label(), state.label());
            assert_eq!(looked_up.source_span(), state.source_span());
            assert_eq!(looked_up.context(), state.context());
            assert_eq!(looked_up.ops().len(), state.ops().len());
            assert_eq!(looked_up.outgoing_edges().len(), state.outgoing_edges().len());
        }

        // -- Dispatch entries --
        let dispatch = contract.dispatch_entries();
        assert!(!dispatch.is_empty());
        for entry in dispatch {
            assert!(!entry.op_fqn().is_empty());
            assert!(!entry.arms().is_empty());
            for arm in entry.arms() {
                // Each dispatch arm's entry_state must be a valid state.
                assert!(
                    contract.machine().get_state(arm.entry_state()).is_some(),
                    "dispatch arm entry_state s{} must exist in states",
                    arm.entry_state()
                );
            }
            // Lookup by op_fqn should find the same entry.
            let looked_up = contract
                .machine()
                .get_dispatch_entry(entry.op_fqn())
                .expect("get_dispatch_entry must find dispatched op");
            assert_eq!(looked_up.op_fqn(), entry.op_fqn());
        }

        // -- Arms --
        let arms = contract.arms();
        assert!(!arms.is_empty());
        for arm in arms {
            assert!(!arm.op_fqn().is_empty());
            assert!(!arm.body_states().is_empty());
            assert!(
                contract.machine().get_state(arm.entry_state()).is_some(),
                "arm entry_state must exist"
            );
        }

        // -- Suspend sites --
        let sites = contract.suspend_sites();
        assert!(!sites.is_empty());
        for site in sites {
            let looked_up = contract
                .machine()
                .get_suspend_site(site.id())
                .expect("get_suspend_site must find every site");
            assert_eq!(looked_up.id(), site.id());
            assert_eq!(looked_up.owner_state(), site.owner_state());
            assert_eq!(looked_up.resume_state(), site.resume_state());
        }

        // -- Cleanup scopes --
        let scopes = contract.cleanup_scopes();
        assert!(!scopes.is_empty(), "finally block should produce cleanup scope");
        for scope in scopes {
            let looked_up = contract
                .machine()
                .get_cleanup_scope(scope.id())
                .expect("get_cleanup_scope must find every scope");
            assert_eq!(looked_up.id(), scope.id());
            assert_eq!(looked_up.kind(), scope.kind());
            assert_eq!(looked_up.entry_state(), scope.entry_state());
            assert_eq!(looked_up.exit_state(), scope.exit_state());
            assert!(!looked_up.note().is_empty());
        }

        // -- Frame schema --
        let frame = contract.frame();
        assert!(frame.fields().len() >= 5, "must have at least 5 system fields");
        assert!(!frame.slots().is_empty());
        for slot in frame.slots() {
            assert_eq!(slot.field_index(), frame.get_slot_field_index(slot.slot().id()).unwrap());
            assert!(!slot.slot().name().is_empty());
        }

        // -- Edge accessors --
        for state in contract.states() {
            for edge in state.outgoing_edges() {
                assert!(
                    contract.machine().get_state(edge.target_state()).is_some(),
                    "edge target s{} must exist",
                    edge.target_state()
                );
                // Verify kind() round-trips without panic.
                let _ = edge.kind();
            }
        }
    }

    #[test]
    fn nested_handles_allocate_unique_synthetic_resume_slot_ids() {
        let lowered = lower_typed_single_source(
            r#"
package a

import scoop.core.*

effect Yield {
    fun next(): Int
}

fun demo(seed: Int): Int {
    val result: Int = handle {
        val marker: Int = seed + 1
        val _: Int = handle {
            val inner: Int = Yield.next()
            marker + inner
        } with {
            Yield.next() -> resume {
                resume(41)
            }
        }
        val outer: Int = Yield.next()
        outer + marker
    } with {
        Yield.next() -> resume {
            resume(42)
        }
    }
    result
}
"#,
        );
        let (fun, handle) =
            first_handle_in_file(&lowered.file).expect("expected a handle expression");
        let context = collect_plan_context(&lowered, fun);
        let max_source_symbol_id = context
            .known_local_metadata
            .keys()
            .copied()
            .map(hir::SymbolId::as_u32)
            .max()
            .unwrap_or(0);

        let source_plan =
            HandleStateMachinePlan::build_with_context(&lowered.types, handle, &context);
        let machine = source_plan
            .build_segment_list()
            .build_unified_state_machine()
            .expect("valid segment contract should transform");

        let outer_resume_slots = machine
            .frame()
            .slots()
            .iter()
            .filter(|slot| slot.slot().name().starts_with("__resume_site"))
            .map(|slot| slot.slot().id())
            .collect::<Vec<_>>();
        assert!(
            !outer_resume_slots.is_empty(),
            "outer handle should allocate a synthetic resume slot"
        );

        let nested = machine
            .nested_handles()
            .first()
            .expect("expected nested handle machine");
        let nested_resume_slots = nested
            .frame()
            .slots()
            .iter()
            .filter(|slot| slot.slot().name().starts_with("__resume_site"))
            .map(|slot| slot.slot().id())
            .collect::<Vec<_>>();
        assert!(
            !nested_resume_slots.is_empty(),
            "nested handle should allocate a synthetic resume slot"
        );

        let synthetic_ids = outer_resume_slots
            .iter()
            .chain(nested_resume_slots.iter())
            .map(|id| id.as_u32())
            .collect::<Vec<_>>();
        let unique_ids = synthetic_ids.iter().copied().collect::<HashSet<_>>();
        assert_eq!(
            unique_ids.len(),
            synthetic_ids.len(),
            "synthetic resume slots must not reuse SymbolId across nested handles"
        );
        assert!(
            synthetic_ids
                .iter()
                .all(|id| *id > max_source_symbol_id),
            "synthetic resume slot ids must stay above all source local ids"
        );
    }

    #[test]
    fn nested_handle_outer_scope_seeding_marks_only_real_outer_slots() {
        let lowered = lower_typed_single_source(
            r#"
package a

import scoop.core.*

effect Yield {
    fun next(): Int
}

fun demo(seed: Int): Int {
    val result: Int = handle {
        val marker: Int = seed + 1
        val _: Int = handle {
            val inner: Int = Yield.next()
            marker + inner
        } with {
            Yield.next() -> resume {
                resume(41)
            }
        }
        val outer: Int = Yield.next()
        outer + marker
    } with {
        Yield.next() -> resume {
            resume(42)
        }
    }
    result
}
"#,
        );
        let (fun, handle) =
            first_handle_in_file(&lowered.file).expect("expected a handle expression");
        let context = collect_plan_context(&lowered, fun);

        let source_plan =
            HandleStateMachinePlan::build_with_context(&lowered.types, handle, &context);
        let machine = source_plan
            .build_segment_list()
            .build_unified_state_machine()
            .expect("valid segment contract should transform");
        let nested = machine
            .nested_handles()
            .first()
            .expect("expected nested handle machine");

        let seeded_slot_names = nested
            .frame()
            .slots()
            .iter()
            .filter(|slot| slot.slot().seed_from_outer_scope())
            .map(|slot| slot.slot().name().to_string())
            .collect::<Vec<_>>();
        assert!(
            seeded_slot_names.iter().any(|name| name == "marker"),
            "nested handle should seed captured outer local marker"
        );
        assert!(
            !seeded_slot_names
                .iter()
                .any(|name| name.starts_with("__resume_site")),
            "synthetic resume slots must not be treated as outer-scope seed slots"
        );

        let synthetic_resume_slots = nested
            .frame()
            .slots()
            .iter()
            .filter(|slot| slot.slot().name().starts_with("__resume_site"))
            .collect::<Vec<_>>();
        assert!(
            !synthetic_resume_slots.is_empty(),
            "nested handle should still expose a synthetic resume slot"
        );
        assert!(
            synthetic_resume_slots
                .iter()
                .all(|slot| !slot.slot().seed_from_outer_scope()),
            "synthetic resume slots must never be seeded from outer scope"
        );
    }

    #[test]
    fn handle_outer_scope_seeding_includes_arm_and_finally_locals() {
        let lowered = lower_typed_single_source(
            r#"
package a

import scoop.core.*

effect Yield {
    fun next(): Int
}

fun demo(): Int {
    var saved: Int = 0
    val result: Int = handle {
        val _: Int = Yield.next()
        7
    } with {
        Yield.next(), k -> {
            saved = 41
            0
        }
    } finally {
        saved = saved + 1
    }
    result + saved
}
"#,
        );
        let (fun, handle) =
            first_handle_in_file(&lowered.file).expect("expected a handle expression");
        let context = collect_plan_context(&lowered, fun);

        let source_plan =
            HandleStateMachinePlan::build_with_context(&lowered.types, handle, &context);
        let machine = source_plan
            .build_segment_list()
            .build_unified_state_machine()
            .expect("valid segment contract should transform");

        let seeded_slot_names = machine
            .frame()
            .slots()
            .iter()
            .filter(|slot| slot.slot().seed_from_outer_scope())
            .map(|slot| slot.slot().name().to_string())
            .collect::<Vec<_>>();
        assert!(
            seeded_slot_names.iter().any(|name| name == "saved"),
            "outer local used only in arm/finally should still be seeded into the handle frame"
        );
        assert!(
            !seeded_slot_names.iter().any(|name| name == "k"),
            "escape continuation local must not be treated as an outer-scope seed slot"
        );
    }

    #[test]
    fn declared_handle_local_overwrites_placeholder_slot_metadata() {
        let lowered = lower_typed_single_source(
            r#"
package a

import scoop.core.*

effect Yield {
    fun next(): Int
}

fun demo(): Int {
    handle {
        var saved: Int = 0
        val _: Int = Yield.next()
        saved = 1
        saved
    } with {
        Yield.next() -> resume {
            resume(41)
        }
    }
}
"#,
        );
        let (fun, handle) = first_handle_in_file(&lowered.file).expect("expected a handle");
        let context = collect_plan_context(&lowered, fun);
        let first_stmt = handle
            .body
            .stmts
            .first()
            .expect("expected first handle-body statement");
        let hir::StmtKind::Val(decl) = &first_stmt.kind else {
            panic!("expected first statement to be a local declaration");
        };
        let saved_id = decl.id.expect("expected declared local id");
        let mut builder = HandlePlanBuilder::new(&lowered.types, handle, &context);
        builder.frame_slots.insert(
            saved_id,
            FrameSlot {
                id: saved_id,
                name: "saved".to_string(),
                ty: decl.ty,
                mutable: false,
                seed_from_outer_scope: true,
                owner_arm: None,
            },
        );

        let entry = builder.new_state("entry");
        let mut env = ScopeEnv::default();
        let _ = builder.build_stmt(first_stmt, entry, &mut env);

        let saved_slot = builder
            .frame_slots
            .get(&saved_id)
            .expect("declaration should keep a frame slot");
        assert!(saved_slot.mutable(), "declaration must restore mutability");
        assert!(
            !saved_slot.seed_from_outer_scope(),
            "declaration must clear stale outer-scope seeding metadata"
        );
    }

    #[test]
    fn handle_context_extension_recovers_nested_handle_outer_var_mutability() {
        let lowered = lower_typed_single_source(
            r#"
package a

import scoop.core.*

effect Yield {
    fun next(): Unit
}

fun demo(): Int {
    handle {
        var saved: Int = 0
        val _: Int = handle {
            val _: Unit = Yield.next()
            saved = saved + 1
            saved
        } with {
            Yield.next() -> resume {
                resume(())
            }
        }
        saved
    } with {
        Yield.next() -> resume {
            resume(())
        }
    }
}
"#,
        );
        let (fun, handle) = first_handle_in_file(&lowered.file).expect("expected an outer handle");
        let first_stmt = handle
            .body
            .stmts
            .first()
            .expect("expected first handle-body statement");
        let hir::StmtKind::Val(decl) = &first_stmt.kind else {
            panic!("expected first statement to be a local declaration");
        };
        let saved_id = decl.id.expect("expected declared local id");

        let mut context = collect_plan_context(&lowered, fun);
        context.known_local_metadata.remove(&saved_id);
        context.extend_known_local_metadata_from_handle(handle);

        let saved_meta = context
            .known_local_metadata
            .get(&saved_id)
            .expect("handle extension should recover local metadata");
        assert!(saved_meta.mutable, "outer handle var should stay mutable");

        let outer_plan = HandleStateMachinePlan::build_with_context(&lowered.types, handle, &context);
        let nested = outer_plan
            .nested_handles
            .first()
            .expect("expected nested handle plan");
        let saved_slot = nested
            .frame_layout
            .slots
            .get(&saved_id)
            .expect("nested handle should expose captured outer local slot");
        assert!(
            saved_slot.seed_from_outer_scope(),
            "nested handle should still classify saved as an outer-scope slot"
        );
        assert!(
            saved_slot.mutable(),
            "nested handle should preserve outer var mutability"
        );
    }

    fn build_source_plan_from_lowered(lowered: &hir::LoweredHir) -> HandleStateMachinePlan {
        let (fun, handle) = first_handle_in_file(&lowered.file).expect("expected a handle expression");
        let context = collect_plan_context(lowered, fun);
        HandleStateMachinePlan::build_with_context(&lowered.types, handle, &context)
    }

    fn build_segment_list_from_lowered(lowered: &hir::LoweredHir) -> HandleSegmentList {
        build_source_plan_from_lowered(lowered).build_segment_list()
    }

    fn segment_slot_id_named(segment_list: &HandleSegmentList, name: &str) -> hir::SymbolId {
        segment_list
            .frame_slots
            .iter()
            .find(|slot| slot.name == name)
            .map(|slot| slot.id)
            .unwrap_or_else(|| panic!("expected frame slot named {name}"))
    }

    fn is_sorted_segment_ids(ids: &[HandleSegmentId]) -> bool {
        ids.windows(2).all(|window| window[0] <= window[1])
    }

    fn is_sorted_symbol_ids(ids: &[hir::SymbolId]) -> bool {
        ids.windows(2)
            .all(|window| window[0].as_u32() <= window[1].as_u32())
    }

    fn lower_typed_single_source(source_text: &str) -> hir::LoweredHir {
        lower_typed_single_source_with_source(source_text).1
    }

    fn lower_typed_single_source_with_source(source_text: &str) -> (SourceFile, hir::LoweredHir) {
        let session = Session::new().unwrap();
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
            hir::ExprKind::StructLit { fields, .. } => {
                fields.iter().find_map(|field| first_handle_in_expr(&field.value))
            }
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
            | hir::ExprKind::Todo(_) => None,
        }
    }

    fn collect_plan_context(
        lowered: &hir::LoweredHir,
        owner_fun: &hir::FunDecl,
    ) -> HandlePlanContext {
        let fun_index = lowered
            .file
            .items
            .iter()
            .filter_map(|item| match item {
                hir::Item::Fun(fun) => Some((fun.fqn.clone(), fun)),
                _ => None,
            })
            .chain(lowered.member_funs.iter().map(|fun| (fun.fqn.clone(), fun)))
            .collect::<HashMap<_, _>>();

        let ctor_call_targets = lowered
            .ctor_call_sites
            .iter()
            .map(|(span, targets)| {
                let mut stable_targets = targets.clone();
                stable_targets.sort();
                stable_targets.dedup();
                (*span, stable_targets)
            })
            .collect::<HashMap<_, _>>();
        let object_value_fqns: HashSet<String> = lowered.object_inits.keys().cloned().collect();
        let object_property_fqns: HashSet<String> = lowered
            .object_inits
            .iter()
            .flat_map(|(owner_fqn, object_init)| {
                object_init
                    .properties
                    .keys()
                    .map(|name| format!("{owner_fqn}.{name}"))
                    .collect::<Vec<_>>()
            })
            .collect();
        let known_fun_effects = collect_known_fun_call_suspendability(
            &lowered.types,
            &fun_index,
            &ctor_call_targets,
            &lowered.continuation_resume_call_sites,
            &object_value_fqns,
            &object_property_fqns,
        );

        let mut known_local_metadata = HashMap::new();
        collect_known_local_metadata_in_fun(owner_fun, &mut known_local_metadata);
        let known_local_fun_effects = collect_known_local_fun_call_suspendability_in_fun(
            owner_fun,
            &lowered.types,
            &known_fun_effects,
            &ctor_call_targets,
            &lowered.continuation_resume_call_sites,
            &object_value_fqns,
            &object_property_fqns,
        );
        let next_synthetic_symbol_raw = known_local_metadata
            .keys()
            .copied()
            .map(hir::SymbolId::as_u32)
            .max()
            .unwrap_or(0)
            .saturating_add(1);

        HandlePlanContext {
            known_fun_effects,
            known_local_fun_effects,
            known_local_metadata,
            next_synthetic_symbol_raw: std::cell::Cell::new(next_synthetic_symbol_raw),
            ctor_call_targets,
            continuation_resume_call_sites: lowered.continuation_resume_call_sites.clone(),
            object_value_fqns,
            object_property_fqns,
        }
    }
}
