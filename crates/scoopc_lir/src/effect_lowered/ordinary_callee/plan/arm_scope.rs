//!  Handle-arm scopes, frame layout, dispatch plan and ScopeEnv: ArmPlan, CleanupScopePlan, FrameLayoutPlan, FrameSlot, CalleeSuspendPlan, DispatchPlan, ScopeEnv.

#![allow(dead_code)]

use super::*;

#[derive(Debug, Clone)]
pub struct ArmPlan {
    pub id: ArmPlanId,
    pub op_fqn: String,
    pub effect_ty: TypeId,
    pub binder_slots: Vec<FrameSlot>,
    pub capture_locals: Vec<hir::SymbolId>,
    pub body_entry_state: PlanStateId,
    pub body_may_suspend_outward: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArmBodyExit {
    ReturnHandle,
    ResumeMatchedSite,
    MaterializeContinuation,
}

impl ArmBodyExit {
    pub fn label(self) -> &'static str {
        match self {
            ArmBodyExit::ReturnHandle => "return-handle",
            ArmBodyExit::ResumeMatchedSite => "resume-matched-site",
            ArmBodyExit::MaterializeContinuation => "materialize-continuation",
        }
    }

    pub fn structural_signature(self) -> usize {
        match self {
            ArmBodyExit::ReturnHandle => 1,
            ArmBodyExit::ResumeMatchedSite => 2,
            ArmBodyExit::MaterializeContinuation => 3,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CleanupScopePlan {
    pub id: CleanupScopeId,
    pub kind: CleanupScopeKind,
    pub entry_state: PlanStateId,
    pub exit_state: PlanStateId,
    pub note: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CleanupScopeKind {
    Finally,
}

impl CleanupScopeKind {
    pub fn label(self) -> &'static str {
        match self {
            CleanupScopeKind::Finally => "finally",
        }
    }

    pub fn structural_signature(self) -> usize {
        match self {
            CleanupScopeKind::Finally => 1,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FrameLayoutPlan {
    pub slots: HashMap<hir::SymbolId, FrameSlot>,
    pub lifted_locals: Vec<FrameSlot>,
    pub arm_binders: Vec<FrameSlot>,
    pub has_cleanup_flag: bool,
    pub has_one_shot_flag: bool,
}

#[derive(Debug, Clone)]
pub struct FrameSlot {
    pub id: hir::SymbolId,
    pub name: String,
    pub ty: TypeId,
    pub mutable: bool,
    pub seed_from_outer_scope: bool,
    pub owner_arm: Option<ArmPlanId>,
}

impl FrameSlot {
    pub fn id(&self) -> hir::SymbolId {
        self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn ty(&self) -> TypeId {
        self.ty
    }

    pub fn mutable(&self) -> bool {
        self.mutable
    }

    pub fn seed_from_outer_scope(&self) -> bool {
        self.seed_from_outer_scope
    }

    pub fn owner_arm(&self) -> Option<ArmPlanId> {
        self.owner_arm
    }

    pub fn display_name(&self) -> String {
        format!("{}#{}", self.name, self.id.as_u32())
    }

    pub fn structural_signature(&self) -> usize {
        self.id.as_u32() as usize
            ^ self.name.len()
            ^ ((self.ty.as_u32() as usize) << 1)
            ^ ((usize::from(self.mutable)) << 2)
            ^ ((usize::from(self.seed_from_outer_scope)) << 3)
            ^ self.owner_arm.unwrap_or(0) as usize
    }
}

/// 单个 ordinary callee suspend-state 中需要保存的一个局部绑定。
#[derive(Debug, Clone)]
pub struct CalleeSuspendSavedLocal {
    pub id: hir::SymbolId,
    pub name: String,
    pub ty: TypeId,
    pub mutable: bool,
}

/// 一个 ordinary callee 的最小 resumed-body 恢复 site。
#[derive(Debug, Clone)]
pub struct CalleeSuspendResumeSite {
    pub site_id: u32,
    pub span: Span,
    pub saved_locals: Vec<CalleeSuspendSavedLocal>,
    pub resume_slot_id: hir::SymbolId,
    pub resume_slot_name: String,
    pub resume_slot_ty: TypeId,
    pub resume_tail: hir::Block,
}

impl CalleeSuspendResumeSite {
    pub fn site_tag(&self) -> u32 {
        self.site_id
    }
}

/// Shared ordinary callee suspend/resume plan consumed by backend emitters.
#[derive(Debug, Clone)]
pub struct CalleeSuspendPlan {
    pub saved_locals: Vec<CalleeSuspendSavedLocal>,
    pub resume_sites: Vec<CalleeSuspendResumeSite>,
}

impl CalleeSuspendPlan {
    pub fn resume_site_for_span(&self, span: Span) -> Option<&CalleeSuspendResumeSite> {
        self.resume_sites.iter().find(|site| site.span == span)
    }

    pub fn saved_local_index(&self, local_id: hir::SymbolId) -> Option<u32> {
        self.saved_locals
            .iter()
            .position(|local| local.id == local_id)
            .map(|index| index as u32)
    }
}

impl PlanState {
    pub fn structural_signature(&self) -> usize {
        let mut acc = self.id as usize ^ self.label.len();
        for action in &self.actions {
            acc ^= action.structural_signature();
        }
        for id in &self.reads {
            acc ^= id.as_u32() as usize;
        }
        acc ^ self.terminator.structural_signature()
    }
}

impl SuspendSitePlan {
    pub fn structural_signature(&self) -> usize {
        let mut acc = self.id as usize
            ^ self.span.start
            ^ self.span.end
            ^ (self.owner_state as usize)
            ^ self.resume_target as usize
            ^ self.kind.structural_signature()
            ^ (self.continuation_escape.structural_signature() << 3);
        if let Some(escape_resume_target) = self.escape_resume_target {
            acc ^= (escape_resume_target as usize) << 2;
        }
        for arm in &self.matching_arms {
            acc ^= *arm as usize;
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
        acc
    }

    pub fn may_suspend_outward(&self) -> bool {
        match self.kind {
            SuspendSiteKind::Perform { .. } | SuspendSiteKind::RuntimeRaise { .. } => {
                self.matching_arms.is_empty()
            }
            SuspendSiteKind::CallMaySuspend { .. }
            | SuspendSiteKind::CallStateMachineCallee { .. }
            | SuspendSiteKind::ObjectInitAccess { .. }
            | SuspendSiteKind::TopLevelValueInitAccess { .. }
            | SuspendSiteKind::ClassCtorInit { .. }
            | SuspendSiteKind::NestedHandleBoundary { .. } => true,
        }
    }
}

impl ArmPlan {
    pub fn structural_signature(&self) -> usize {
        let mut acc = self.id as usize
            ^ self.op_fqn.len()
            ^ self.effect_ty.as_u32() as usize
            ^ self.body_entry_state as usize
            ^ (usize::from(self.body_may_suspend_outward) << 1);
        for slot in &self.binder_slots {
            acc ^= slot.structural_signature();
        }
        for id in &self.capture_locals {
            acc ^= (id.as_u32() as usize) << 2;
        }
        acc
    }
}

impl CleanupScopePlan {
    pub fn structural_signature(&self) -> usize {
        self.id as usize
            ^ self.kind.structural_signature()
            ^ self.entry_state as usize
            ^ self.exit_state as usize
            ^ self.note.len()
    }
}

impl FrameLayoutPlan {
    pub fn structural_signature(&self) -> usize {
        let mut acc = self.slots.len()
            ^ self.lifted_locals.len()
            ^ self.arm_binders.len()
            ^ usize::from(self.has_cleanup_flag)
            ^ (usize::from(self.has_one_shot_flag) << 1);
        for slot in self.slots.values() {
            acc ^= slot.structural_signature();
        }
        acc
    }
}

impl DispatchPlan {
    pub fn structural_signature(&self) -> usize {
        self.entries.iter().fold(self.entries.len(), |acc, entry| {
            acc ^ entry.structural_signature()
        })
    }
}

impl DispatchEntry {
    pub fn structural_signature(&self) -> usize {
        let mut acc = self.op_fqn.len();
        for arm_id in &self.arm_ids {
            acc ^= *arm_id as usize;
        }
        acc
    }
}

#[derive(Debug, Clone)]
pub struct DispatchPlan {
    pub entries: Vec<DispatchEntry>,
}

#[derive(Debug, Clone)]
pub struct DispatchEntry {
    pub op_fqn: String,
    pub arm_ids: Vec<ArmPlanId>,
}

#[derive(Clone, Default)]
pub struct ScopeEnv {
    pub slots: Vec<FrameSlot>,
}

impl ScopeEnv {
    pub fn with_outer(slots: Vec<FrameSlot>) -> Self {
        Self { slots }
    }

    pub fn push(&mut self, slot: FrameSlot) {
        self.slots.push(slot);
    }

    pub fn available_ids(&self) -> Vec<hir::SymbolId> {
        self.slots.iter().map(|slot| slot.id).collect()
    }
}
