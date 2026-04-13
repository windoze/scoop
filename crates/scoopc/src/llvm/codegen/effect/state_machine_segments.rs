type HandleSegmentId = PlanStateId;

#[derive(Debug, Clone)]
pub(super) struct HandleSegmentList {
    handle_span: Span,
    result_ty: TypeId,
    entry_segment: HandleSegmentId,
    segments: Vec<HandleSegment>,
    edges: Vec<HandleSegmentEdge>,
    suspend_sites: Vec<HandleSegmentSuspendSite>,
    cleanup_scopes: Vec<HandleSegmentCleanupScope>,
    nested_handles: Vec<HandleSegmentList>,
}

#[derive(Debug, Clone)]
struct HandleSegment {
    id: HandleSegmentId,
    label: String,
    source_span: Option<Span>,
    ops: Vec<String>,
    terminator: HandleSegmentTerminator,
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
            .map(|state| HandleSegment::from_plan_state(state, plan.handle_span, &suspend_sites, &resume_targets))
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
                    "{pad}  site{} kind={} span={:?} resume=seg{} arms=[{}]\n",
                    site.id,
                    site.kind.label(),
                    site.span,
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
        for op in &self.ops {
            acc ^= op.len();
        }
        acc ^ self.terminator.structural_signature()
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

impl HandleSegmentSuspendSite {
    fn from_plan(site: &SuspendSitePlan) -> Self {
        Self {
            id: site.id,
            span: site.span,
            kind: site.kind.clone(),
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
