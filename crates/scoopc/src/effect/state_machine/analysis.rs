// Shared effect/state-machine planning and direct-step summary analysis.

use crate::ast;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;

use crate::effect::analysis::{
    ContinuationEscapeFacts, ContinuationEscapeState, EffectAnalysisCtx, KnownLocalMetadata,
    collect_known_local_metadata_in_block, collect_known_local_metadata_in_expr,
    collect_known_local_metadata_in_fun, collect_known_local_metadata_in_handle,
    collect_known_local_metadata_in_handle_arm,
};
use crate::expr_facts::ExprFactResolver;
use crate::hir;
use crate::program_facts::ProgramFacts;
use crate::span::Span;
use crate::ty::{EffectRow, RefTypeKind, TypeId, TypeKind, TypeStore};

type PlanStateId = u32;
type SuspendSiteId = u32;
type ArmPlanId = u32;
type CleanupScopeId = u32;

type HandlePlanContext = EffectAnalysisCtx;

#[derive(Debug, Clone)]
pub(crate) struct HandleStateMachinePlan {
    handle_span: Span,
    result_ty: TypeId,
    entry_state: PlanStateId,
    states: Vec<PlanState>,
    suspend_sites: Vec<SuspendSitePlan>,
    arm_plans: Vec<ArmPlan>,
    cleanup_scopes: Vec<CleanupScopePlan>,
    frame_layout: FrameLayoutPlan,
    dispatch_plan: DispatchPlan,
    nested_handles: Vec<HandleStateMachinePlan>,
}

impl HandleStateMachinePlan {
    fn build_with_context(
        types: &TypeStore,
        handle: &hir::HandleExpr,
        context: &HandlePlanContext,
    ) -> Self {
        HandlePlanBuilder::new(types, handle, context).build()
    }

    pub(super) fn arm_capture_locals(&self, arm_id: ArmPlanId) -> &[hir::SymbolId] {
        self.arm_plans
            .iter()
            .find(|arm| arm.id == arm_id)
            .map(|arm| arm.capture_locals.as_slice())
            .unwrap_or(&[])
    }

    #[cfg(test)]
    pub(super) fn pretty_dump(&self, types: &TypeStore) -> String {
        let mut out = String::new();
        self.write_pretty_dump(types, 0, &mut out);
        out
    }

    fn structural_signature(&self) -> usize {
        let mut acc = self.handle_span.start
            ^ self.handle_span.end
            ^ self.result_ty.as_u32() as usize
            ^ self.entry_state as usize;
        for state in &self.states {
            acc ^= state.structural_signature();
        }
        for site in &self.suspend_sites {
            acc ^= site.structural_signature();
        }
        for arm in &self.arm_plans {
            acc ^= arm.structural_signature();
        }
        for scope in &self.cleanup_scopes {
            acc ^= scope.structural_signature();
        }
        acc ^= self.frame_layout.structural_signature();
        acc ^= self.dispatch_plan.structural_signature();
        for nested in &self.nested_handles {
            acc ^= nested.structural_signature();
        }
        acc
    }

    fn contains_suspend_subtree(&self) -> bool {
        !self.suspend_sites.is_empty()
            || self
                .nested_handles
                .iter()
                .any(Self::contains_suspend_subtree)
    }

    fn materializes_escape_continuation(&self) -> bool {
        self.states.iter().any(|state| {
            matches!(
                state.terminator,
                StateTerminator::ArmExit(ArmBodyExit::MaterializeContinuation)
            )
        }) || self
            .nested_handles
            .iter()
            .any(Self::materializes_escape_continuation)
    }

    /// Return `true` iff this handle may propagate suspension/effect dispatch
    /// to its enclosing state machine rather than resolving everything within
    /// its own dispatch loop.
    ///
    /// Self-contained nested handles such as `try { k.resume(...) } catch`
    /// still contain internal suspend sites, but they do not require the
    /// enclosing `when` / block / outer handle to split around them.
    fn may_suspend_outward(&self) -> bool {
        self.materializes_escape_continuation()
            || self
                .suspend_sites
                .iter()
                .any(SuspendSitePlan::may_suspend_outward)
            || self
                .arm_plans
                .iter()
                .any(|arm| arm.body_may_suspend_outward)
            || self.nested_handles.iter().any(Self::may_suspend_outward)
    }

    #[cfg(test)]
    fn write_pretty_dump(&self, types: &TypeStore, indent: usize, out: &mut String) {
        let pad = " ".repeat(indent);
        out.push_str(&format!(
            "{pad}handle span={:?} result={} entry=s{}\n",
            self.handle_span,
            types.display(self.result_ty),
            self.entry_state
        ));

        out.push_str(&format!("{pad}dispatch:\n"));
        for entry in &self.dispatch_plan.entries {
            let arm_ids = entry
                .arm_ids
                .iter()
                .map(|id| format!("arm{id}"))
                .collect::<Vec<_>>()
                .join(", ");
            out.push_str(&format!("{pad}  {} => [{}]\n", entry.op_fqn, arm_ids));
        }

        out.push_str(&format!("{pad}frame-layout:\n"));
        out.push_str(&format!(
            "{pad}  state_slot=yes resume_payload=yes cleanup_flag={} one_shot_flag={}\n",
            yes_no(self.frame_layout.has_cleanup_flag),
            yes_no(self.frame_layout.has_one_shot_flag)
        ));
        if self.frame_layout.lifted_locals.is_empty() && self.frame_layout.arm_binders.is_empty() {
            out.push_str(&format!("{pad}  slots=[]\n"));
        } else {
            for slot in &self.frame_layout.lifted_locals {
                out.push_str(&format!(
                    "{pad}  lifted {}:{}\n",
                    slot.display_name(),
                    types.display(slot.ty)
                ));
            }
            for slot in &self.frame_layout.arm_binders {
                out.push_str(&format!(
                    "{pad}  binder arm{} {}:{}\n",
                    slot.owner_arm.unwrap_or(0),
                    slot.display_name(),
                    types.display(slot.ty)
                ));
            }
        }

        out.push_str(&format!("{pad}arms:\n"));
        for arm in &self.arm_plans {
            let binders = if arm.binder_slots.is_empty() {
                "[]".to_string()
            } else {
                format!(
                    "[{}]",
                    arm.binder_slots
                        .iter()
                        .map(|slot| format!("{}:{}", slot.display_name(), types.display(slot.ty)))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            };
            out.push_str(&format!(
                "{pad}  arm{} op={} effect={} body_entry=s{}\n",
                arm.id,
                arm.op_fqn,
                types.display(arm.effect_ty),
                arm.body_entry_state,
            ));
            out.push_str(&format!("{pad}    binders={binders}\n"));
            let captures = render_symbol_list(&arm.capture_locals, &self.frame_layout.slots);
            out.push_str(&format!("{pad}    captures={captures}\n"));
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
                let available =
                    render_symbol_list(&site.available_locals, &self.frame_layout.slots);
                let captures = render_symbol_list(&site.capture_locals, &self.frame_layout.slots);
                let matching = site
                    .matching_arms
                    .iter()
                    .map(|id| format!("arm{id}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                out.push_str(&format!(
                    "{pad}  site{} kind={} span={:?} owner=s{} resume=s{} arms=[{}]\n",
                    site.id,
                    site.kind.label(),
                    site.span,
                    site.owner_state,
                    site.resume_target,
                    matching
                ));
                out.push_str(&format!("{pad}    available=[{available}]\n"));
                out.push_str(&format!("{pad}    captures=[{captures}]\n"));
                if let Some(escape_resume_target) = site.escape_resume_target {
                    out.push_str(&format!(
                        "{pad}    escape-resume=s{}\n",
                        escape_resume_target
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
                if site.kind.is_continuation_resume_boundary() {
                    out.push_str(&format!(
                        "{pad}    continuation-escape={}\n",
                        site.continuation_escape.label()
                    ));
                }
            }
        }

        out.push_str(&format!("{pad}states:\n"));
        for state in &self.states {
            out.push_str(&format!("{pad}  s{} {}:\n", state.id, state.label));
            for action in &state.actions {
                out.push_str(&format!(
                    "{pad}    {}\n",
                    action.label(&self.frame_layout.slots, types)
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

#[derive(Debug, Clone)]
struct PlanState {
    id: PlanStateId,
    label: String,
    actions: Vec<HandleStateOp>,
    terminator: StateTerminator,
    reads: Vec<hir::SymbolId>,
}

#[derive(Debug, Clone)]
pub(crate) enum HandleStateOp {
    StmtEmpty {
        stmt: Box<hir::Stmt>,
    },
    BindLocal {
        id: hir::SymbolId,
        decl: Box<hir::ValDecl>,
        init_from_last_value: bool,
    },
    DeclareAnonymousVal {
        decl: Box<hir::ValDecl>,
        init_from_last_value: bool,
    },
    Assign {
        stmt: Box<hir::Stmt>,
    },
    Break {
        stmt: Box<hir::Stmt>,
    },
    Continue {
        stmt: Box<hir::Stmt>,
    },
    Return {
        stmt: Box<hir::Stmt>,
    },
    TodoStmt {
        stmt: Box<hir::Stmt>,
        kind: String,
    },
    WhileCondHeader {
        stmt: Box<hir::Stmt>,
    },
    LoopReentry {
        cond_state: PlanStateId,
    },
    ExprMissing {
        expr: Box<hir::Expr>,
    },
    Literal {
        expr: Box<hir::Expr>,
    },
    ReadLocal {
        id: hir::SymbolId,
        expr: Box<hir::Expr>,
    },
    CleanupEdgeComplete,
    ReturnToEnclosingExpression,
    ObjectInitAccessBoundary {
        site_id: SuspendSiteId,
        expr: Box<hir::Expr>,
    },
    ResumeAfterSite {
        site_id: SuspendSiteId,
        reason: ResumeAfterSiteReason,
        source_span: Span,
        source_ty: TypeId,
        resume_slot: Option<FrameSlot>,
    },
    VarRef {
        expr: Box<hir::Expr>,
    },
    StructLit {
        expr: Box<hir::Expr>,
    },
    TupleLit {
        expr: Box<hir::Expr>,
    },
    InterpolatedString {
        expr: Box<hir::Expr>,
    },
    Expr {
        expr: Box<hir::Expr>,
    },
    RuntimeRaiseBoundary {
        site_id: SuspendSiteId,
        expr: Box<hir::Expr>,
    },
    BinaryExpr {
        expr: Box<hir::Expr>,
    },
    ImplicitElseUnit {
        span: Span,
    },
    WhenExpr {
        expr: Box<hir::Expr>,
    },
    SuspendCall {
        site_id: SuspendSiteId,
        expr: Box<hir::Expr>,
    },
    Call {
        expr: Box<hir::Expr>,
    },
    Perform {
        op_fqn: String,
        expr: Box<hir::Expr>,
    },
    NestedHandleBoundary {
        site_id: SuspendSiteId,
        nested_id: usize,
        expr: Box<hir::Expr>,
    },
    NestedHandle {
        nested_id: usize,
        expr: Box<hir::Expr>,
    },
    Closure {
        expr: Box<hir::Expr>,
    },
    TodoExpr {
        expr: Box<hir::Expr>,
        kind: String,
    },
    ExecuteArmBody {
        arm_id: ArmPlanId,
        op_fqn: String,
        arm: Box<hir::HandleArm>,
        segmented_body: bool,
    },
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum ResumeAfterSiteReason {
    ObjectInitAccess,
    RuntimeRaiseBoundary,
    Call,
    Perform,
    NestedHandleBoundary,
}

#[derive(Debug, Clone)]
pub(crate) enum HandleBranchCondition {
    WhileCond { condition: Box<hir::Expr> },
    IfCond { condition: Box<hir::Expr> },
}

#[derive(Debug, Clone)]
enum StateTerminator {
    Goto(PlanStateId),
    Branch {
        condition: HandleBranchCondition,
        then_state: PlanStateId,
        else_state: PlanStateId,
        merge_state: PlanStateId,
    },
    Suspend {
        site_id: SuspendSiteId,
    },
    CleanupEnter {
        scope_id: CleanupScopeId,
        next_state: PlanStateId,
    },
    ReturnHandle,
    ReturnFromFunction,
    ArmExit(ArmBodyExit),
}

impl StateTerminator {
    #[cfg(test)]
    fn label(&self) -> String {
        match self {
            StateTerminator::Goto(state) => format!("goto s{state}"),
            StateTerminator::Branch {
                condition,
                then_state,
                else_state,
                merge_state,
            } => format!(
                "branch cond={} then=s{then_state} else=s{else_state} merge=s{merge_state}",
                condition.label()
            ),
            StateTerminator::Suspend { site_id } => format!("suspend site{site_id}"),
            StateTerminator::CleanupEnter {
                scope_id,
                next_state,
            } => {
                format!("cleanup scope{scope_id} -> s{next_state}")
            }
            StateTerminator::ReturnHandle => "return handle".to_string(),
            StateTerminator::ReturnFromFunction => "return function".to_string(),
            StateTerminator::ArmExit(exit) => format!("arm-exit {}", exit.label()),
        }
    }

    fn structural_signature(&self) -> usize {
        match self {
            StateTerminator::Goto(state) => *state as usize,
            StateTerminator::Branch {
                condition,
                then_state,
                else_state,
                merge_state,
            } => {
                condition.structural_signature()
                    ^ (*then_state as usize)
                    ^ ((*else_state as usize) << 1)
                    ^ ((*merge_state as usize) << 2)
            }
            StateTerminator::Suspend { site_id } => 0x1000 ^ (*site_id as usize),
            StateTerminator::CleanupEnter {
                scope_id,
                next_state,
            } => 0x2000 ^ (*scope_id as usize) ^ ((*next_state as usize) << 1),
            StateTerminator::ReturnHandle => 0x3000,
            StateTerminator::ReturnFromFunction => 0x4000,
            StateTerminator::ArmExit(exit) => 0x5000 ^ exit.structural_signature(),
        }
    }
}

impl HandleStateOp {
    #[cfg(test)]
    fn label(&self, slots: &HashMap<hir::SymbolId, FrameSlot>, types: &TypeStore) -> String {
        match self {
            HandleStateOp::StmtEmpty { .. } => "stmt empty".to_string(),
            HandleStateOp::BindLocal { id, .. } => {
                let Some(slot) = slots.get(id) else {
                    return format!("bind local unknown#{}:<?>", id.as_u32());
                };
                format!(
                    "bind local {}:{}",
                    slot.display_name(),
                    types.display(slot.ty)
                )
            }
            HandleStateOp::DeclareAnonymousVal { .. } => "declare anonymous val".to_string(),
            HandleStateOp::Assign { .. } => "assign".to_string(),
            HandleStateOp::Break { .. } => "break".to_string(),
            HandleStateOp::Continue { .. } => "continue".to_string(),
            HandleStateOp::Return { .. } => "return".to_string(),
            HandleStateOp::TodoStmt { kind, .. } => format!("todo stmt {kind}"),
            HandleStateOp::WhileCondHeader { stmt } => {
                format!("while cond span={:?}", stmt.span)
            }
            HandleStateOp::LoopReentry { cond_state } => {
                format!("loop re-entry -> s{cond_state}")
            }
            HandleStateOp::ExprMissing { .. } => "expr missing".to_string(),
            HandleStateOp::Literal { .. } => "literal".to_string(),
            HandleStateOp::ReadLocal { id, .. } => {
                let label = slots
                    .get(id)
                    .map(FrameSlot::display_name)
                    .unwrap_or_else(|| format!("unknown#{}", id.as_u32()));
                format!("read local {label}")
            }
            HandleStateOp::CleanupEdgeComplete => "cleanup edge completes here".to_string(),
            HandleStateOp::ReturnToEnclosingExpression => {
                "handle result returns to enclosing expression".to_string()
            }
            HandleStateOp::ObjectInitAccessBoundary { site_id, .. } => {
                format!("object init access site{site_id}")
            }
            HandleStateOp::ResumeAfterSite {
                site_id,
                reason,
                resume_slot,
                ..
            } => {
                let slot = resume_slot
                    .as_ref()
                    .map(|slot| format!(" via {}", slot.display_name()))
                    .unwrap_or_default();
                format!("resume target for site{site_id} {}{slot}", reason.label())
            }
            HandleStateOp::VarRef { .. } => "var-ref".to_string(),
            HandleStateOp::StructLit { .. } => "struct-lit".to_string(),
            HandleStateOp::TupleLit { .. } => "tuple-lit".to_string(),
            HandleStateOp::InterpolatedString { .. } => "interpolated-string".to_string(),
            HandleStateOp::Expr { .. } => "expr".to_string(),
            HandleStateOp::RuntimeRaiseBoundary { site_id, .. } => {
                format!("runtime raise site{site_id}")
            }
            HandleStateOp::BinaryExpr { .. } => "binary-expr".to_string(),
            HandleStateOp::ImplicitElseUnit { .. } => "implicit else unit".to_string(),
            HandleStateOp::WhenExpr { .. } => "when-expr".to_string(),
            HandleStateOp::SuspendCall { site_id, .. } => format!("suspend call site{site_id}"),
            HandleStateOp::Call { .. } => "call".to_string(),
            HandleStateOp::Perform { op_fqn, .. } => format!("perform {op_fqn}"),
            HandleStateOp::NestedHandleBoundary {
                site_id, nested_id, ..
            } => {
                format!("nested handle boundary site{site_id} nested#{nested_id}")
            }
            HandleStateOp::NestedHandle { nested_id, .. } => {
                format!("nested handle nested#{nested_id}")
            }
            HandleStateOp::Closure { .. } => "closure".to_string(),
            HandleStateOp::TodoExpr { kind, .. } => format!("todo expr {kind}"),
            HandleStateOp::ExecuteArmBody {
                op_fqn,
                arm,
                segmented_body,
                ..
            } => {
                let mode = if *segmented_body {
                    "segmented"
                } else {
                    "opaque"
                };
                format!("execute arm body op={op_fqn} span={:?}", arm.body.span)
                    + &format!(" mode={mode}")
            }
        }
    }

    fn structural_signature(&self) -> usize {
        match self {
            HandleStateOp::StmtEmpty { stmt } => 1 ^ stmt_payload_signature(stmt),
            HandleStateOp::BindLocal {
                id,
                decl,
                init_from_last_value,
            } => {
                2 ^ (id.as_u32() as usize)
                    ^ decl_payload_signature(decl)
                    ^ ((usize::from(*init_from_last_value)) << 3)
            }
            HandleStateOp::DeclareAnonymousVal {
                decl,
                init_from_last_value,
            } => 3 ^ decl_payload_signature(decl) ^ ((usize::from(*init_from_last_value)) << 2),
            HandleStateOp::Assign { stmt } => 4 ^ stmt_payload_signature(stmt),
            HandleStateOp::Break { stmt } => 5 ^ stmt_payload_signature(stmt),
            HandleStateOp::Continue { stmt } => 6 ^ stmt_payload_signature(stmt),
            HandleStateOp::Return { stmt } => 7 ^ stmt_payload_signature(stmt),
            HandleStateOp::TodoStmt { stmt, kind } => 8 ^ stmt_payload_signature(stmt) ^ kind.len(),
            HandleStateOp::WhileCondHeader { stmt } => 9 ^ stmt_payload_signature(stmt),
            HandleStateOp::LoopReentry { cond_state } => 10 ^ (*cond_state as usize),
            HandleStateOp::ExprMissing { expr } => 11 ^ expr_payload_signature(expr),
            HandleStateOp::Literal { expr } => 12 ^ expr_payload_signature(expr),
            HandleStateOp::ReadLocal { id, expr } => {
                13 ^ (id.as_u32() as usize) ^ expr_payload_signature(expr)
            }
            HandleStateOp::CleanupEdgeComplete => 14,
            HandleStateOp::ReturnToEnclosingExpression => 15,
            HandleStateOp::ObjectInitAccessBoundary { site_id, expr } => {
                16 ^ (*site_id as usize) ^ expr_payload_signature(expr)
            }
            HandleStateOp::ResumeAfterSite {
                site_id,
                reason,
                source_span,
                source_ty,
                resume_slot,
            } => {
                17 ^ (*site_id as usize)
                    ^ (reason.structural_signature() << 1)
                    ^ source_span.start
                    ^ (source_span.end << 1)
                    ^ ((*source_ty).as_u32() as usize)
                    ^ resume_slot
                        .as_ref()
                        .map(FrameSlot::structural_signature)
                        .unwrap_or(0)
            }
            HandleStateOp::VarRef { expr } => 18 ^ expr_payload_signature(expr),
            HandleStateOp::StructLit { expr } => 19 ^ expr_payload_signature(expr),
            HandleStateOp::TupleLit { expr } => 20 ^ expr_payload_signature(expr),
            HandleStateOp::InterpolatedString { expr } => 21 ^ expr_payload_signature(expr),
            HandleStateOp::Expr { expr } => 22 ^ expr_payload_signature(expr),
            HandleStateOp::RuntimeRaiseBoundary { site_id, expr } => {
                23 ^ (*site_id as usize) ^ expr_payload_signature(expr)
            }
            HandleStateOp::BinaryExpr { expr } => 24 ^ expr_payload_signature(expr),
            HandleStateOp::ImplicitElseUnit { span } => 25 ^ span.start ^ (span.end << 1),
            HandleStateOp::WhenExpr { expr } => 26 ^ expr_payload_signature(expr),
            HandleStateOp::SuspendCall { site_id, expr } => {
                27 ^ (*site_id as usize) ^ expr_payload_signature(expr)
            }
            HandleStateOp::Call { expr } => 28 ^ expr_payload_signature(expr),
            HandleStateOp::Perform { op_fqn, expr } => {
                29 ^ op_fqn.len() ^ expr_payload_signature(expr)
            }
            HandleStateOp::NestedHandleBoundary {
                site_id,
                nested_id,
                expr,
            } => 30 ^ (*site_id as usize) ^ *nested_id ^ expr_payload_signature(expr),
            HandleStateOp::NestedHandle { nested_id, expr } => {
                31 ^ *nested_id ^ expr_payload_signature(expr)
            }
            HandleStateOp::Closure { expr } => 32 ^ expr_payload_signature(expr),
            HandleStateOp::TodoExpr { expr, kind } => {
                33 ^ kind.len() ^ expr_payload_signature(expr)
            }
            HandleStateOp::ExecuteArmBody {
                arm_id,
                op_fqn,
                arm,
                segmented_body,
            } => {
                34 ^ (*arm_id as usize)
                    ^ op_fqn.len()
                    ^ handle_arm_payload_signature(arm)
                    ^ ((usize::from(*segmented_body)) << 2)
            }
        }
    }
}

impl ResumeAfterSiteReason {
    #[cfg(test)]
    fn label(self) -> &'static str {
        match self {
            ResumeAfterSiteReason::ObjectInitAccess => "after object init access",
            ResumeAfterSiteReason::RuntimeRaiseBoundary => "after runtime raise boundary",
            ResumeAfterSiteReason::Call => "after call",
            ResumeAfterSiteReason::Perform => "after perform",
            ResumeAfterSiteReason::NestedHandleBoundary => "after nested handle boundary",
        }
    }

    fn structural_signature(self) -> usize {
        match self {
            ResumeAfterSiteReason::ObjectInitAccess => 1,
            ResumeAfterSiteReason::RuntimeRaiseBoundary => 2,
            ResumeAfterSiteReason::Call => 3,
            ResumeAfterSiteReason::Perform => 4,
            ResumeAfterSiteReason::NestedHandleBoundary => 5,
        }
    }
}

impl HandleBranchCondition {
    fn label(&self) -> String {
        match self {
            HandleBranchCondition::WhileCond { condition } => {
                format!("while-cond@{:?}", condition.span)
            }
            HandleBranchCondition::IfCond { condition } => {
                format!("if-cond@{:?}", condition.span)
            }
        }
    }

    fn structural_signature(&self) -> usize {
        match self {
            HandleBranchCondition::WhileCond { condition } => 1 ^ expr_payload_signature(condition),
            HandleBranchCondition::IfCond { condition } => 2 ^ expr_payload_signature(condition),
        }
    }
}

#[derive(Debug, Clone)]
struct SuspendSitePlan {
    id: SuspendSiteId,
    span: Span,
    kind: SuspendSiteKind,
    owner_state: PlanStateId,
    resume_target: PlanStateId,
    escape_resume_target: Option<PlanStateId>,
    matching_arms: Vec<ArmPlanId>,
    available_locals: Vec<hir::SymbolId>,
    capture_locals: Vec<hir::SymbolId>,
    source_path: Option<SuspendSourcePath>,
    resume_path: Option<SuspendResumePath>,
    continuation_escape: ContinuationEscapeState,
}

/// suspend site 在其所属外层 source root（handle body stmt / arm body / finally stmt）
/// 下的源码路径。
///
/// 该路径只描述源码中的根位置与嵌套控制流层级，供统一 state-machine
/// 构建、重建与验证阶段使用。
#[derive(Debug, Clone)]
enum SuspendSourceRoot {
    HandleBodyStmt { stmt_idx: usize, stmt_span: Span },
    ArmBody { arm_index: usize, body_span: Span },
    FinallyStmt { stmt_idx: usize, stmt_span: Span },
}

impl SuspendSourceRoot {
    #[cfg(test)]
    fn label(&self) -> String {
        match self {
            SuspendSourceRoot::HandleBodyStmt { stmt_idx, .. } => format!("top[{stmt_idx}]"),
            SuspendSourceRoot::ArmBody { arm_index, .. } => format!("arm#{arm_index}"),
            SuspendSourceRoot::FinallyStmt { stmt_idx, .. } => format!("finally[{stmt_idx}]"),
        }
    }

    fn structural_signature(&self) -> usize {
        match self {
            SuspendSourceRoot::HandleBodyStmt {
                stmt_idx,
                stmt_span,
            } => 0x51 ^ stmt_idx ^ stmt_span.start ^ (stmt_span.end << 1),
            SuspendSourceRoot::ArmBody {
                arm_index,
                body_span,
            } => 0xA1 ^ arm_index ^ body_span.start ^ (body_span.end << 1),
            SuspendSourceRoot::FinallyStmt {
                stmt_idx,
                stmt_span,
            } => 0xF1 ^ stmt_idx ^ stmt_span.start ^ (stmt_span.end << 1),
        }
    }

    fn span(&self) -> Span {
        match self {
            SuspendSourceRoot::HandleBodyStmt { stmt_span, .. }
            | SuspendSourceRoot::FinallyStmt { stmt_span, .. } => *stmt_span,
            SuspendSourceRoot::ArmBody { body_span, .. } => *body_span,
        }
    }

    fn handle_body_stmt_idx(&self) -> Option<usize> {
        match self {
            SuspendSourceRoot::HandleBodyStmt { stmt_idx, .. } => Some(*stmt_idx),
            SuspendSourceRoot::ArmBody { .. } | SuspendSourceRoot::FinallyStmt { .. } => None,
        }
    }
}

#[derive(Debug, Clone)]
struct SuspendSourcePath {
    root: SuspendSourceRoot,
    frames: Vec<SuspendSourceFramePath>,
}

impl SuspendSourcePath {
    #[cfg(test)]
    fn label(&self) -> String {
        let mut parts = vec![self.root.label()];
        parts.extend(self.frames.iter().map(SuspendSourceFramePath::label));
        parts.join(" -> ")
    }

    fn structural_signature(&self) -> usize {
        let mut acc = self.root.structural_signature();
        for frame in &self.frames {
            acc ^= frame.structural_signature();
        }
        acc
    }

    fn root_span(&self) -> Span {
        self.root.span()
    }

    fn handle_body_stmt_idx(&self) -> Option<usize> {
        self.root.handle_body_stmt_idx()
    }
}

/// suspend site 在其“消费位置表达式”中的恢复路径。
///
/// 这份路径不描述外层源码语句/控制流位置；那部分仍由 `SuspendSourcePath`
/// 承担。这里仅描述：
/// 1. suspend site 恢复后的值首先要回到哪个 consumer root；
/// 2. 在该 consumer 内部，site 位于哪条表达式子路径上。
///
/// 后续 `T3010b2+` 会基于这份冻结合同构造真正的 post-suspend
/// continuation fragment，而不是在 emitter 中重新扫描 AST。
#[derive(Debug, Clone)]
struct SuspendResumePath {
    consumer: SuspendResumeConsumer,
    expr_frames: Vec<SuspendResumeExprFrame>,
}

impl SuspendResumePath {
    #[cfg(test)]
    fn label(&self) -> String {
        let mut parts = vec![self.consumer.label().to_string()];
        parts.extend(self.expr_frames.iter().map(SuspendResumeExprFrame::label));
        parts.join(" -> ")
    }

    fn structural_signature(&self) -> usize {
        let mut acc = self.consumer.structural_signature();
        for frame in &self.expr_frames {
            acc ^= frame.structural_signature();
        }
        acc
    }
}

#[derive(Debug, Clone, Copy)]
enum SuspendResumeConsumer {
    ValInit,
    ExprStmt,
    AssignLhs,
    AssignRhs,
    ReturnValue,
    WhileCond,
}

impl SuspendResumeConsumer {
    #[cfg(test)]
    fn label(self) -> &'static str {
        match self {
            SuspendResumeConsumer::ValInit => "val-init",
            SuspendResumeConsumer::ExprStmt => "expr-stmt",
            SuspendResumeConsumer::AssignLhs => "assign-lhs",
            SuspendResumeConsumer::AssignRhs => "assign-rhs",
            SuspendResumeConsumer::ReturnValue => "return-value",
            SuspendResumeConsumer::WhileCond => "while-cond",
        }
    }

    fn structural_signature(self) -> usize {
        match self {
            SuspendResumeConsumer::ValInit => 0x11,
            SuspendResumeConsumer::ExprStmt => 0x22,
            SuspendResumeConsumer::AssignLhs => 0x33,
            SuspendResumeConsumer::AssignRhs => 0x44,
            SuspendResumeConsumer::ReturnValue => 0x55,
            SuspendResumeConsumer::WhileCond => 0x66,
        }
    }
}

#[derive(Debug, Clone)]
enum SuspendResumeExprFrame {
    CallCallee {
        call_span: Span,
    },
    CallArg {
        call_span: Span,
        arg_index: usize,
    },
    NamedArgValue {
        call_span: Span,
        arg_index: usize,
        name_span: Span,
    },
    PerformArg {
        perform_span: Span,
        arg_index: usize,
    },
    MemberReceiver {
        access_span: Span,
    },
    BinaryLhs {
        binary_span: Span,
    },
    BinaryRhs {
        binary_span: Span,
    },
    StructField {
        struct_span: Span,
        field_name: String,
    },
    TupleElement {
        tuple_span: Span,
        element_index: usize,
    },
    InterpolatedExpr {
        string_span: Span,
        part_index: usize,
    },
    UnaryOperand {
        expr_span: Span,
    },
    CastOperand {
        expr_span: Span,
    },
    TypeCheckOperand {
        expr_span: Span,
    },
    IfCond {
        if_span: Span,
    },
    IfThenExpr {
        if_span: Span,
    },
    IfElseExpr {
        if_span: Span,
    },
    WhenSubject {
        when_span: Span,
    },
    WhenArmGuard {
        when_span: Span,
        arm_index: usize,
    },
    WhenArmBody {
        when_span: Span,
        arm_index: usize,
    },
}

impl SuspendResumeExprFrame {
    fn expr_span(&self) -> Span {
        match self {
            SuspendResumeExprFrame::CallCallee { call_span }
            | SuspendResumeExprFrame::CallArg { call_span, .. }
            | SuspendResumeExprFrame::NamedArgValue { call_span, .. } => *call_span,
            SuspendResumeExprFrame::PerformArg { perform_span, .. } => *perform_span,
            SuspendResumeExprFrame::MemberReceiver { access_span } => *access_span,
            SuspendResumeExprFrame::BinaryLhs { binary_span }
            | SuspendResumeExprFrame::BinaryRhs { binary_span } => *binary_span,
            SuspendResumeExprFrame::StructField { struct_span, .. } => *struct_span,
            SuspendResumeExprFrame::TupleElement { tuple_span, .. } => *tuple_span,
            SuspendResumeExprFrame::InterpolatedExpr { string_span, .. } => *string_span,
            SuspendResumeExprFrame::UnaryOperand { expr_span }
            | SuspendResumeExprFrame::CastOperand { expr_span }
            | SuspendResumeExprFrame::TypeCheckOperand { expr_span } => *expr_span,
            SuspendResumeExprFrame::IfCond { if_span }
            | SuspendResumeExprFrame::IfThenExpr { if_span }
            | SuspendResumeExprFrame::IfElseExpr { if_span } => *if_span,
            SuspendResumeExprFrame::WhenSubject { when_span }
            | SuspendResumeExprFrame::WhenArmGuard { when_span, .. }
            | SuspendResumeExprFrame::WhenArmBody { when_span, .. } => *when_span,
        }
    }

    #[cfg(test)]
    fn label(&self) -> String {
        match self {
            SuspendResumeExprFrame::CallCallee { .. } => "call-callee".to_string(),
            SuspendResumeExprFrame::CallArg { arg_index, .. } => {
                format!("call-arg#{arg_index}")
            }
            SuspendResumeExprFrame::NamedArgValue { arg_index, .. } => {
                format!("named-arg#{arg_index}")
            }
            SuspendResumeExprFrame::PerformArg { arg_index, .. } => {
                format!("perform-arg#{arg_index}")
            }
            SuspendResumeExprFrame::MemberReceiver { .. } => "member-receiver".to_string(),
            SuspendResumeExprFrame::BinaryLhs { .. } => "binary-lhs".to_string(),
            SuspendResumeExprFrame::BinaryRhs { .. } => "binary-rhs".to_string(),
            SuspendResumeExprFrame::StructField { field_name, .. } => {
                format!("struct-field({field_name})")
            }
            SuspendResumeExprFrame::TupleElement { element_index, .. } => {
                format!("tuple-elem#{element_index}")
            }
            SuspendResumeExprFrame::InterpolatedExpr { part_index, .. } => {
                format!("interp-expr#{part_index}")
            }
            SuspendResumeExprFrame::UnaryOperand { .. } => "unary-operand".to_string(),
            SuspendResumeExprFrame::CastOperand { .. } => "cast-operand".to_string(),
            SuspendResumeExprFrame::TypeCheckOperand { .. } => "typecheck-operand".to_string(),
            SuspendResumeExprFrame::IfCond { .. } => "if-cond".to_string(),
            SuspendResumeExprFrame::IfThenExpr { .. } => "if-then-expr".to_string(),
            SuspendResumeExprFrame::IfElseExpr { .. } => "if-else-expr".to_string(),
            SuspendResumeExprFrame::WhenSubject { .. } => "when-subject".to_string(),
            SuspendResumeExprFrame::WhenArmGuard { arm_index, .. } => {
                format!("when-arm#{arm_index}-guard")
            }
            SuspendResumeExprFrame::WhenArmBody { arm_index, .. } => {
                format!("when-arm#{arm_index}-body")
            }
        }
    }

    fn structural_signature(&self) -> usize {
        match self {
            SuspendResumeExprFrame::CallCallee { call_span } => {
                0x101 ^ call_span.start ^ (call_span.end << 1)
            }
            SuspendResumeExprFrame::CallArg {
                call_span,
                arg_index,
            } => 0x202 ^ call_span.start ^ (call_span.end << 1) ^ arg_index,
            SuspendResumeExprFrame::NamedArgValue {
                call_span,
                arg_index,
                name_span,
            } => {
                0x303
                    ^ call_span.start
                    ^ (call_span.end << 1)
                    ^ arg_index
                    ^ (name_span.start << 2)
                    ^ (name_span.end << 3)
            }
            SuspendResumeExprFrame::PerformArg {
                perform_span,
                arg_index,
            } => 0x404 ^ perform_span.start ^ (perform_span.end << 1) ^ arg_index,
            SuspendResumeExprFrame::MemberReceiver { access_span } => {
                0x505 ^ access_span.start ^ (access_span.end << 1)
            }
            SuspendResumeExprFrame::BinaryLhs { binary_span } => {
                0x606 ^ binary_span.start ^ (binary_span.end << 1)
            }
            SuspendResumeExprFrame::BinaryRhs { binary_span } => {
                0x707 ^ binary_span.start ^ (binary_span.end << 1)
            }
            SuspendResumeExprFrame::StructField {
                struct_span,
                field_name,
            } => 0x808 ^ struct_span.start ^ (struct_span.end << 1) ^ field_name.len(),
            SuspendResumeExprFrame::TupleElement {
                tuple_span,
                element_index,
            } => 0x909 ^ tuple_span.start ^ (tuple_span.end << 1) ^ element_index,
            SuspendResumeExprFrame::InterpolatedExpr {
                string_span,
                part_index,
            } => 0xA0A ^ string_span.start ^ (string_span.end << 1) ^ part_index,
            SuspendResumeExprFrame::UnaryOperand { expr_span } => {
                0xB0B ^ expr_span.start ^ (expr_span.end << 1)
            }
            SuspendResumeExprFrame::CastOperand { expr_span } => {
                0xC0C ^ expr_span.start ^ (expr_span.end << 1)
            }
            SuspendResumeExprFrame::TypeCheckOperand { expr_span } => {
                0xD0D ^ expr_span.start ^ (expr_span.end << 1)
            }
            SuspendResumeExprFrame::IfCond { if_span } => {
                0xE0E ^ if_span.start ^ (if_span.end << 1)
            }
            SuspendResumeExprFrame::IfThenExpr { if_span } => {
                0xF0F ^ if_span.start ^ (if_span.end << 1)
            }
            SuspendResumeExprFrame::IfElseExpr { if_span } => {
                0x1111 ^ if_span.start ^ (if_span.end << 1)
            }
            SuspendResumeExprFrame::WhenSubject { when_span } => {
                0x1212 ^ when_span.start ^ (when_span.end << 1)
            }
            SuspendResumeExprFrame::WhenArmGuard {
                when_span,
                arm_index,
            } => 0x1313 ^ when_span.start ^ (when_span.end << 1) ^ arm_index,
            SuspendResumeExprFrame::WhenArmBody {
                when_span,
                arm_index,
            } => 0x1414 ^ when_span.start ^ (when_span.end << 1) ^ arm_index,
        }
    }
}

#[derive(Debug, Clone)]
enum SuspendSourceFramePath {
    Block {
        block_span: Span,
        stmt_idx: usize,
    },
    WhenArm {
        when_span: Span,
        arm_index: usize,
        arm_span: Span,
        stmt_idx: usize,
    },
    IfThen {
        if_span: Span,
        then_span: Span,
        stmt_idx: usize,
    },
    IfElse {
        if_span: Span,
        else_span: Span,
        stmt_idx: usize,
    },
    WhileBody {
        while_cond_span: Span,
        while_body_span: Span,
        stmt_idx: usize,
    },
}

impl SuspendSourceFramePath {
    #[cfg(test)]
    fn label(&self) -> String {
        match self {
            SuspendSourceFramePath::Block { stmt_idx, .. } => format!("block[{stmt_idx}]"),
            SuspendSourceFramePath::WhenArm {
                arm_index,
                stmt_idx,
                ..
            } => format!("when-arm#{arm_index}[{stmt_idx}]"),
            SuspendSourceFramePath::IfThen { stmt_idx, .. } => format!("if-then[{stmt_idx}]"),
            SuspendSourceFramePath::IfElse { stmt_idx, .. } => format!("if-else[{stmt_idx}]"),
            SuspendSourceFramePath::WhileBody { stmt_idx, .. } => {
                format!("while-body[{stmt_idx}]")
            }
        }
    }

    fn structural_signature(&self) -> usize {
        match self {
            SuspendSourceFramePath::Block {
                block_span,
                stmt_idx,
            } => 0x101 ^ block_span.start ^ (block_span.end << 1) ^ stmt_idx,
            SuspendSourceFramePath::WhenArm {
                when_span,
                arm_index,
                arm_span,
                stmt_idx,
            } => {
                0x151
                    ^ when_span.start
                    ^ (when_span.end << 1)
                    ^ arm_index
                    ^ (arm_span.start << 2)
                    ^ (arm_span.end << 3)
                    ^ stmt_idx
            }
            SuspendSourceFramePath::IfThen {
                if_span,
                then_span,
                stmt_idx,
            } => {
                0x202
                    ^ if_span.start
                    ^ (if_span.end << 1)
                    ^ (then_span.start << 2)
                    ^ (then_span.end << 3)
                    ^ stmt_idx
            }
            SuspendSourceFramePath::IfElse {
                if_span,
                else_span,
                stmt_idx,
            } => {
                0x303
                    ^ if_span.start
                    ^ (if_span.end << 1)
                    ^ (else_span.start << 2)
                    ^ (else_span.end << 3)
                    ^ stmt_idx
            }
            SuspendSourceFramePath::WhileBody {
                while_cond_span,
                while_body_span,
                stmt_idx,
            } => {
                0x404
                    ^ while_cond_span.start
                    ^ (while_cond_span.end << 1)
                    ^ (while_body_span.start << 2)
                    ^ (while_body_span.end << 3)
                    ^ stmt_idx
            }
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) enum SuspendSiteKind {
    Perform { op_fqn: String },
    CallMaySuspend { callee: String },
    CallStateMachineCallee { callee: String },
    RuntimeRaise { reason: String },
    ObjectInitAccess { target: String },
    TopLevelValueInitAccess { target: String },
    ClassCtorInit { class_name: String },
    NestedHandleBoundary { detail: String },
}

impl SuspendSiteKind {
    #[cfg(test)]
    fn label(&self) -> &'static str {
        match self {
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

    #[cfg(test)]
    fn detail(&self) -> Option<String> {
        match self {
            SuspendSiteKind::Perform { op_fqn }
            | SuspendSiteKind::CallMaySuspend { callee: op_fqn }
            | SuspendSiteKind::CallStateMachineCallee { callee: op_fqn }
            | SuspendSiteKind::RuntimeRaise { reason: op_fqn }
            | SuspendSiteKind::ObjectInitAccess { target: op_fqn }
            | SuspendSiteKind::TopLevelValueInitAccess { target: op_fqn }
            | SuspendSiteKind::ClassCtorInit { class_name: op_fqn }
            | SuspendSiteKind::NestedHandleBoundary { detail: op_fqn } => Some(op_fqn.clone()),
        }
    }

    fn is_continuation_resume_boundary(&self) -> bool {
        matches!(
            self,
            SuspendSiteKind::CallMaySuspend { callee }
                | SuspendSiteKind::RuntimeRaise { reason: callee }
                if callee == "Continuation.resume"
        )
    }

    fn structural_signature(&self) -> usize {
        match self {
            SuspendSiteKind::Perform { op_fqn } => 0x11 ^ op_fqn.len(),
            SuspendSiteKind::CallMaySuspend { callee } => 0x22 ^ callee.len(),
            SuspendSiteKind::CallStateMachineCallee { callee } => 0x33 ^ callee.len(),
            SuspendSiteKind::RuntimeRaise { reason } => 0x44 ^ reason.len(),
            SuspendSiteKind::ObjectInitAccess { target } => 0x55 ^ target.len(),
            SuspendSiteKind::TopLevelValueInitAccess { target } => 0x58 ^ target.len(),
            SuspendSiteKind::ClassCtorInit { class_name } => 0x66 ^ class_name.len(),
            SuspendSiteKind::NestedHandleBoundary { detail } => 0x77 ^ detail.len(),
        }
    }

    fn needs_escape_resume_replay(&self) -> bool {
        matches!(
            self,
            SuspendSiteKind::CallMaySuspend { .. }
                | SuspendSiteKind::CallStateMachineCallee { .. }
                | SuspendSiteKind::ObjectInitAccess { .. }
                | SuspendSiteKind::TopLevelValueInitAccess { .. }
                | SuspendSiteKind::ClassCtorInit { .. }
                | SuspendSiteKind::NestedHandleBoundary { .. }
        )
    }
}

#[derive(Debug, Clone)]
struct ArmPlan {
    id: ArmPlanId,
    op_fqn: String,
    effect_ty: TypeId,
    binder_slots: Vec<FrameSlot>,
    capture_locals: Vec<hir::SymbolId>,
    body_entry_state: PlanStateId,
    body_may_suspend_outward: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ArmBodyExit {
    ReturnHandle,
    ResumeMatchedSite,
    MaterializeContinuation,
}

impl ArmBodyExit {
    fn label(self) -> &'static str {
        match self {
            ArmBodyExit::ReturnHandle => "return-handle",
            ArmBodyExit::ResumeMatchedSite => "resume-matched-site",
            ArmBodyExit::MaterializeContinuation => "materialize-continuation",
        }
    }

    fn structural_signature(self) -> usize {
        match self {
            ArmBodyExit::ReturnHandle => 1,
            ArmBodyExit::ResumeMatchedSite => 2,
            ArmBodyExit::MaterializeContinuation => 3,
        }
    }
}

#[derive(Debug, Clone)]
struct CleanupScopePlan {
    id: CleanupScopeId,
    kind: CleanupScopeKind,
    entry_state: PlanStateId,
    exit_state: PlanStateId,
    note: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CleanupScopeKind {
    Finally,
}

impl CleanupScopeKind {
    fn label(self) -> &'static str {
        match self {
            CleanupScopeKind::Finally => "finally",
        }
    }

    fn structural_signature(self) -> usize {
        match self {
            CleanupScopeKind::Finally => 1,
        }
    }
}

#[derive(Debug, Clone)]
struct FrameLayoutPlan {
    slots: HashMap<hir::SymbolId, FrameSlot>,
    lifted_locals: Vec<FrameSlot>,
    arm_binders: Vec<FrameSlot>,
    has_cleanup_flag: bool,
    has_one_shot_flag: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct FrameSlot {
    id: hir::SymbolId,
    name: String,
    ty: TypeId,
    mutable: bool,
    seed_from_outer_scope: bool,
    owner_arm: Option<ArmPlanId>,
}

impl FrameSlot {
    pub(crate) fn id(&self) -> hir::SymbolId {
        self.id
    }

    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    pub(crate) fn ty(&self) -> TypeId {
        self.ty
    }

    pub(crate) fn mutable(&self) -> bool {
        self.mutable
    }

    pub(crate) fn seed_from_outer_scope(&self) -> bool {
        self.seed_from_outer_scope
    }

    pub(crate) fn owner_arm(&self) -> Option<ArmPlanId> {
        self.owner_arm
    }

    fn display_name(&self) -> String {
        format!("{}#{}", self.name, self.id.as_u32())
    }

    fn structural_signature(&self) -> usize {
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
pub(crate) struct CalleeSuspendSavedLocal {
    pub(crate) id: hir::SymbolId,
    pub(crate) name: String,
    pub(crate) ty: TypeId,
    pub(crate) mutable: bool,
}

/// 一个 ordinary callee 的最小 resumed-body 恢复 site。
#[derive(Debug, Clone)]
pub(crate) struct CalleeSuspendResumeSite {
    pub(crate) site_id: u32,
    pub(crate) span: Span,
    pub(crate) saved_locals: Vec<CalleeSuspendSavedLocal>,
    pub(crate) resume_slot_id: hir::SymbolId,
    pub(crate) resume_slot_name: String,
    pub(crate) resume_slot_ty: TypeId,
    pub(crate) resume_tail: hir::Block,
}

impl CalleeSuspendResumeSite {
    pub(crate) fn site_tag(&self) -> u32 {
        self.site_id
    }
}

/// Shared ordinary callee suspend/resume plan consumed by backend emitters.
#[derive(Debug, Clone)]
pub(crate) struct CalleeSuspendPlan {
    pub(crate) saved_locals: Vec<CalleeSuspendSavedLocal>,
    pub(crate) resume_sites: Vec<CalleeSuspendResumeSite>,
}

impl CalleeSuspendPlan {
    pub(crate) fn resume_site_for_span(&self, span: Span) -> Option<&CalleeSuspendResumeSite> {
        self.resume_sites.iter().find(|site| site.span == span)
    }

    pub(crate) fn saved_local_index(&self, local_id: hir::SymbolId) -> Option<u32> {
        self.saved_locals
            .iter()
            .position(|local| local.id == local_id)
            .map(|index| index as u32)
    }
}

impl PlanState {
    fn structural_signature(&self) -> usize {
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
    fn structural_signature(&self) -> usize {
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

    fn may_suspend_outward(&self) -> bool {
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
    fn structural_signature(&self) -> usize {
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
    fn structural_signature(&self) -> usize {
        self.id as usize
            ^ self.kind.structural_signature()
            ^ self.entry_state as usize
            ^ self.exit_state as usize
            ^ self.note.len()
    }
}

impl FrameLayoutPlan {
    fn structural_signature(&self) -> usize {
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
    fn structural_signature(&self) -> usize {
        self.entries.iter().fold(self.entries.len(), |acc, entry| {
            acc ^ entry.structural_signature()
        })
    }
}

impl DispatchEntry {
    fn structural_signature(&self) -> usize {
        let mut acc = self.op_fqn.len();
        for arm_id in &self.arm_ids {
            acc ^= *arm_id as usize;
        }
        acc
    }
}

#[derive(Debug, Clone)]
struct DispatchPlan {
    entries: Vec<DispatchEntry>,
}

#[derive(Debug, Clone)]
struct DispatchEntry {
    op_fqn: String,
    arm_ids: Vec<ArmPlanId>,
}

#[derive(Clone, Default)]
struct ScopeEnv {
    slots: Vec<FrameSlot>,
}

impl ScopeEnv {
    fn with_outer(slots: Vec<FrameSlot>) -> Self {
        Self { slots }
    }

    fn push(&mut self, slot: FrameSlot) {
        self.slots.push(slot);
    }

    fn available_ids(&self) -> Vec<hir::SymbolId> {
        self.slots.iter().map(|slot| slot.id).collect()
    }
}

#[derive(Clone)]
struct LocalBlockReturnContext {
    decl: hir::ValDecl,
    continuation_state: PlanStateId,
}

struct HandlePlanBuilder<'a, 'hir> {
    types: &'a TypeStore,
    handle: &'hir hir::HandleExpr,
    context: &'a HandlePlanContext,
    known_local_fun_effects: HashMap<hir::SymbolId, bool>,
    next_state_id: PlanStateId,
    next_site_id: SuspendSiteId,
    next_cleanup_id: CleanupScopeId,
    states: Vec<PlanState>,
    suspend_sites: Vec<SuspendSitePlan>,
    arm_plans: Vec<ArmPlan>,
    cleanup_scopes: Vec<CleanupScopePlan>,
    frame_slots: HashMap<hir::SymbolId, FrameSlot>,
    resume_source_exprs: HashMap<SuspendSiteId, hir::Expr>,
    nested_handles: Vec<HandleStateMachinePlan>,
    local_block_return_contexts: Vec<LocalBlockReturnContext>,
}

impl<'a, 'hir> HandlePlanBuilder<'a, 'hir> {
    fn snapshot_synthetic_symbol_seed<T>(&self, f: impl FnOnce() -> T) -> T {
        let saved_seed = self.context.synthetic_symbol_seed();
        let result = f();
        self.context.restore_synthetic_symbol_seed(saved_seed);
        result
    }

    fn nested_handle_may_suspend_outward(&self, handle: &hir::HandleExpr) -> bool {
        self.snapshot_synthetic_symbol_seed(|| {
            HandleStateMachinePlan::build_with_context(self.types, handle, self.context)
                .may_suspend_outward()
        })
    }

    fn arm_body_may_suspend_outward(&self, arm: &hir::HandleArm) -> bool {
        match arm.kind {
            hir::HandleArmKind::NonResuming => self.expr_contains_suspend_subtree(&arm.body),
            hir::HandleArmKind::EscapeContinuation { continuation } => {
                if self.tail_resume_arm_matches(&arm.body, continuation) {
                    self.tail_resume_arm_may_suspend_outward(&arm.body, continuation)
                } else {
                    self.expr_contains_suspend_subtree(&arm.body)
                }
            }
        }
    }

    fn tail_resume_arm_matches(
        &self,
        expr: &hir::Expr,
        continuation_symbol: hir::SymbolId,
    ) -> bool {
        tail_resume_arm_matches_static(expr, continuation_symbol)
    }

    fn tail_resume_stmt_matches(
        &self,
        stmt: &hir::Stmt,
        continuation_symbol: hir::SymbolId,
    ) -> bool {
        matches!(&stmt.kind, hir::StmtKind::Expr(expr) if tail_resume_arm_matches_static(expr, continuation_symbol))
    }

    fn tail_resume_arm_may_suspend_outward(
        &self,
        expr: &hir::Expr,
        continuation_symbol: hir::SymbolId,
    ) -> bool {
        if let Some(payload) = extract_tail_resume_payload_expr(expr, continuation_symbol) {
            return self.expr_contains_suspend_subtree(payload);
        }

        match &expr.kind {
            hir::ExprKind::Block(block) => {
                let Some((tail_stmt, prefix_stmts)) = block.stmts.split_last() else {
                    return true;
                };
                prefix_stmts
                    .iter()
                    .any(|stmt| self.stmt_contains_suspend_subtree(stmt))
                    || self.tail_resume_stmt_may_suspend_outward(tail_stmt, continuation_symbol)
            }
            hir::ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => {
                self.expr_contains_suspend_subtree(cond)
                    || self.tail_resume_arm_may_suspend_outward(then_branch, continuation_symbol)
                    || else_branch.as_deref().is_none_or(|expr| {
                        self.tail_resume_arm_may_suspend_outward(expr, continuation_symbol)
                    })
            }
            hir::ExprKind::When { subject, arms } => {
                self.expr_contains_suspend_subtree(subject)
                    || arms.iter().any(|arm| {
                        arm.guard
                            .as_ref()
                            .is_some_and(|guard| self.expr_contains_suspend_subtree(guard))
                            || self
                                .tail_resume_arm_may_suspend_outward(&arm.body, continuation_symbol)
                    })
            }
            _ => true,
        }
    }

    fn tail_resume_stmt_may_suspend_outward(
        &self,
        stmt: &hir::Stmt,
        continuation_symbol: hir::SymbolId,
    ) -> bool {
        let hir::StmtKind::Expr(expr) = &stmt.kind else {
            return true;
        };
        self.tail_resume_arm_may_suspend_outward(expr, continuation_symbol)
    }

    fn local_function_value_may_suspend_when_called(&self, expr: &hir::Expr) -> bool {
        SuspendCallAnalysis {
            types: self.types,
            context: self.context,
        }
        .function_value_may_suspend_when_called(expr, &self.known_local_fun_effects)
    }

    fn record_local_fun_binding_if_needed(&mut self, decl: &hir::ValDecl) {
        let Some(id) = decl.id else {
            return;
        };
        if !hir_ty_is_function_value(self.types, decl.ty) {
            return;
        }
        let may_suspend = decl.init.as_ref().map_or_else(
            || function_ty_declared_effectful(self.types, decl.ty),
            |expr| self.local_function_value_may_suspend_when_called(expr),
        );
        self.known_local_fun_effects.insert(id, may_suspend);
    }

    fn record_local_fun_assignment_if_needed(&mut self, lhs: &hir::Expr, rhs: &hir::Expr) {
        let hir::ExprKind::VarRef(hir::ValueRef::Local { id, .. }) = &lhs.kind else {
            return;
        };
        if !hir_ty_is_function_value(self.types, lhs.ty)
            && !hir_ty_is_function_value(self.types, rhs.ty)
            && !self.known_local_fun_effects.contains_key(id)
        {
            return;
        }
        let may_suspend = self.local_function_value_may_suspend_when_called(rhs);
        let entry = self.known_local_fun_effects.entry(*id).or_insert(false);
        *entry |= may_suspend;
    }

    fn new(
        types: &'a TypeStore,
        handle: &'hir hir::HandleExpr,
        context: &'a HandlePlanContext,
    ) -> Self {
        context.reserve_synthetic_symbol_floor(next_synthetic_symbol_seed(
            handle,
            &context.known_local_metadata,
        ));
        Self {
            types,
            handle,
            context,
            known_local_fun_effects: context.known_local_fun_effects.clone(),
            next_state_id: 0,
            next_site_id: 0,
            next_cleanup_id: 0,
            states: Vec::new(),
            suspend_sites: Vec::new(),
            arm_plans: Vec::new(),
            cleanup_scopes: Vec::new(),
            frame_slots: HashMap::new(),
            resume_source_exprs: HashMap::new(),
            nested_handles: Vec::new(),
            local_block_return_contexts: Vec::new(),
        }
    }

    fn build(mut self) -> HandleStateMachinePlan {
        let outer_slots =
            collect_outer_scope_slots(self.handle, &self.context.known_local_metadata);
        let mut env = ScopeEnv::with_outer(outer_slots.clone());
        for slot in &outer_slots {
            self.frame_slots.insert(slot.id, slot.clone());
        }

        let entry_state = self.new_state("body.entry");
        let exit_state = self.new_state("body.exit");
        let body_end_state = self.build_block(&self.handle.body, entry_state, &mut env);

        let final_exit_state = if let Some(finally_block) = &self.handle.finally {
            let cleanup_entry = self.new_state("cleanup.finally.entry");
            let cleanup_exit = self.new_state("cleanup.finally.exit");
            let cleanup_scope_id = self.next_cleanup_id;
            self.next_cleanup_id = self.next_cleanup_id.saturating_add(1);
            self.cleanup_scopes.push(CleanupScopePlan {
                id: cleanup_scope_id,
                kind: CleanupScopeKind::Finally,
                entry_state: cleanup_entry,
                exit_state: cleanup_exit,
                note: "normal/raise edges converge through a shared finally scope".to_string(),
            });

            self.set_terminator(
                body_end_state,
                StateTerminator::CleanupEnter {
                    scope_id: cleanup_scope_id,
                    next_state: cleanup_entry,
                },
            );

            let mut cleanup_env = ScopeEnv::with_outer(outer_slots);
            let cleanup_end = self.build_block(finally_block, cleanup_entry, &mut cleanup_env);
            self.state_mut(cleanup_end)
                .actions
                .push(HandleStateOp::CleanupEdgeComplete);
            self.set_terminator(cleanup_end, StateTerminator::Goto(cleanup_exit));
            self.set_terminator(cleanup_exit, StateTerminator::Goto(exit_state));
            cleanup_exit
        } else {
            self.set_terminator(body_end_state, StateTerminator::Goto(exit_state));
            exit_state
        };

        self.state_mut(exit_state)
            .actions
            .push(HandleStateOp::ReturnToEnclosingExpression);
        self.set_terminator(exit_state, StateTerminator::ReturnHandle);

        let dispatch_plan = self.build_dispatch_plan();
        self.build_arm_states();
        self.compute_capture_sets();
        self.attach_suspend_source_paths();
        self.attach_suspend_resume_paths();
        self.materialize_resume_fragments();
        self.attach_escape_resume_targets();
        self.compute_capture_sets();
        let frame_layout = self.build_frame_layout();

        let _ = final_exit_state;

        HandleStateMachinePlan {
            handle_span: self.handle.body.span,
            result_ty: self.handle.body.ty,
            entry_state,
            states: self.states,
            suspend_sites: self.suspend_sites,
            arm_plans: self.arm_plans,
            cleanup_scopes: self.cleanup_scopes,
            frame_layout,
            dispatch_plan,
            nested_handles: self.nested_handles,
        }
    }

    fn resume_slot_for_site(&self, site_id: SuspendSiteId) -> Option<FrameSlot> {
        self.states.iter().find_map(|state| {
            state.actions.iter().find_map(|op| match op {
                HandleStateOp::ResumeAfterSite {
                    site_id: resume_site_id,
                    resume_slot: Some(slot),
                    ..
                } if *resume_site_id == site_id => Some(slot.clone()),
                _ => None,
            })
        })
    }

    fn build_block(
        &mut self,
        block: &'hir hir::Block,
        start_state: PlanStateId,
        env: &mut ScopeEnv,
    ) -> PlanStateId {
        let mut state = start_state;
        let saved_len = env.slots.len();
        for stmt in &block.stmts {
            state = self.build_stmt(stmt, state, env);
        }
        env.slots.truncate(saved_len);
        state
    }

    fn build_stmt(
        &mut self,
        stmt: &'hir hir::Stmt,
        current_state: PlanStateId,
        env: &mut ScopeEnv,
    ) -> PlanStateId {
        match &stmt.kind {
            hir::StmtKind::Empty => {
                self.push_action(
                    current_state,
                    HandleStateOp::StmtEmpty {
                        stmt: Box::new(stmt.clone()),
                    },
                );
                current_state
            }
            hir::StmtKind::Expr(expr) => self.build_expr(expr, current_state, env),
            hir::StmtKind::Val(decl) => {
                if self.async_task_ready_decl_uses_local_return_context(decl) {
                    return self.build_async_task_ready_decl(decl, current_state, env);
                }

                let init_from_last_value = self.decl_init_uses_prior_actions(decl.init.as_ref());
                let mut state = current_state;
                if let Some(init) = decl.init.as_ref() {
                    state = self.build_expr_for_consumer(init, state, env);
                }
                self.record_local_fun_binding_if_needed(decl);
                if let Some(id) = self.install_decl_slot(decl, env) {
                    self.push_action(
                        state,
                        HandleStateOp::BindLocal {
                            id,
                            decl: Box::new(decl.clone()),
                            init_from_last_value,
                        },
                    );
                } else {
                    self.push_action(
                        state,
                        HandleStateOp::DeclareAnonymousVal {
                            decl: Box::new(decl.clone()),
                            init_from_last_value,
                        },
                    );
                }
                state
            }
            hir::StmtKind::Assign { lhs, rhs, .. } => {
                let mut state = self.build_expr_for_consumer(lhs, current_state, env);
                state = self.build_expr_for_consumer(rhs, state, env);
                self.record_local_fun_assignment_if_needed(lhs, rhs);
                self.record_stmt_reads(state, stmt);
                self.push_action(
                    state,
                    HandleStateOp::Assign {
                        stmt: Box::new(stmt.clone()),
                    },
                );
                state
            }
            hir::StmtKind::While { cond, body } => {
                self.build_while(stmt, cond, body, current_state, env)
            }
            hir::StmtKind::Break { .. } => {
                self.push_action(
                    current_state,
                    HandleStateOp::Break {
                        stmt: Box::new(stmt.clone()),
                    },
                );
                self.set_terminator(current_state, StateTerminator::ReturnFromFunction);
                self.new_state("unreachable.after.break")
            }
            hir::StmtKind::Continue { .. } => {
                self.push_action(
                    current_state,
                    HandleStateOp::Continue {
                        stmt: Box::new(stmt.clone()),
                    },
                );
                self.set_terminator(current_state, StateTerminator::ReturnFromFunction);
                self.new_state("unreachable.after.continue")
            }
            hir::StmtKind::Return { value } => {
                if let Some(local_return_ctx) = self.local_block_return_contexts.last().cloned() {
                    let state = if let Some(expr) = value {
                        self.build_expr_for_consumer(expr, current_state, env)
                    } else {
                        current_state
                    };
                    let mut synthetic_decl = local_return_ctx.decl.clone();
                    synthetic_decl.init = value.clone();
                    let init_from_last_value =
                        self.decl_init_uses_prior_actions(synthetic_decl.init.as_ref());
                    self.push_action(
                        state,
                        HandleStateOp::BindLocal {
                            id: synthetic_decl
                                .id
                                .expect("async task ready local return target must have a slot"),
                            decl: Box::new(synthetic_decl),
                            init_from_last_value,
                        },
                    );
                    self.set_terminator(
                        state,
                        StateTerminator::Goto(local_return_ctx.continuation_state),
                    );
                    return self.new_state("unreachable.after.local.block.return");
                }

                if let Some(expr) = value {
                    let state = self.build_expr_for_consumer(expr, current_state, env);
                    self.push_action(
                        state,
                        HandleStateOp::Return {
                            stmt: Box::new(stmt.clone()),
                        },
                    );
                    self.set_terminator(state, StateTerminator::ReturnFromFunction);
                    self.new_state("unreachable.after.return")
                } else {
                    self.push_action(
                        current_state,
                        HandleStateOp::Return {
                            stmt: Box::new(stmt.clone()),
                        },
                    );
                    self.set_terminator(current_state, StateTerminator::ReturnFromFunction);
                    self.new_state("unreachable.after.return")
                }
            }
            hir::StmtKind::Todo(kind) => {
                self.push_action(
                    current_state,
                    HandleStateOp::TodoStmt {
                        stmt: Box::new(stmt.clone()),
                        kind: kind.to_string(),
                    },
                );
                current_state
            }
        }
    }

    fn async_task_ready_decl_uses_local_return_context(&self, decl: &hir::ValDecl) -> bool {
        decl.name
            .as_deref()
            .is_some_and(|name| name.starts_with("__task_ready_value"))
            && matches!(
                decl.init.as_ref().map(|expr| &expr.kind),
                Some(hir::ExprKind::Block(_))
            )
    }

    fn decl_init_uses_prior_actions(&self, init: Option<&hir::Expr>) -> bool {
        init.is_some_and(|expr| self.expr_contains_suspend_subtree(expr))
    }

    fn install_decl_slot(
        &mut self,
        decl: &hir::ValDecl,
        env: &mut ScopeEnv,
    ) -> Option<hir::SymbolId> {
        let id = decl.id?;
        let slot = FrameSlot {
            id,
            name: decl
                .name
                .clone()
                .unwrap_or_else(|| format!("local{}", id.as_u32())),
            ty: decl.ty,
            mutable: decl.mutable,
            seed_from_outer_scope: false,
            owner_arm: None,
        };
        // Declarations are the authoritative source of slot metadata. If an
        // earlier fallback path pre-seeded this symbol as immutable /
        // outer-scope, overwrite it here.
        self.frame_slots.insert(id, slot.clone());
        env.push(slot);
        Some(id)
    }

    fn build_async_task_ready_decl(
        &mut self,
        decl: &'hir hir::ValDecl,
        current_state: PlanStateId,
        env: &mut ScopeEnv,
    ) -> PlanStateId {
        let continuation_state = self.new_state("async.task.ready.cont");
        let _ = self.install_decl_slot(decl, env);
        self.record_local_fun_binding_if_needed(decl);
        let init_from_last_value = self.decl_init_uses_prior_actions(decl.init.as_ref());

        self.local_block_return_contexts
            .push(LocalBlockReturnContext {
                decl: decl.clone(),
                continuation_state,
            });

        let mut state = current_state;
        if let Some(init) = decl.init.as_ref() {
            state = self.build_expr_for_consumer(init, state, env);
        }

        let _ = self.local_block_return_contexts.pop();

        if let Some(id) = decl.id {
            self.push_action(
                state,
                HandleStateOp::BindLocal {
                    id,
                    decl: Box::new(decl.clone()),
                    init_from_last_value,
                },
            );
        } else {
            self.push_action(
                state,
                HandleStateOp::DeclareAnonymousVal {
                    decl: Box::new(decl.clone()),
                    init_from_last_value,
                },
            );
        }

        self.set_terminator(state, StateTerminator::Goto(continuation_state));
        continuation_state
    }

    fn build_while(
        &mut self,
        stmt: &'hir hir::Stmt,
        cond: &'hir hir::Expr,
        body: &'hir hir::Block,
        current_state: PlanStateId,
        env: &mut ScopeEnv,
    ) -> PlanStateId {
        let cond_state = self.new_state("while.cond");
        self.push_action(
            cond_state,
            HandleStateOp::WhileCondHeader {
                stmt: Box::new(stmt.clone()),
            },
        );
        self.set_terminator(current_state, StateTerminator::Goto(cond_state));

        let cond_eval_state = self.build_expr_for_consumer(cond, cond_state, env);
        let body_state = self.new_state("while.body");
        let exit_state = self.new_state("while.exit");
        self.record_expr_reads(cond_eval_state, cond);
        self.set_terminator(
            cond_eval_state,
            StateTerminator::Branch {
                condition: HandleBranchCondition::WhileCond {
                    condition: Box::new(cond.clone()),
                },
                then_state: body_state,
                else_state: exit_state,
                merge_state: exit_state,
            },
        );

        let mut body_env = env.clone();
        let body_end = self.build_block(body, body_state, &mut body_env);
        self.push_action(body_end, HandleStateOp::LoopReentry { cond_state });
        self.set_terminator(body_end, StateTerminator::Goto(cond_state));
        exit_state
    }

    fn build_expr(
        &mut self,
        expr: &'hir hir::Expr,
        current_state: PlanStateId,
        env: &mut ScopeEnv,
    ) -> PlanStateId {
        match &expr.kind {
            hir::ExprKind::Missing => {
                self.push_action(
                    current_state,
                    HandleStateOp::ExprMissing {
                        expr: Box::new(expr.clone()),
                    },
                );
                current_state
            }
            hir::ExprKind::Literal(_) => {
                self.push_action(
                    current_state,
                    HandleStateOp::Literal {
                        expr: Box::new(expr.clone()),
                    },
                );
                current_state
            }
            hir::ExprKind::ClassLiteral(_) => {
                self.push_action(
                    current_state,
                    HandleStateOp::Literal {
                        expr: Box::new(expr.clone()),
                    },
                );
                current_state
            }
            hir::ExprKind::VarRef(hir::ValueRef::Local { id, name, .. }) => {
                let slot = self.authoritative_local_slot(*id, name, expr.ty);
                self.frame_slots.entry(*id).or_insert(slot);
                self.push_action(
                    current_state,
                    HandleStateOp::ReadLocal {
                        id: *id,
                        expr: Box::new(expr.clone()),
                    },
                );
                self.record_expr_reads(current_state, expr);
                current_state
            }
            hir::ExprKind::VarRef(value_ref) => {
                if let Some(kind) = self.classify_hidden_suspend_var_ref(value_ref) {
                    self.record_expr_reads(current_state, expr);
                    let site_id =
                        self.new_suspend_site(expr.span, kind, env.available_ids(), current_state);
                    self.push_action(
                        current_state,
                        HandleStateOp::ObjectInitAccessBoundary {
                            site_id,
                            expr: Box::new(expr.clone()),
                        },
                    );
                    self.set_terminator(current_state, StateTerminator::Suspend { site_id });
                    let resume_state = self.new_state(format!("resume.after.site{site_id}"));
                    self.record_resume_source_expr(site_id, expr);
                    self.push_action(
                        resume_state,
                        HandleStateOp::ResumeAfterSite {
                            site_id,
                            reason: ResumeAfterSiteReason::ObjectInitAccess,
                            source_span: expr.span,
                            source_ty: expr.ty,
                            resume_slot: None,
                        },
                    );
                    self.set_suspend_resume_target(site_id, resume_state);
                    return resume_state;
                }
                self.push_action(
                    current_state,
                    HandleStateOp::VarRef {
                        expr: Box::new(expr.clone()),
                    },
                );
                current_state
            }
            hir::ExprKind::UnresolvedIdent { .. } => {
                self.push_action(
                    current_state,
                    HandleStateOp::VarRef {
                        expr: Box::new(expr.clone()),
                    },
                );
                current_state
            }
            hir::ExprKind::StructLit { fields, .. } => {
                let mut state = current_state;
                for field in fields {
                    state = self.build_expr_if_suspend_subtree(&field.value, state, env);
                }
                self.push_action(
                    state,
                    HandleStateOp::StructLit {
                        expr: Box::new(expr.clone()),
                    },
                );
                state
            }
            hir::ExprKind::TupleLit { elements } => {
                let mut state = current_state;
                for element in elements {
                    state = self.build_expr_if_suspend_subtree(element, state, env);
                }
                self.push_action(
                    state,
                    HandleStateOp::TupleLit {
                        expr: Box::new(expr.clone()),
                    },
                );
                state
            }
            hir::ExprKind::InterpolatedString { parts, .. } => {
                let mut state = current_state;
                for part in parts {
                    if let hir::InterpolatedStringPart::Expr { expr } = part {
                        state = self.build_expr_if_suspend_subtree(expr, state, env);
                    }
                }
                self.push_action(
                    state,
                    HandleStateOp::InterpolatedString {
                        expr: Box::new(expr.clone()),
                    },
                );
                state
            }
            hir::ExprKind::Unary { expr: inner, .. }
            | hir::ExprKind::TypeCheck { expr: inner, .. } => {
                let state = self.build_expr_if_suspend_subtree(inner, current_state, env);
                self.record_expr_reads(state, expr);
                self.push_action(
                    state,
                    HandleStateOp::Expr {
                        expr: Box::new(expr.clone()),
                    },
                );
                state
            }
            hir::ExprKind::Cast {
                expr: inner, op, ..
            } => {
                let state = self.build_expr_if_suspend_subtree(inner, current_state, env);
                if matches!(op, ast::CastOp::As) {
                    self.record_expr_reads(state, expr);
                    let site_id = self.new_suspend_site(
                        expr.span,
                        SuspendSiteKind::RuntimeRaise {
                            reason: "ClassCastFailed".to_string(),
                        },
                        env.available_ids(),
                        state,
                    );
                    self.push_action(
                        state,
                        HandleStateOp::RuntimeRaiseBoundary {
                            site_id,
                            expr: Box::new(expr.clone()),
                        },
                    );
                    self.set_terminator(state, StateTerminator::Suspend { site_id });
                    let resume_state = self.new_state(format!("resume.after.site{site_id}"));
                    self.record_resume_source_expr(site_id, expr);
                    self.push_action(
                        resume_state,
                        HandleStateOp::ResumeAfterSite {
                            site_id,
                            reason: ResumeAfterSiteReason::RuntimeRaiseBoundary,
                            source_span: expr.span,
                            source_ty: expr.ty,
                            resume_slot: None,
                        },
                    );
                    self.set_suspend_resume_target(site_id, resume_state);
                    return resume_state;
                }
                self.record_expr_reads(state, expr);
                self.push_action(
                    state,
                    HandleStateOp::Expr {
                        expr: Box::new(expr.clone()),
                    },
                );
                state
            }
            hir::ExprKind::MemberAccess { receiver, member } => {
                let state = self.build_expr_if_suspend_subtree(receiver, current_state, env);
                if let Some(kind) = self.classify_hidden_suspend_member_access(member) {
                    self.record_expr_reads(state, expr);
                    let site_id =
                        self.new_suspend_site(expr.span, kind, env.available_ids(), state);
                    self.push_action(
                        state,
                        HandleStateOp::ObjectInitAccessBoundary {
                            site_id,
                            expr: Box::new(expr.clone()),
                        },
                    );
                    self.set_terminator(state, StateTerminator::Suspend { site_id });
                    let resume_state = self.new_state(format!("resume.after.site{site_id}"));
                    self.record_resume_source_expr(site_id, expr);
                    self.push_action(
                        resume_state,
                        HandleStateOp::ResumeAfterSite {
                            site_id,
                            reason: ResumeAfterSiteReason::ObjectInitAccess,
                            source_span: expr.span,
                            source_ty: expr.ty,
                            resume_slot: None,
                        },
                    );
                    self.set_suspend_resume_target(site_id, resume_state);
                    return resume_state;
                }
                self.record_expr_reads(state, expr);
                self.push_action(
                    state,
                    HandleStateOp::Expr {
                        expr: Box::new(expr.clone()),
                    },
                );
                state
            }
            hir::ExprKind::Binary { lhs, rhs, .. } => {
                let state = self.build_expr_if_suspend_subtree(lhs, current_state, env);
                let state = self.build_expr_if_suspend_subtree(rhs, state, env);
                self.record_expr_reads(state, expr);
                self.push_action(
                    state,
                    HandleStateOp::BinaryExpr {
                        expr: Box::new(expr.clone()),
                    },
                );
                state
            }
            hir::ExprKind::Block(block) => self.build_block(block, current_state, env),
            hir::ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => {
                let cond_state = self.build_expr_for_consumer(cond, current_state, env);
                let then_state = self.new_state("if.then");
                let else_state = self.new_state("if.else");
                let merge_state = self.new_state("if.merge");
                self.record_expr_reads(cond_state, cond);
                self.set_terminator(
                    cond_state,
                    StateTerminator::Branch {
                        condition: HandleBranchCondition::IfCond {
                            condition: Box::new(cond.as_ref().clone()),
                        },
                        then_state,
                        else_state,
                        merge_state,
                    },
                );

                let mut then_env = env.clone();
                let then_end = self.build_expr(then_branch, then_state, &mut then_env);
                self.set_terminator(then_end, StateTerminator::Goto(merge_state));

                let mut else_env = env.clone();
                if let Some(else_branch) = else_branch.as_deref() {
                    let else_end = self.build_expr(else_branch, else_state, &mut else_env);
                    self.set_terminator(else_end, StateTerminator::Goto(merge_state));
                } else {
                    self.push_action(
                        else_state,
                        HandleStateOp::ImplicitElseUnit { span: expr.span },
                    );
                    self.set_terminator(else_state, StateTerminator::Goto(merge_state));
                }
                merge_state
            }
            hir::ExprKind::When { subject, arms } => {
                let mut state = self.build_expr_if_suspend_subtree(subject, current_state, env);
                for arm in arms {
                    if let Some(guard) = arm.guard.as_ref() {
                        state = self.build_expr_if_suspend_subtree(guard, state, env);
                    }
                    state = self.build_expr_if_suspend_subtree(&arm.body, state, env);
                }
                self.push_action(
                    state,
                    HandleStateOp::WhenExpr {
                        expr: Box::new(expr.clone()),
                    },
                );
                state
            }
            hir::ExprKind::Call { callee, args } => {
                let mut state = self.build_expr_if_suspend_subtree(callee, current_state, env);
                for arg in args {
                    state = match arg {
                        hir::CallArg::Positional(expr) => {
                            self.build_expr_if_suspend_subtree(expr, state, env)
                        }
                        hir::CallArg::Named { value, .. } => {
                            self.build_expr_if_suspend_subtree(value, state, env)
                        }
                    };
                }
                if let Some(kind) = self.classify_suspend_call(expr, callee) {
                    self.record_expr_reads(state, expr);
                    let site_id =
                        self.new_suspend_site(expr.span, kind, env.available_ids(), state);
                    self.push_action(
                        state,
                        HandleStateOp::SuspendCall {
                            site_id,
                            expr: Box::new(expr.clone()),
                        },
                    );
                    self.set_terminator(state, StateTerminator::Suspend { site_id });
                    let resume_state = self.new_state(format!("resume.after.site{site_id}"));
                    let resume_slot = self.new_resume_temp_slot(site_id, expr);
                    self.record_resume_source_expr(site_id, expr);
                    self.push_action(
                        resume_state,
                        HandleStateOp::ResumeAfterSite {
                            site_id,
                            reason: ResumeAfterSiteReason::Call,
                            source_span: expr.span,
                            source_ty: expr.ty,
                            resume_slot: Some(resume_slot),
                        },
                    );
                    self.set_suspend_resume_target(site_id, resume_state);
                    return resume_state;
                }
                self.record_expr_reads(state, expr);
                self.push_action(
                    state,
                    HandleStateOp::Call {
                        expr: Box::new(expr.clone()),
                    },
                );
                state
            }
            hir::ExprKind::Perform { op, args, .. } => {
                let mut state = current_state;
                for arg in args {
                    state = match arg {
                        hir::CallArg::Positional(expr) => {
                            self.build_expr_if_suspend_subtree(expr, state, env)
                        }
                        hir::CallArg::Named { value, .. } => {
                            self.build_expr_if_suspend_subtree(value, state, env)
                        }
                    };
                }
                self.record_expr_reads(state, expr);
                let site_id = self.new_suspend_site(
                    expr.span,
                    SuspendSiteKind::Perform {
                        op_fqn: op.fqn.clone(),
                    },
                    env.available_ids(),
                    state,
                );
                self.push_action(
                    state,
                    HandleStateOp::Perform {
                        op_fqn: op.fqn.clone(),
                        expr: Box::new(expr.clone()),
                    },
                );
                self.set_terminator(state, StateTerminator::Suspend { site_id });
                let resume_state = self.new_state(format!("resume.after.site{site_id}"));
                let resume_slot = self.new_resume_temp_slot(site_id, expr);
                self.record_resume_source_expr(site_id, expr);
                self.push_action(
                    resume_state,
                    HandleStateOp::ResumeAfterSite {
                        site_id,
                        reason: ResumeAfterSiteReason::Perform,
                        source_span: expr.span,
                        source_ty: expr.ty,
                        resume_slot: Some(resume_slot),
                    },
                );
                self.set_suspend_resume_target(site_id, resume_state);
                resume_state
            }
            hir::ExprKind::Handle(handle) => {
                let nested_id = self.nested_handles.len();
                let nested =
                    HandleStateMachinePlan::build_with_context(self.types, handle, self.context);
                let nested_may_suspend = nested.may_suspend_outward();
                self.nested_handles.push(nested);
                if nested_may_suspend {
                    self.record_expr_reads(current_state, expr);
                    let site_id = self.new_suspend_site(
                        expr.span,
                        SuspendSiteKind::NestedHandleBoundary {
                            detail: format!("nested#{nested_id}"),
                        },
                        env.available_ids(),
                        current_state,
                    );
                    self.push_action(
                        current_state,
                        HandleStateOp::NestedHandleBoundary {
                            site_id,
                            nested_id,
                            expr: Box::new(expr.clone()),
                        },
                    );
                    self.set_terminator(current_state, StateTerminator::Suspend { site_id });
                    let resume_state = self.new_state(format!("resume.after.site{site_id}"));
                    let resume_slot = self.new_resume_temp_slot(site_id, expr);
                    self.record_resume_source_expr(site_id, expr);
                    self.push_action(
                        resume_state,
                        HandleStateOp::ResumeAfterSite {
                            site_id,
                            reason: ResumeAfterSiteReason::NestedHandleBoundary,
                            source_span: expr.span,
                            source_ty: expr.ty,
                            resume_slot: Some(resume_slot),
                        },
                    );
                    self.set_suspend_resume_target(site_id, resume_state);
                    return resume_state;
                }
                self.push_action(
                    current_state,
                    HandleStateOp::NestedHandle {
                        nested_id,
                        expr: Box::new(expr.clone()),
                    },
                );
                current_state
            }
            hir::ExprKind::Closure(closure) => {
                self.push_action(
                    current_state,
                    HandleStateOp::Closure {
                        expr: Box::new(expr.clone()),
                    },
                );
                self.record_expr_reads(current_state, &closure.body);
                current_state
            }
            hir::ExprKind::Todo(kind) => {
                self.push_action(
                    current_state,
                    HandleStateOp::TodoExpr {
                        expr: Box::new(expr.clone()),
                        kind: kind.to_string(),
                    },
                );
                current_state
            }
        }
    }

    fn build_expr_for_consumer(
        &mut self,
        expr: &'hir hir::Expr,
        current_state: PlanStateId,
        env: &mut ScopeEnv,
    ) -> PlanStateId {
        if self.expr_contains_suspend_subtree(expr) {
            self.build_expr(expr, current_state, env)
        } else {
            current_state
        }
    }

    fn build_expr_if_suspend_subtree(
        &mut self,
        expr: &'hir hir::Expr,
        current_state: PlanStateId,
        env: &mut ScopeEnv,
    ) -> PlanStateId {
        if self.expr_contains_suspend_subtree(expr) {
            self.build_expr(expr, current_state, env)
        } else {
            current_state
        }
    }

    fn expr_contains_suspend_subtree(&self, expr: &hir::Expr) -> bool {
        match &expr.kind {
            hir::ExprKind::Missing
            | hir::ExprKind::Literal(_)
            | hir::ExprKind::UnresolvedIdent { .. }
            | hir::ExprKind::ClassLiteral(_)
            | hir::ExprKind::Closure(_)
            | hir::ExprKind::Todo(_) => false,
            hir::ExprKind::VarRef(value_ref) => {
                self.classify_hidden_suspend_var_ref(value_ref).is_some()
            }
            hir::ExprKind::StructLit { fields, .. } => fields
                .iter()
                .any(|field| self.expr_contains_suspend_subtree(&field.value)),
            hir::ExprKind::TupleLit { elements } => elements
                .iter()
                .any(|element| self.expr_contains_suspend_subtree(element)),
            hir::ExprKind::InterpolatedString { parts, .. } => parts.iter().any(|part| {
                matches!(
                    part,
                    hir::InterpolatedStringPart::Expr { expr }
                        if self.expr_contains_suspend_subtree(expr)
                )
            }),
            hir::ExprKind::Unary { expr: inner, .. }
            | hir::ExprKind::TypeCheck { expr: inner, .. } => {
                self.expr_contains_suspend_subtree(inner)
            }
            hir::ExprKind::Cast {
                expr: inner, op, ..
            } => matches!(op, ast::CastOp::As) || self.expr_contains_suspend_subtree(inner),
            hir::ExprKind::MemberAccess { receiver, member } => {
                self.expr_contains_suspend_subtree(receiver)
                    || self.classify_hidden_suspend_member_access(member).is_some()
            }
            hir::ExprKind::Binary { lhs, rhs, .. } => {
                self.expr_contains_suspend_subtree(lhs) || self.expr_contains_suspend_subtree(rhs)
            }
            hir::ExprKind::Block(block) => self.block_contains_suspend_subtree(block),
            hir::ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => {
                self.expr_contains_suspend_subtree(cond)
                    || self.expr_contains_suspend_subtree(then_branch)
                    || else_branch
                        .as_deref()
                        .is_some_and(|expr| self.expr_contains_suspend_subtree(expr))
            }
            hir::ExprKind::When { subject, arms } => {
                self.expr_contains_suspend_subtree(subject)
                    || arms.iter().any(|arm| {
                        arm.guard
                            .as_ref()
                            .is_some_and(|guard| self.expr_contains_suspend_subtree(guard))
                            || self.expr_contains_suspend_subtree(&arm.body)
                    })
            }
            hir::ExprKind::Call { callee, args } => {
                self.classify_suspend_call(expr, callee).is_some()
                    || self.expr_contains_suspend_subtree(callee)
                    || args.iter().any(|arg| match arg {
                        hir::CallArg::Positional(expr) => self.expr_contains_suspend_subtree(expr),
                        hir::CallArg::Named { value, .. } => {
                            self.expr_contains_suspend_subtree(value)
                        }
                    })
            }
            hir::ExprKind::Perform { .. } => true,
            hir::ExprKind::Handle(handle) => self.nested_handle_may_suspend_outward(handle),
        }
    }

    fn handle_contains_suspend_subtree(&self, handle: &hir::HandleExpr) -> bool {
        self.block_contains_suspend_subtree(&handle.body)
            || handle
                .arms
                .iter()
                .any(|arm| self.expr_contains_suspend_subtree(&arm.body))
            || handle
                .finally
                .as_ref()
                .is_some_and(|finally_block| self.block_contains_suspend_subtree(finally_block))
    }

    fn block_contains_suspend_subtree(&self, block: &hir::Block) -> bool {
        block
            .stmts
            .iter()
            .any(|stmt| self.stmt_contains_suspend_subtree(stmt))
    }

    fn stmt_contains_suspend_subtree(&self, stmt: &hir::Stmt) -> bool {
        match &stmt.kind {
            hir::StmtKind::Empty
            | hir::StmtKind::Break { .. }
            | hir::StmtKind::Continue { .. }
            | hir::StmtKind::Todo(_) => false,
            hir::StmtKind::Expr(expr) => self.expr_contains_suspend_subtree(expr),
            hir::StmtKind::Val(decl) => decl
                .init
                .as_ref()
                .is_some_and(|expr| self.expr_contains_suspend_subtree(expr)),
            hir::StmtKind::Assign { lhs, rhs, .. } => {
                self.expr_contains_suspend_subtree(lhs) || self.expr_contains_suspend_subtree(rhs)
            }
            hir::StmtKind::While { cond, body } => {
                self.expr_contains_suspend_subtree(cond)
                    || self.block_contains_suspend_subtree(body)
            }
            hir::StmtKind::Return { value } => value
                .as_ref()
                .is_some_and(|expr| self.expr_contains_suspend_subtree(expr)),
        }
    }

    fn classify_suspend_call(
        &self,
        expr: &hir::Expr,
        callee: &hir::Expr,
    ) -> Option<SuspendSiteKind> {
        if let Some(kind) = self.classify_builtin_suspend_call(expr.span) {
            return Some(kind);
        }

        if let Some(fqn) = try_extract_callee_fqn(callee)
            && let Some(effectful) = self.context.known_fun_effects.get(&fqn).copied()
        {
            return if effectful {
                Some(SuspendSiteKind::CallStateMachineCallee { callee: fqn })
            } else {
                None
            };
        }

        if let hir::ExprKind::VarRef(hir::ValueRef::Local { id, .. }) = &callee.kind
            && let Some(effectful) = self.known_local_fun_effects.get(id).copied()
        {
            return if effectful {
                Some(SuspendSiteKind::CallMaySuspend {
                    callee: format!("local#{}", id.as_u32()),
                })
            } else {
                None
            };
        }

        if let hir::ExprKind::VarRef(hir::ValueRef::Local { id, .. }) = &callee.kind
            && let Some(slot) = self.frame_slots.get(id)
            && let TypeKind::Ref(RefTypeKind::Function(fun_ty)) = self.types.kind(slot.ty)
        {
            return if fun_ty.effects.is_pure() {
                None
            } else {
                Some(SuspendSiteKind::CallMaySuspend {
                    callee: format!("local#{}", id.as_u32()),
                })
            };
        }

        if let Some(target) = self
            .context
            .program_facts
            .ctor_call_targets
            .get(&self.context.call_site(expr.span))
        {
            let class_name = if target.class_fqn.is_empty() {
                format!("ctor@{:?}", callee.span)
            } else {
                target.class_fqn.clone()
            };
            return Some(SuspendSiteKind::ClassCtorInit { class_name });
        }

        let callee_ty = resolve_plan_expr_concrete_type(
            self.context,
            self.types,
            callee,
            &self.context.known_local_metadata,
        )
        .unwrap_or(callee.ty);
        if let TypeKind::Ref(RefTypeKind::Function(fun_ty)) = self.types.kind(callee_ty) {
            if fun_ty.effects.is_pure() {
                return self
                    .local_function_value_may_suspend_when_called(callee)
                    .then(|| SuspendSiteKind::CallMaySuspend {
                        callee: format!("expr@{:?}", expr.span),
                    });
            }
            return try_extract_callee_fqn(callee).map_or_else(
                || {
                    Some(SuspendSiteKind::CallMaySuspend {
                        callee: format!("expr@{:?}", expr.span),
                    })
                },
                |fqn| Some(SuspendSiteKind::CallStateMachineCallee { callee: fqn }),
            );
        }
        None
    }

    fn classify_builtin_suspend_call(&self, call_span: Span) -> Option<SuspendSiteKind> {
        // `Continuation.resume` 的 builtin 语义只来自上游 typecheck 已确认的 side tables；
        // segmentation 本身不再按成员名、receiver 类型或其它代码形状做推断。
        //
        // 只有 receiver continuation 的 effect row 非 Pure 时，resumed body 才会像普通
        // effectful callee 一样再次 suspend outward，outer handle 需要走
        // resume.after.call replay 主线。Pure continuation 则只保留 hidden
        // `Raise<RuntimeError>` 边界，使 `try { k.resume(...) } catch` 继续保持
        // self-contained nested-handle 语义。
        let call_site = self.context.call_site(call_span);
        if !self
            .context
            .program_facts
            .continuation_resume_call_sites
            .contains(&call_site)
        {
            return None;
        }

        if self
            .context
            .program_facts
            .non_pure_continuation_resume_call_sites
            .contains(&call_site)
        {
            Some(SuspendSiteKind::CallMaySuspend {
                callee: "Continuation.resume".to_string(),
            })
        } else {
            Some(SuspendSiteKind::RuntimeRaise {
                reason: "Continuation.resume".to_string(),
            })
        }
    }

    fn classify_hidden_suspend_var_ref(
        &self,
        value_ref: &hir::ValueRef,
    ) -> Option<SuspendSiteKind> {
        let hir::ValueRef::TopLevel { fqn, .. } = value_ref else {
            return None;
        };
        if self.context.program_facts.object_value_fqns.contains(fqn) {
            Some(SuspendSiteKind::ObjectInitAccess {
                target: fqn.clone(),
            })
        } else if self
            .context
            .program_facts
            .top_level_immutable_value_fqns
            .contains(fqn)
        {
            Some(SuspendSiteKind::TopLevelValueInitAccess {
                target: fqn.clone(),
            })
        } else {
            None
        }
    }

    fn classify_hidden_suspend_member_access(
        &self,
        member: &hir::MemberAccess,
    ) -> Option<SuspendSiteKind> {
        let hir::MemberRef::Value { fqn, .. } = member.resolved.as_ref()? else {
            return None;
        };
        (self.context.program_facts.object_value_fqns.contains(fqn)
            || self
                .context
                .program_facts
                .object_property_fqns
                .contains(fqn))
        .then(|| SuspendSiteKind::ObjectInitAccess {
            target: fqn.clone(),
        })
    }

    fn build_dispatch_plan(&self) -> DispatchPlan {
        let mut by_op: HashMap<String, Vec<ArmPlanId>> = HashMap::new();
        for (idx, arm) in self.handle.arms.iter().enumerate() {
            by_op
                .entry(arm.op.op.fqn.clone())
                .or_default()
                .push(idx as u32);
        }
        let mut entries = by_op
            .into_iter()
            .map(|(op_fqn, arm_ids)| DispatchEntry { op_fqn, arm_ids })
            .collect::<Vec<_>>();
        entries.sort_by(|a, b| a.op_fqn.cmp(&b.op_fqn));
        DispatchPlan { entries }
    }

    fn build_arm_states(&mut self) {
        for (idx, arm) in self.handle.arms.iter().enumerate() {
            let arm_id = idx as ArmPlanId;
            let binder_slots = arm
                .op
                .binders
                .iter()
                .map(|binder| FrameSlot {
                    id: binder.id,
                    name: binder.name.clone(),
                    ty: binder.ty,
                    mutable: false,
                    seed_from_outer_scope: false,
                    owner_arm: Some(arm_id),
                })
                .collect::<Vec<_>>();
            for slot in &binder_slots {
                self.frame_slots.insert(slot.id, slot.clone());
            }

            let mut declared = binder_slots
                .iter()
                .map(|slot| slot.id)
                .collect::<HashSet<_>>();
            match arm.kind {
                hir::HandleArmKind::NonResuming => {}
                hir::HandleArmKind::EscapeContinuation { continuation } => {
                    declared.insert(continuation);
                }
            }
            collect_declared_local_ids_in_expr(&arm.body, &mut declared);

            let mut used = HashMap::new();
            collect_local_refs_in_expr(&arm.body, &mut used);
            let continuation_slot = match arm.kind {
                hir::HandleArmKind::EscapeContinuation { continuation } => {
                    used.get(&continuation).cloned().map(|(name, ty)| {
                        if !self.frame_slots.contains_key(&continuation) {
                            let slot = self.authoritative_local_slot(continuation, &name, ty);
                            self.frame_slots.insert(continuation, slot);
                        }
                        self.frame_slots
                            .get(&continuation)
                            .cloned()
                            .expect("escape continuation slot must exist")
                    })
                }
                hir::HandleArmKind::NonResuming => None,
            };
            let mut capture_locals = Vec::new();
            for (id, (name, ty)) in used {
                if declared.contains(&id) {
                    continue;
                }
                if !self.frame_slots.contains_key(&id) {
                    let slot = self.authoritative_local_slot(id, &name, ty);
                    self.frame_slots.insert(id, slot);
                }
                capture_locals.push(id);
            }
            capture_locals.sort_by_key(|id| id.as_u32());

            let body_may_suspend_outward = self.arm_body_may_suspend_outward(arm);
            let segmented_body = matches!(
                arm.kind,
                hir::HandleArmKind::EscapeContinuation { continuation }
                    if !self.tail_resume_arm_matches(&arm.body, continuation)
            ) && body_may_suspend_outward;
            let body_entry_state = self.new_state(format!("arm{arm_id}.body"));
            self.push_action(
                body_entry_state,
                HandleStateOp::ExecuteArmBody {
                    arm_id,
                    op_fqn: arm.op.op.fqn.clone(),
                    arm: Box::new(arm.clone()),
                    segmented_body,
                },
            );

            let arm_exit = match arm.kind {
                hir::HandleArmKind::NonResuming => ArmBodyExit::ReturnHandle,
                hir::HandleArmKind::EscapeContinuation { continuation }
                    if self.tail_resume_arm_matches(&arm.body, continuation) =>
                {
                    ArmBodyExit::ResumeMatchedSite
                }
                hir::HandleArmKind::EscapeContinuation { .. } => {
                    ArmBodyExit::MaterializeContinuation
                }
            };
            let body_end_state = if segmented_body {
                let mut arm_env = ScopeEnv::default();
                for slot in &binder_slots {
                    arm_env.push(slot.clone());
                }
                if let Some(slot) = continuation_slot.clone() {
                    arm_env.push(slot);
                }
                for local_id in &capture_locals {
                    if let Some(slot) = self.frame_slots.get(local_id).cloned() {
                        arm_env.push(slot);
                    }
                }
                self.build_expr(&arm.body, body_entry_state, &mut arm_env)
            } else {
                body_entry_state
            };
            self.set_terminator(body_end_state, StateTerminator::ArmExit(arm_exit));

            self.arm_plans.push(ArmPlan {
                id: arm_id,
                op_fqn: arm.op.op.fqn.clone(),
                effect_ty: arm.op.effect_ty,
                binder_slots,
                capture_locals,
                body_entry_state,
                body_may_suspend_outward,
            });
        }
    }

    fn build_frame_layout(&self) -> FrameLayoutPlan {
        let mut lifted_ids = self
            .suspend_sites
            .iter()
            .flat_map(|site| site.capture_locals.iter().copied())
            .collect::<Vec<_>>();
        lifted_ids.extend(
            self.arm_plans
                .iter()
                .flat_map(|arm| arm.capture_locals.iter().copied()),
        );
        lifted_ids.sort_by_key(|id| id.as_u32());
        lifted_ids.dedup_by_key(|id| id.as_u32());

        let mut lifted_locals = lifted_ids
            .into_iter()
            .filter_map(|id| self.frame_slots.get(&id).cloned())
            .collect::<Vec<_>>();
        lifted_locals.sort_by_key(|slot| slot.id.as_u32());

        let mut arm_binders = self
            .arm_plans
            .iter()
            .flat_map(|arm| arm.binder_slots.clone())
            .collect::<Vec<_>>();
        arm_binders.sort_by_key(|slot| (slot.owner_arm.unwrap_or(0), slot.id.as_u32()));

        FrameLayoutPlan {
            slots: self.frame_slots.clone(),
            lifted_locals,
            arm_binders,
            has_cleanup_flag: !self.cleanup_scopes.is_empty(),
            has_one_shot_flag: self.states.iter().any(|state| {
                matches!(
                    state.terminator,
                    StateTerminator::ArmExit(ArmBodyExit::MaterializeContinuation)
                )
            }),
        }
    }

    fn compute_capture_sets(&mut self) {
        let successors = build_successor_map(&self.states);
        let state_reads = self
            .states
            .iter()
            .map(|state| (state.id, state.reads.clone()))
            .collect::<HashMap<_, _>>();
        let suspend_state_reads = self
            .states
            .iter()
            .filter_map(|state| match state.terminator {
                StateTerminator::Suspend { site_id } => Some((site_id, state.reads.clone())),
                _ => None,
            })
            .collect::<HashMap<_, _>>();
        for site in &mut self.suspend_sites {
            let mut reachable = reachable_states(site.resume_target, &successors);
            if let Some(escape_resume_target) = site.escape_resume_target {
                reachable.extend(reachable_states(escape_resume_target, &successors));
            }
            let mut used_after = reachable
                .into_iter()
                .flat_map(|state_id| state_reads.get(&state_id).cloned().unwrap_or_default())
                .collect::<Vec<_>>();
            if matches!(
                site.kind,
                SuspendSiteKind::CallMaySuspend { .. }
                    | SuspendSiteKind::CallStateMachineCallee { .. }
            ) {
                used_after.extend(
                    suspend_state_reads
                        .get(&site.id)
                        .cloned()
                        .unwrap_or_default(),
                );
            }
            used_after.sort_by_key(|id| id.as_u32());
            used_after.dedup_by_key(|id| id.as_u32());

            let used_set = used_after.into_iter().collect::<HashSet<_>>();
            site.capture_locals = site
                .available_locals
                .iter()
                .copied()
                .filter(|id| used_set.contains(id))
                .collect::<Vec<_>>();
            site.capture_locals.sort_by_key(|id| id.as_u32());
            site.matching_arms = matching_arms(&self.arm_plans, &site.kind);
        }
    }

    fn attach_suspend_source_paths(&mut self) {
        let mut path = Vec::new();
        for (stmt_idx, stmt) in self.handle.body.stmts.iter().enumerate() {
            let root = SuspendSourceRoot::HandleBodyStmt {
                stmt_idx,
                stmt_span: stmt.span,
            };
            self.attach_suspend_source_paths_in_stmt(stmt, &root, &mut path);
        }
        for (arm_index, arm) in self.handle.arms.iter().enumerate() {
            let root = SuspendSourceRoot::ArmBody {
                arm_index,
                body_span: arm.body.span,
            };
            self.attach_suspend_source_paths_in_expr(&arm.body, &root, &mut path);
        }
        if let Some(finally_block) = self.handle.finally.as_ref() {
            for (stmt_idx, stmt) in finally_block.stmts.iter().enumerate() {
                let root = SuspendSourceRoot::FinallyStmt {
                    stmt_idx,
                    stmt_span: stmt.span,
                };
                self.attach_suspend_source_paths_in_stmt(stmt, &root, &mut path);
            }
        }
    }

    fn attach_suspend_source_paths_in_stmt(
        &mut self,
        stmt: &'hir hir::Stmt,
        root: &SuspendSourceRoot,
        path: &mut Vec<SuspendSourceFramePath>,
    ) {
        match &stmt.kind {
            hir::StmtKind::Empty
            | hir::StmtKind::Break { .. }
            | hir::StmtKind::Continue { .. }
            | hir::StmtKind::Todo(_) => {}
            hir::StmtKind::Val(decl) => {
                let Some(init) = decl.init.as_ref() else {
                    return;
                };
                self.attach_suspend_source_paths_in_expr(init, root, path);
            }
            hir::StmtKind::Expr(expr) => {
                self.attach_suspend_source_paths_in_expr(expr, root, path);
            }
            hir::StmtKind::Assign { lhs, rhs, .. } => {
                self.attach_suspend_source_paths_in_expr(lhs, root, path);
                self.attach_suspend_source_paths_in_expr(rhs, root, path);
            }
            hir::StmtKind::Return { value } => {
                if let Some(value) = value.as_ref() {
                    self.attach_suspend_source_paths_in_expr(value, root, path);
                }
            }
            hir::StmtKind::While { cond, body } => {
                self.attach_suspend_source_paths_in_expr(cond, root, path);
                for (stmt_idx, body_stmt) in body.stmts.iter().enumerate() {
                    path.push(SuspendSourceFramePath::WhileBody {
                        while_cond_span: cond.span,
                        while_body_span: body.span,
                        stmt_idx,
                    });
                    self.attach_suspend_source_paths_in_stmt(body_stmt, root, path);
                    let _ = path.pop();
                }
            }
        }
    }

    fn attach_suspend_source_paths_in_expr(
        &mut self,
        expr: &'hir hir::Expr,
        root: &SuspendSourceRoot,
        path: &mut Vec<SuspendSourceFramePath>,
    ) {
        self.record_suspend_source_path(expr, root, path);
        match &expr.kind {
            hir::ExprKind::Missing
            | hir::ExprKind::Literal(_)
            | hir::ExprKind::VarRef(_)
            | hir::ExprKind::UnresolvedIdent { .. }
            | hir::ExprKind::ClassLiteral(_)
            | hir::ExprKind::Closure(_)
            | hir::ExprKind::Todo(_) => {}
            hir::ExprKind::Handle(_) => {}
            hir::ExprKind::StructLit { fields, .. } => {
                for field in fields {
                    self.attach_suspend_source_paths_in_expr(&field.value, root, path);
                }
            }
            hir::ExprKind::TupleLit { elements } => {
                for element in elements {
                    self.attach_suspend_source_paths_in_expr(element, root, path);
                }
            }
            hir::ExprKind::InterpolatedString { parts, .. } => {
                for part in parts {
                    let hir::InterpolatedStringPart::Expr { expr: part_expr } = part else {
                        continue;
                    };
                    self.attach_suspend_source_paths_in_expr(part_expr, root, path);
                }
            }
            hir::ExprKind::Unary { expr: inner, .. }
            | hir::ExprKind::TypeCheck { expr: inner, .. }
            | hir::ExprKind::Cast { expr: inner, .. } => {
                self.attach_suspend_source_paths_in_expr(inner, root, path);
            }
            hir::ExprKind::Binary { lhs, rhs, .. } => {
                self.attach_suspend_source_paths_in_expr(lhs, root, path);
                self.attach_suspend_source_paths_in_expr(rhs, root, path);
            }
            hir::ExprKind::Block(block) => {
                for (stmt_idx, stmt) in block.stmts.iter().enumerate() {
                    path.push(SuspendSourceFramePath::Block {
                        block_span: block.span,
                        stmt_idx,
                    });
                    self.attach_suspend_source_paths_in_stmt(stmt, root, path);
                    let _ = path.pop();
                }
            }
            hir::ExprKind::If {
                cond,
                then_branch,
                else_branch,
                ..
            } => {
                self.attach_suspend_source_paths_in_expr(cond, root, path);
                if let hir::ExprKind::Block(block) = &then_branch.kind {
                    for (stmt_idx, stmt) in block.stmts.iter().enumerate() {
                        path.push(SuspendSourceFramePath::IfThen {
                            if_span: expr.span,
                            then_span: block.span,
                            stmt_idx,
                        });
                        self.attach_suspend_source_paths_in_stmt(stmt, root, path);
                        let _ = path.pop();
                    }
                } else {
                    self.attach_suspend_source_paths_in_expr(then_branch, root, path);
                }
                if let Some(else_expr) = else_branch.as_deref()
                    && let hir::ExprKind::Block(block) = &else_expr.kind
                {
                    for (stmt_idx, stmt) in block.stmts.iter().enumerate() {
                        path.push(SuspendSourceFramePath::IfElse {
                            if_span: expr.span,
                            else_span: block.span,
                            stmt_idx,
                        });
                        self.attach_suspend_source_paths_in_stmt(stmt, root, path);
                        let _ = path.pop();
                    }
                } else if let Some(else_expr) = else_branch.as_deref() {
                    self.attach_suspend_source_paths_in_expr(else_expr, root, path);
                }
            }
            hir::ExprKind::When { subject, arms } => {
                self.attach_suspend_source_paths_in_expr(subject, root, path);
                for (arm_index, when_arm) in arms.iter().enumerate() {
                    if let Some(guard) = when_arm.guard.as_ref() {
                        self.attach_suspend_source_paths_in_expr(guard, root, path);
                    }
                    if let hir::ExprKind::Block(block) = &when_arm.body.kind {
                        for (stmt_idx, stmt) in block.stmts.iter().enumerate() {
                            path.push(SuspendSourceFramePath::WhenArm {
                                when_span: expr.span,
                                arm_index,
                                arm_span: block.span,
                                stmt_idx,
                            });
                            self.attach_suspend_source_paths_in_stmt(stmt, root, path);
                            let _ = path.pop();
                        }
                    } else {
                        self.attach_suspend_source_paths_in_expr(&when_arm.body, root, path);
                    }
                }
            }
            hir::ExprKind::MemberAccess { receiver, .. } => {
                self.attach_suspend_source_paths_in_expr(receiver, root, path);
            }
            hir::ExprKind::Call { callee, args } => {
                self.attach_suspend_source_paths_in_expr(callee, root, path);
                for arg in args {
                    match arg {
                        hir::CallArg::Positional(arg_expr) => {
                            self.attach_suspend_source_paths_in_expr(arg_expr, root, path)
                        }
                        hir::CallArg::Named { value, .. } => {
                            self.attach_suspend_source_paths_in_expr(value, root, path)
                        }
                    }
                }
            }
            hir::ExprKind::Perform { args, .. } => {
                for arg in args {
                    match arg {
                        hir::CallArg::Positional(arg_expr) => {
                            self.attach_suspend_source_paths_in_expr(arg_expr, root, path)
                        }
                        hir::CallArg::Named { value, .. } => {
                            self.attach_suspend_source_paths_in_expr(value, root, path)
                        }
                    }
                }
            }
        }
    }

    fn record_suspend_source_path(
        &mut self,
        expr: &'hir hir::Expr,
        root: &SuspendSourceRoot,
        path: &[SuspendSourceFramePath],
    ) {
        let Some(site) = self.suspend_sites.iter_mut().find(|site| {
            suspend_site_kind_matches_source_path_expr_kind(&site.kind, &expr.kind)
                && site.span == expr.span
                && site.source_path.is_none()
        }) else {
            return;
        };
        site.source_path = Some(SuspendSourcePath {
            root: root.clone(),
            frames: path.to_vec(),
        });
    }

    fn attach_suspend_resume_paths(&mut self) {
        for stmt in &self.handle.body.stmts {
            self.attach_suspend_resume_paths_in_stmt(stmt);
        }
        for arm in &self.handle.arms {
            self.attach_suspend_resume_paths_in_expr(
                &arm.body,
                SuspendResumeConsumer::ExprStmt,
                &mut Vec::new(),
            );
        }
        if let Some(finally_block) = self.handle.finally.as_ref() {
            for stmt in &finally_block.stmts {
                self.attach_suspend_resume_paths_in_stmt(stmt);
            }
        }
    }

    fn attach_suspend_resume_paths_in_stmt(&mut self, stmt: &'hir hir::Stmt) {
        match &stmt.kind {
            hir::StmtKind::Empty
            | hir::StmtKind::Break { .. }
            | hir::StmtKind::Continue { .. }
            | hir::StmtKind::Todo(_) => {}
            hir::StmtKind::Expr(expr) => {
                self.attach_suspend_resume_paths_in_expr(
                    expr,
                    SuspendResumeConsumer::ExprStmt,
                    &mut Vec::new(),
                );
            }
            hir::StmtKind::Val(decl) => {
                if let Some(init) = decl.init.as_ref() {
                    self.attach_suspend_resume_paths_in_expr(
                        init,
                        SuspendResumeConsumer::ValInit,
                        &mut Vec::new(),
                    );
                }
            }
            hir::StmtKind::Assign { lhs, rhs, .. } => {
                self.attach_suspend_resume_paths_in_expr(
                    lhs,
                    SuspendResumeConsumer::AssignLhs,
                    &mut Vec::new(),
                );
                self.attach_suspend_resume_paths_in_expr(
                    rhs,
                    SuspendResumeConsumer::AssignRhs,
                    &mut Vec::new(),
                );
            }
            hir::StmtKind::While { cond, body } => {
                self.attach_suspend_resume_paths_in_expr(
                    cond,
                    SuspendResumeConsumer::WhileCond,
                    &mut Vec::new(),
                );
                for stmt in &body.stmts {
                    self.attach_suspend_resume_paths_in_stmt(stmt);
                }
            }
            hir::StmtKind::Return { value } => {
                if let Some(expr) = value {
                    self.attach_suspend_resume_paths_in_expr(
                        expr,
                        SuspendResumeConsumer::ReturnValue,
                        &mut Vec::new(),
                    );
                }
            }
        }
    }

    fn attach_suspend_resume_paths_in_expr(
        &mut self,
        expr: &'hir hir::Expr,
        consumer: SuspendResumeConsumer,
        expr_frames: &mut Vec<SuspendResumeExprFrame>,
    ) {
        self.record_suspend_resume_path(expr, consumer, expr_frames);
        match &expr.kind {
            hir::ExprKind::Missing
            | hir::ExprKind::Literal(_)
            | hir::ExprKind::VarRef(_)
            | hir::ExprKind::UnresolvedIdent { .. }
            | hir::ExprKind::ClassLiteral(_)
            | hir::ExprKind::Closure(_)
            | hir::ExprKind::Todo(_) => {}
            hir::ExprKind::StructLit { fields, .. } => {
                for field in fields {
                    expr_frames.push(SuspendResumeExprFrame::StructField {
                        struct_span: expr.span,
                        field_name: field.name.clone(),
                    });
                    self.attach_suspend_resume_paths_in_expr(&field.value, consumer, expr_frames);
                    let _ = expr_frames.pop();
                }
            }
            hir::ExprKind::TupleLit { elements } => {
                for (element_index, element) in elements.iter().enumerate() {
                    expr_frames.push(SuspendResumeExprFrame::TupleElement {
                        tuple_span: expr.span,
                        element_index,
                    });
                    self.attach_suspend_resume_paths_in_expr(element, consumer, expr_frames);
                    let _ = expr_frames.pop();
                }
            }
            hir::ExprKind::InterpolatedString { parts, .. } => {
                for (part_index, part) in parts.iter().enumerate() {
                    let hir::InterpolatedStringPart::Expr { expr: part_expr } = part else {
                        continue;
                    };
                    expr_frames.push(SuspendResumeExprFrame::InterpolatedExpr {
                        string_span: expr.span,
                        part_index,
                    });
                    self.attach_suspend_resume_paths_in_expr(part_expr, consumer, expr_frames);
                    let _ = expr_frames.pop();
                }
            }
            hir::ExprKind::Unary { expr: inner, .. } => {
                expr_frames.push(SuspendResumeExprFrame::UnaryOperand {
                    expr_span: expr.span,
                });
                self.attach_suspend_resume_paths_in_expr(inner, consumer, expr_frames);
                let _ = expr_frames.pop();
            }
            hir::ExprKind::Binary { lhs, rhs, .. } => {
                expr_frames.push(SuspendResumeExprFrame::BinaryLhs {
                    binary_span: expr.span,
                });
                self.attach_suspend_resume_paths_in_expr(lhs, consumer, expr_frames);
                let _ = expr_frames.pop();

                expr_frames.push(SuspendResumeExprFrame::BinaryRhs {
                    binary_span: expr.span,
                });
                self.attach_suspend_resume_paths_in_expr(rhs, consumer, expr_frames);
                let _ = expr_frames.pop();
            }
            hir::ExprKind::TypeCheck { expr: inner, .. } => {
                expr_frames.push(SuspendResumeExprFrame::TypeCheckOperand {
                    expr_span: expr.span,
                });
                self.attach_suspend_resume_paths_in_expr(inner, consumer, expr_frames);
                let _ = expr_frames.pop();
            }
            hir::ExprKind::Cast { expr: inner, .. } => {
                expr_frames.push(SuspendResumeExprFrame::CastOperand {
                    expr_span: expr.span,
                });
                self.attach_suspend_resume_paths_in_expr(inner, consumer, expr_frames);
                let _ = expr_frames.pop();
            }
            hir::ExprKind::Block(block) => {
                for stmt in &block.stmts {
                    self.attach_suspend_resume_paths_in_stmt(stmt);
                }
            }
            hir::ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => {
                expr_frames.push(SuspendResumeExprFrame::IfCond { if_span: expr.span });
                self.attach_suspend_resume_paths_in_expr(cond, consumer, expr_frames);
                let _ = expr_frames.pop();

                expr_frames.push(SuspendResumeExprFrame::IfThenExpr { if_span: expr.span });
                self.attach_suspend_resume_paths_in_expr(then_branch, consumer, expr_frames);
                let _ = expr_frames.pop();

                if let Some(else_branch) = else_branch.as_deref() {
                    expr_frames.push(SuspendResumeExprFrame::IfElseExpr { if_span: expr.span });
                    self.attach_suspend_resume_paths_in_expr(else_branch, consumer, expr_frames);
                    let _ = expr_frames.pop();
                }
            }
            hir::ExprKind::When { subject, arms } => {
                expr_frames.push(SuspendResumeExprFrame::WhenSubject {
                    when_span: expr.span,
                });
                self.attach_suspend_resume_paths_in_expr(subject, consumer, expr_frames);
                let _ = expr_frames.pop();

                for (arm_index, arm) in arms.iter().enumerate() {
                    if let Some(guard) = arm.guard.as_ref() {
                        expr_frames.push(SuspendResumeExprFrame::WhenArmGuard {
                            when_span: expr.span,
                            arm_index,
                        });
                        self.attach_suspend_resume_paths_in_expr(guard, consumer, expr_frames);
                        let _ = expr_frames.pop();
                    }

                    expr_frames.push(SuspendResumeExprFrame::WhenArmBody {
                        when_span: expr.span,
                        arm_index,
                    });
                    self.attach_suspend_resume_paths_in_expr(&arm.body, consumer, expr_frames);
                    let _ = expr_frames.pop();
                }
            }
            hir::ExprKind::MemberAccess { receiver, .. } => {
                expr_frames.push(SuspendResumeExprFrame::MemberReceiver {
                    access_span: expr.span,
                });
                self.attach_suspend_resume_paths_in_expr(receiver, consumer, expr_frames);
                let _ = expr_frames.pop();
            }
            hir::ExprKind::Call { callee, args } => {
                expr_frames.push(SuspendResumeExprFrame::CallCallee {
                    call_span: expr.span,
                });
                self.attach_suspend_resume_paths_in_expr(callee, consumer, expr_frames);
                let _ = expr_frames.pop();

                for (arg_index, arg) in args.iter().enumerate() {
                    match arg {
                        hir::CallArg::Positional(arg_expr) => {
                            expr_frames.push(SuspendResumeExprFrame::CallArg {
                                call_span: expr.span,
                                arg_index,
                            });
                            self.attach_suspend_resume_paths_in_expr(
                                arg_expr,
                                consumer,
                                expr_frames,
                            );
                            let _ = expr_frames.pop();
                        }
                        hir::CallArg::Named {
                            name_span, value, ..
                        } => {
                            expr_frames.push(SuspendResumeExprFrame::NamedArgValue {
                                call_span: expr.span,
                                arg_index,
                                name_span: *name_span,
                            });
                            self.attach_suspend_resume_paths_in_expr(value, consumer, expr_frames);
                            let _ = expr_frames.pop();
                        }
                    }
                }
            }
            hir::ExprKind::Perform { args, .. } => {
                for (arg_index, arg) in args.iter().enumerate() {
                    let value = match arg {
                        hir::CallArg::Positional(expr) => expr,
                        hir::CallArg::Named { value, .. } => value,
                    };
                    expr_frames.push(SuspendResumeExprFrame::PerformArg {
                        perform_span: expr.span,
                        arg_index,
                    });
                    self.attach_suspend_resume_paths_in_expr(value, consumer, expr_frames);
                    let _ = expr_frames.pop();
                }
            }
            hir::ExprKind::Handle(_) => {
                // Nested handle boundaries keep their own inner state machine
                // contract. We still record the outer resume_path on the
                // boundary expression itself so inactive returns can feed the
                // authoritative nested-handle result into the outer caller-tail
                // without re-running the inner handle.
            }
        }
    }

    fn record_suspend_resume_path(
        &mut self,
        expr: &'hir hir::Expr,
        consumer: SuspendResumeConsumer,
        expr_frames: &[SuspendResumeExprFrame],
    ) {
        let Some(site) = self.suspend_sites.iter_mut().find(|site| {
            suspend_site_kind_matches_resume_path_expr_kind(&site.kind, &expr.kind)
                && site.span == expr.span
                && site.resume_path.is_none()
        }) else {
            return;
        };
        site.resume_path = Some(SuspendResumePath {
            consumer,
            expr_frames: expr_frames.to_vec(),
        });
    }

    fn new_resume_temp_slot(
        &mut self,
        site_id: SuspendSiteId,
        source_expr: &'hir hir::Expr,
    ) -> FrameSlot {
        let id = self.context.allocate_synthetic_symbol_id();
        let slot = FrameSlot {
            id,
            name: format!("__resume_site{site_id}"),
            ty: source_expr.ty,
            mutable: false,
            seed_from_outer_scope: false,
            owner_arm: None,
        };
        self.frame_slots.insert(id, slot.clone());
        slot
    }

    fn record_resume_source_expr(&mut self, site_id: SuspendSiteId, source_expr: &'hir hir::Expr) {
        self.resume_source_exprs
            .entry(site_id)
            .or_insert_with(|| source_expr.clone());
    }

    fn materialize_resume_fragments(&mut self) {
        let resume_paths = self
            .suspend_sites
            .iter()
            .filter_map(|site| site.resume_path.clone().map(|path| (site.id, path)))
            .collect::<HashMap<_, _>>();
        let source_paths = self
            .suspend_sites
            .iter()
            .filter_map(|site| site.source_path.clone().map(|path| (site.id, path)))
            .collect::<HashMap<_, _>>();

        let original_state_count = self.states.len();
        for state_index in 0..original_state_count {
            let state_id = self.states[state_index].id;
            let mut rewrites = self.states[state_index]
                .actions
                .iter()
                .enumerate()
                .filter_map(|(op_index, op)| match op {
                    HandleStateOp::ResumeAfterSite {
                        site_id,
                        resume_slot: Some(resume_slot),
                        ..
                    } => resume_paths.get(site_id).cloned().map(|resume_path| {
                        let source_expr = self
                            .resume_source_exprs
                            .get(site_id)
                            .unwrap_or_else(|| {
                                panic!(
                                    "resume source expr missing for site{site_id} during rewrite"
                                )
                            })
                            .clone();
                        (
                            op_index,
                            *site_id,
                            source_expr,
                            resume_path,
                            source_paths.get(site_id).cloned(),
                            resume_slot.clone(),
                        )
                    }),
                    _ => None,
                })
                .collect::<Vec<_>>();

            rewrites.sort_by_key(|entry| std::cmp::Reverse(entry.0));

            for (op_index, site_id, source_expr, resume_path, source_path, resume_slot) in rewrites
            {
                {
                    let state = &mut self.states[state_index];
                    for op in state.actions.iter_mut().skip(op_index + 1) {
                        rewrite_state_op_with_resume_slot(
                            op,
                            &source_expr,
                            &resume_path,
                            &resume_slot,
                        );
                    }
                    rewrite_state_terminator_with_resume_slot(
                        &mut state.terminator,
                        &source_expr,
                        &resume_path,
                        &resume_slot,
                    );
                }

                let Some(source_path) = source_path.as_ref() else {
                    self.clone_linear_resume_consumer_chain(
                        state_id,
                        site_id,
                        &source_expr,
                        &resume_path,
                        &resume_slot,
                    );
                    continue;
                };
                let mut allocate_synthetic_symbol_id =
                    || self.context.allocate_synthetic_symbol_id();
                let mut when_rewrite_input = MaterializedWhenResumeInput {
                    source_path,
                    source_expr: &source_expr,
                    resume_path: &resume_path,
                    resume_slot: &resume_slot,
                    allocate_synthetic_symbol_id: &mut allocate_synthetic_symbol_id,
                };
                let when_rewrite = {
                    let state = &self.states[state_index];
                    prepare_materialized_when_resume_rewrite(
                        &state.actions,
                        op_index,
                        &state.terminator,
                        &mut when_rewrite_input,
                    )
                };
                let Some(when_rewrite) = when_rewrite else {
                    self.clone_linear_resume_consumer_chain(
                        state_id,
                        site_id,
                        &source_expr,
                        &resume_path,
                        &resume_slot,
                    );
                    continue;
                };

                {
                    let state = &mut self.states[state_index];
                    if let Some(replacement_expr) = when_rewrite.replacement_expr.as_ref() {
                        for consumer_index in &when_rewrite.consumer_action_indices {
                            rewrite_state_op_replacing_expr_span(
                                &mut state.actions[*consumer_index],
                                when_rewrite.when_span,
                                replacement_expr,
                            );
                        }
                        if when_rewrite.rewrite_terminator {
                            rewrite_state_terminator_replacing_expr_span(
                                &mut state.terminator,
                                when_rewrite.when_span,
                                replacement_expr,
                            );
                        }
                    }

                    let removal_start = if when_rewrite.replacement_expr.is_some() {
                        op_index + 1
                    } else {
                        when_rewrite.when_index
                    };
                    for action_index in (removal_start..=when_rewrite.when_index).rev() {
                        state.actions.remove(action_index);
                    }
                }

                self.clone_linear_resume_consumer_chain(
                    state_id,
                    site_id,
                    &source_expr,
                    &resume_path,
                    &resume_slot,
                );
            }
        }
    }

    fn clone_linear_resume_consumer_chain(
        &mut self,
        resume_state_id: PlanStateId,
        site_id: SuspendSiteId,
        source_expr: &hir::Expr,
        resume_path: &SuspendResumePath,
        resume_slot: &FrameSlot,
    ) {
        let StateTerminator::Goto(first_target) = &self.state(resume_state_id).terminator else {
            return;
        };
        let first_target = *first_target;

        let candidate_spans = resume_rewrite_candidate_spans(source_expr, resume_path);
        let mut seen = HashSet::new();
        let mut chain = Vec::new();
        let mut current = first_target;

        loop {
            if !seen.insert(current) {
                return;
            }

            let state = self.state(current);
            chain.push(current);
            if state_contains_any_expr_span(state, &candidate_spans) {
                break;
            }

            let StateTerminator::Goto(next) = &state.terminator else {
                return;
            };
            current = *next;
        }

        let mut cloned_ids = Vec::with_capacity(chain.len());
        for _ in &chain {
            let cloned_id = self.next_state_id;
            self.next_state_id = self.next_state_id.saturating_add(1);
            cloned_ids.push(cloned_id);
        }

        let consumer_index = chain.len() - 1;
        let mut cloned_states = Vec::with_capacity(chain.len());
        for (idx, original_state_id) in chain.iter().copied().enumerate() {
            let mut cloned = self.state(original_state_id).clone();
            cloned.id = cloned_ids[idx];
            cloned.label = format!("{}.resume.site{site_id}.clone{idx}", cloned.label);

            if idx == consumer_index {
                for op in &mut cloned.actions {
                    rewrite_state_op_with_resume_slot(op, source_expr, resume_path, resume_slot);
                }
                rewrite_state_terminator_with_resume_slot(
                    &mut cloned.terminator,
                    source_expr,
                    resume_path,
                    resume_slot,
                );
            } else {
                cloned.terminator = StateTerminator::Goto(cloned_ids[idx + 1]);
            }

            cloned_states.push(cloned);
        }

        self.states.extend(cloned_states);
        let state = self.state_mut(resume_state_id);
        if let StateTerminator::Goto(target) = &mut state.terminator
            && *target == first_target
        {
            *target = cloned_ids[0];
        }
    }

    fn new_state(&mut self, label: impl Into<String>) -> PlanStateId {
        let id = self.next_state_id;
        self.next_state_id = self.next_state_id.saturating_add(1);
        self.states.push(PlanState {
            id,
            label: label.into(),
            actions: Vec::new(),
            terminator: StateTerminator::ReturnHandle,
            reads: Vec::new(),
        });
        id
    }

    fn push_action(&mut self, state_id: PlanStateId, action: HandleStateOp) {
        self.state_mut(state_id).actions.push(action);
    }

    fn state(&self, state_id: PlanStateId) -> &PlanState {
        self.states
            .iter()
            .find(|state| state.id == state_id)
            .expect("state should exist")
    }

    fn state_mut(&mut self, state_id: PlanStateId) -> &mut PlanState {
        self.states
            .iter_mut()
            .find(|state| state.id == state_id)
            .expect("state should exist")
    }

    fn set_terminator(&mut self, state_id: PlanStateId, terminator: StateTerminator) {
        self.state_mut(state_id).terminator = terminator;
    }

    fn new_suspend_site(
        &mut self,
        span: Span,
        kind: SuspendSiteKind,
        available_locals: Vec<hir::SymbolId>,
        owner_state: PlanStateId,
    ) -> SuspendSiteId {
        let id = self.next_site_id;
        self.next_site_id = self.next_site_id.saturating_add(1);
        let continuation_escape = self.continuation_escape_state_for_suspend_site(span, &kind);
        self.suspend_sites.push(SuspendSitePlan {
            id,
            span,
            kind,
            owner_state,
            resume_target: 0,
            escape_resume_target: None,
            matching_arms: Vec::new(),
            available_locals,
            capture_locals: Vec::new(),
            source_path: None,
            resume_path: None,
            continuation_escape,
        });
        id
    }

    fn continuation_escape_state_for_suspend_site(
        &self,
        span: Span,
        kind: &SuspendSiteKind,
    ) -> ContinuationEscapeState {
        if kind.is_continuation_resume_boundary() {
            self.context.continuation_escape_state_for_call_span(span)
        } else {
            ContinuationEscapeState::Unknown
        }
    }

    fn set_suspend_resume_target(&mut self, site_id: SuspendSiteId, resume_target: PlanStateId) {
        let site = self
            .suspend_sites
            .iter_mut()
            .find(|site| site.id == site_id)
            .expect("site should exist");
        site.resume_target = resume_target;
    }

    fn attach_escape_resume_targets(&mut self) {
        let original_state_count = self.states.len();
        let mut replay_states = Vec::<(SuspendSiteId, PlanState)>::new();
        let replayable_sites = self
            .suspend_sites
            .iter()
            .filter(|site| site.kind.needs_escape_resume_replay())
            .map(|site| site.id)
            .collect::<HashSet<_>>();

        for state in self.states.iter().take(original_state_count) {
            let Some(HandleStateOp::ResumeAfterSite {
                resume_slot: Some(_),
                ..
            }) = state.actions.first()
            else {
                continue;
            };
            let StateTerminator::Suspend { site_id } = state.terminator else {
                continue;
            };
            // Direct perform/runtime-raise continuations already resume at their
            // dedicated post-site state. Rewriting them back into an owner-state
            // replay path would duplicate earlier effects/prints and corrupt the
            // captured continuation contract.
            if !replayable_sites.contains(&site_id) {
                continue;
            }
            if state.actions.len() <= 1 {
                continue;
            }
            let Some(site) = self.suspend_sites.iter().find(|site| site.id == site_id) else {
                continue;
            };
            let replay_actions = self.escape_replay_actions_for_site(state, site);
            if replay_actions.is_empty() {
                continue;
            }

            let replay_state_id = self.next_state_id + replay_states.len() as u32;
            let replay_state = PlanState {
                id: replay_state_id,
                label: format!("{}.escape-replay.site{site_id}", state.label),
                actions: replay_actions,
                terminator: state.terminator.clone(),
                reads: state.reads.clone(),
            };
            replay_states.push((site_id, replay_state));
        }

        if replay_states.is_empty() {
            return;
        }

        self.next_state_id = self
            .next_state_id
            .saturating_add(replay_states.len() as u32);
        for (site_id, replay_state) in replay_states {
            let replay_state_id = replay_state.id;
            self.states.push(replay_state);
            let site = self
                .suspend_sites
                .iter_mut()
                .find(|site| site.id == site_id)
                .expect("escape replay target site should exist");
            site.escape_resume_target = Some(replay_state_id);
        }
    }

    fn escape_replay_actions_for_site(
        &self,
        state: &PlanState,
        site: &SuspendSitePlan,
    ) -> Vec<HandleStateOp> {
        let Some(source_path) = site.source_path.as_ref() else {
            return state.actions[1..].to_vec();
        };
        let root_span = source_path.root_span();

        let replay_actions = state.actions[1..]
            .iter()
            .filter(|op| state_op_within_span(op, root_span))
            .cloned()
            .collect::<Vec<_>>();

        if replay_actions.is_empty() {
            state.actions[1..].to_vec()
        } else {
            replay_actions
        }
    }

    fn record_stmt_reads(&mut self, _state_id: PlanStateId, _stmt: &hir::Stmt) {
        let mut used = HashSet::new();
        collect_used_locals_in_stmt_static(_stmt, &mut used);
        self.add_reads(_state_id, used);
    }

    fn authoritative_local_slot(
        &self,
        id: hir::SymbolId,
        name: &str,
        fallback_ty: TypeId,
    ) -> FrameSlot {
        let metadata = self.context.known_local_metadata.get(&id).copied();
        FrameSlot {
            id,
            name: name.to_string(),
            ty: metadata.map_or(fallback_ty, |meta| meta.ty),
            mutable: metadata.is_some_and(|meta| meta.mutable),
            seed_from_outer_scope: false,
            owner_arm: None,
        }
    }

    fn record_expr_reads(&mut self, _state_id: PlanStateId, _expr: &hir::Expr) {
        let mut used = HashSet::new();
        collect_used_locals_in_expr_static(_expr, &mut used);
        self.add_reads(_state_id, used);
    }

    fn add_reads(&mut self, state_id: PlanStateId, used: HashSet<hir::SymbolId>) {
        let state = self.state_mut(state_id);
        state.reads.extend(used);
        state.reads.sort_by_key(|id| id.as_u32());
        state.reads.dedup_by_key(|id| id.as_u32());
    }
}

fn rewrite_state_op_with_resume_slot(
    op: &mut HandleStateOp,
    source_expr: &hir::Expr,
    resume_path: &SuspendResumePath,
    resume_slot: &FrameSlot,
) {
    match op {
        HandleStateOp::BindLocal { decl, .. } | HandleStateOp::DeclareAnonymousVal { decl, .. } => {
            if let Some(init) = decl.init.as_mut() {
                *init = rewrite_expr_with_resume_slot(init, source_expr, resume_path, resume_slot);
            }
        }
        HandleStateOp::Assign { stmt }
        | HandleStateOp::Return { stmt }
        | HandleStateOp::TodoStmt { stmt, .. }
        | HandleStateOp::StmtEmpty { stmt }
        | HandleStateOp::WhileCondHeader { stmt }
        | HandleStateOp::Break { stmt }
        | HandleStateOp::Continue { stmt } => {
            rewrite_stmt_with_resume_slot(stmt, source_expr, resume_path, resume_slot);
        }
        HandleStateOp::ExprMissing { expr }
        | HandleStateOp::Literal { expr }
        | HandleStateOp::ReadLocal { expr, .. }
        | HandleStateOp::ObjectInitAccessBoundary { expr, .. }
        | HandleStateOp::VarRef { expr }
        | HandleStateOp::StructLit { expr }
        | HandleStateOp::TupleLit { expr }
        | HandleStateOp::InterpolatedString { expr }
        | HandleStateOp::Expr { expr }
        | HandleStateOp::RuntimeRaiseBoundary { expr, .. }
        | HandleStateOp::BinaryExpr { expr }
        | HandleStateOp::WhenExpr { expr }
        | HandleStateOp::SuspendCall { expr, .. }
        | HandleStateOp::Call { expr }
        | HandleStateOp::Perform { expr, .. }
        | HandleStateOp::NestedHandleBoundary { expr, .. }
        | HandleStateOp::NestedHandle { expr, .. }
        | HandleStateOp::Closure { expr }
        | HandleStateOp::TodoExpr { expr, .. } => {
            **expr = rewrite_expr_with_resume_slot(expr, source_expr, resume_path, resume_slot);
        }
        HandleStateOp::ResumeAfterSite { .. }
        | HandleStateOp::CleanupEdgeComplete
        | HandleStateOp::ReturnToEnclosingExpression
        | HandleStateOp::LoopReentry { .. }
        | HandleStateOp::ImplicitElseUnit { .. }
        | HandleStateOp::ExecuteArmBody { .. } => {}
    }
}

fn build_ordinary_callee_resume_tail_block(
    body: &hir::Block,
    source_path: &SuspendSourcePath,
    source_expr: &hir::Expr,
    resume_path: &SuspendResumePath,
    resume_slot: &FrameSlot,
    allocate_synthetic_symbol_id: &mut dyn FnMut() -> hir::SymbolId,
) -> Option<hir::Block> {
    let start_idx = source_path.handle_body_stmt_idx()?;
    build_resume_tail_block_from_stmt_slice(
        body,
        start_idx,
        &source_path.frames,
        source_expr,
        resume_path,
        resume_slot,
        allocate_synthetic_symbol_id,
    )
}

fn build_resume_tail_block_from_stmt_slice(
    block: &hir::Block,
    start_idx: usize,
    frames: &[SuspendSourceFramePath],
    source_expr: &hir::Expr,
    resume_path: &SuspendResumePath,
    resume_slot: &FrameSlot,
    allocate_synthetic_symbol_id: &mut dyn FnMut() -> hir::SymbolId,
) -> Option<hir::Block> {
    let first_stmt = block.stmts.get(start_idx)?;
    let rebuilt_first = build_resume_tail_stmt(
        first_stmt,
        frames,
        source_expr,
        resume_path,
        resume_slot,
        allocate_synthetic_symbol_id,
    )?;
    let mut tail_stmts = vec![rebuilt_first];

    // Resume-tail rebuilding is path-specific. When the rebuilt leading stmt
    // already exits control flow (for example nested `return perform(...)`
    // rewritten to `return __resume_siteN`), sibling stmts from the original
    // enclosing block are unreachable on the resumed path and must not be
    // appended, otherwise ordinary callee resume blocks emit instructions
    // after a terminator.
    if !stmt_guarantees_control_flow_exit(&tail_stmts[0]) {
        tail_stmts.extend(block.stmts.iter().skip(start_idx + 1).cloned());
    }

    let tail_span = tail_stmts
        .first()
        .map(|stmt| stmt.span)
        .unwrap_or(block.span);
    Some(hir::Block {
        span: tail_span,
        ty: block.ty,
        stmts: tail_stmts,
    })
}

fn stmt_guarantees_control_flow_exit(stmt: &hir::Stmt) -> bool {
    match &stmt.kind {
        hir::StmtKind::Expr(expr) => expr_guarantees_control_flow_exit(expr),
        hir::StmtKind::Return { .. }
        | hir::StmtKind::Break { .. }
        | hir::StmtKind::Continue { .. } => true,
        hir::StmtKind::Empty
        | hir::StmtKind::Val(_)
        | hir::StmtKind::Assign { .. }
        | hir::StmtKind::While { .. }
        | hir::StmtKind::Todo(_) => false,
    }
}

fn block_guarantees_control_flow_exit(block: &hir::Block) -> bool {
    block.stmts.iter().any(stmt_guarantees_control_flow_exit)
}

fn when_expr_guarantees_control_flow_exit(arms: &[hir::WhenArm]) -> bool {
    !arms.is_empty()
        && arms
            .iter()
            .all(|arm| expr_guarantees_control_flow_exit(&arm.body))
}

fn expr_guarantees_control_flow_exit(expr: &hir::Expr) -> bool {
    match &expr.kind {
        hir::ExprKind::Block(block) => block_guarantees_control_flow_exit(block),
        hir::ExprKind::If {
            then_branch,
            else_branch,
            ..
        } => else_branch.as_deref().is_some_and(|else_branch| {
            expr_guarantees_control_flow_exit(then_branch)
                && expr_guarantees_control_flow_exit(else_branch)
        }),
        hir::ExprKind::When { arms, .. } => when_expr_guarantees_control_flow_exit(arms),
        _ => false,
    }
}

fn build_resume_tail_stmt(
    stmt: &hir::Stmt,
    frames: &[SuspendSourceFramePath],
    source_expr: &hir::Expr,
    resume_path: &SuspendResumePath,
    resume_slot: &FrameSlot,
    allocate_synthetic_symbol_id: &mut dyn FnMut() -> hir::SymbolId,
) -> Option<hir::Stmt> {
    if frames.is_empty() {
        let mut rewritten = stmt.clone();
        rewrite_stmt_with_resume_slot(&mut rewritten, source_expr, resume_path, resume_slot);
        return Some(rewritten);
    }

    match &stmt.kind {
        hir::StmtKind::Expr(expr) => {
            let rebuilt_expr = build_resume_tail_expr(
                expr,
                frames,
                source_expr,
                resume_path,
                resume_slot,
                allocate_synthetic_symbol_id,
            )?;
            Some(hir::Stmt {
                span: stmt.span,
                ty: stmt.ty,
                kind: hir::StmtKind::Expr(rebuilt_expr),
            })
        }
        hir::StmtKind::Val(decl) => {
            let init = decl.init.as_ref()?;
            let rebuilt_init = build_resume_tail_expr(
                init,
                frames,
                source_expr,
                resume_path,
                resume_slot,
                allocate_synthetic_symbol_id,
            )?;
            let mut rebuilt_decl = decl.clone();
            rebuilt_decl.init = Some(rebuilt_init);
            Some(hir::Stmt {
                span: stmt.span,
                ty: stmt.ty,
                kind: hir::StmtKind::Val(rebuilt_decl),
            })
        }
        hir::StmtKind::Assign { lhs, eq_span, rhs } => {
            if let Some(rebuilt_lhs) = build_resume_tail_expr(
                lhs,
                frames,
                source_expr,
                resume_path,
                resume_slot,
                allocate_synthetic_symbol_id,
            ) {
                return Some(hir::Stmt {
                    span: stmt.span,
                    ty: stmt.ty,
                    kind: hir::StmtKind::Assign {
                        lhs: rebuilt_lhs,
                        eq_span: *eq_span,
                        rhs: rhs.clone(),
                    },
                });
            }

            let rebuilt_rhs = build_resume_tail_expr(
                rhs,
                frames,
                source_expr,
                resume_path,
                resume_slot,
                allocate_synthetic_symbol_id,
            )?;
            Some(hir::Stmt {
                span: stmt.span,
                ty: stmt.ty,
                kind: hir::StmtKind::Assign {
                    lhs: lhs.clone(),
                    eq_span: *eq_span,
                    rhs: rebuilt_rhs,
                },
            })
        }
        hir::StmtKind::While { cond, body } => {
            if let Some(SuspendSourceFramePath::WhileBody {
                while_cond_span,
                while_body_span,
                stmt_idx,
            }) = frames.first()
                && cond.span == *while_cond_span
                && body.span == *while_body_span
            {
                let current_iteration_tail = build_resume_tail_block_from_stmt_slice(
                    body,
                    *stmt_idx,
                    &frames[1..],
                    source_expr,
                    resume_path,
                    resume_slot,
                    allocate_synthetic_symbol_id,
                )?;
                return Some(build_resume_tail_while_stmt(
                    stmt,
                    cond,
                    body,
                    current_iteration_tail,
                    allocate_synthetic_symbol_id,
                ));
            }

            let rebuilt_cond = build_resume_tail_expr(
                cond,
                frames,
                source_expr,
                resume_path,
                resume_slot,
                allocate_synthetic_symbol_id,
            )?;
            Some(hir::Stmt {
                span: stmt.span,
                ty: stmt.ty,
                kind: hir::StmtKind::While {
                    cond: rebuilt_cond,
                    body: body.clone(),
                },
            })
        }
        hir::StmtKind::Return { value } => {
            let expr = value.as_ref()?;
            let rebuilt = build_resume_tail_expr(
                expr,
                frames,
                source_expr,
                resume_path,
                resume_slot,
                allocate_synthetic_symbol_id,
            )?;
            Some(hir::Stmt {
                span: stmt.span,
                ty: stmt.ty,
                kind: hir::StmtKind::Return {
                    value: Some(rebuilt),
                },
            })
        }
        hir::StmtKind::Empty
        | hir::StmtKind::Break { .. }
        | hir::StmtKind::Continue { .. }
        | hir::StmtKind::Todo(_) => None,
    }
}

fn build_resume_tail_expr(
    expr: &hir::Expr,
    frames: &[SuspendSourceFramePath],
    source_expr: &hir::Expr,
    resume_path: &SuspendResumePath,
    resume_slot: &FrameSlot,
    allocate_synthetic_symbol_id: &mut dyn FnMut() -> hir::SymbolId,
) -> Option<hir::Expr> {
    if frames.is_empty() {
        return Some(rewrite_expr_with_resume_slot(
            expr,
            source_expr,
            resume_path,
            resume_slot,
        ));
    }

    if let Some(frame) = frames.first() {
        match frame {
            SuspendSourceFramePath::Block {
                block_span,
                stmt_idx,
            } => {
                if let hir::ExprKind::Block(block) = &expr.kind
                    && block.span == *block_span
                {
                    let rebuilt_block = build_resume_tail_block_from_stmt_slice(
                        block,
                        *stmt_idx,
                        &frames[1..],
                        source_expr,
                        resume_path,
                        resume_slot,
                        allocate_synthetic_symbol_id,
                    )?;
                    return Some(make_block_expr_with_original_span(expr, rebuilt_block));
                }
            }
            SuspendSourceFramePath::IfThen {
                if_span,
                then_span,
                stmt_idx,
            } => {
                if let hir::ExprKind::If { then_branch, .. } = &expr.kind
                    && expr.span == *if_span
                    && let hir::ExprKind::Block(block) = &then_branch.kind
                    && block.span == *then_span
                {
                    let rebuilt_block = build_resume_tail_block_from_stmt_slice(
                        block,
                        *stmt_idx,
                        &frames[1..],
                        source_expr,
                        resume_path,
                        resume_slot,
                        allocate_synthetic_symbol_id,
                    )?;
                    return Some(make_block_expr_with_original_span(expr, rebuilt_block));
                }
            }
            SuspendSourceFramePath::IfElse {
                if_span,
                else_span,
                stmt_idx,
            } => {
                if let hir::ExprKind::If {
                    else_branch: Some(else_branch),
                    ..
                } = &expr.kind
                    && expr.span == *if_span
                    && let hir::ExprKind::Block(block) = &else_branch.kind
                    && block.span == *else_span
                {
                    let rebuilt_block = build_resume_tail_block_from_stmt_slice(
                        block,
                        *stmt_idx,
                        &frames[1..],
                        source_expr,
                        resume_path,
                        resume_slot,
                        allocate_synthetic_symbol_id,
                    )?;
                    return Some(make_block_expr_with_original_span(expr, rebuilt_block));
                }
            }
            SuspendSourceFramePath::WhenArm {
                when_span,
                arm_index,
                arm_span,
                stmt_idx,
            } => {
                if let hir::ExprKind::When { arms, .. } = &expr.kind
                    && expr.span == *when_span
                    && let Some(arm) = arms.get(*arm_index)
                    && let hir::ExprKind::Block(block) = &arm.body.kind
                    && block.span == *arm_span
                {
                    let rebuilt_block = build_resume_tail_block_from_stmt_slice(
                        block,
                        *stmt_idx,
                        &frames[1..],
                        source_expr,
                        resume_path,
                        resume_slot,
                        allocate_synthetic_symbol_id,
                    )?;
                    return Some(make_block_expr_with_original_span(expr, rebuilt_block));
                }
            }
            SuspendSourceFramePath::WhileBody { .. } => {}
        }
    }

    match &expr.kind {
        hir::ExprKind::StructLit { fields, ty } => {
            for (field_index, field) in fields.iter().enumerate() {
                let Some(rebuilt_value) = build_resume_tail_expr(
                    &field.value,
                    frames,
                    source_expr,
                    resume_path,
                    resume_slot,
                    allocate_synthetic_symbol_id,
                ) else {
                    continue;
                };
                let mut rebuilt_fields = fields.clone();
                rebuilt_fields[field_index].value = rebuilt_value;
                return Some(hir::Expr {
                    span: expr.span,
                    ty: expr.ty,
                    kind: hir::ExprKind::StructLit {
                        ty: *ty,
                        fields: rebuilt_fields,
                    },
                });
            }
            None
        }
        hir::ExprKind::TupleLit { elements } => {
            for (element_index, element) in elements.iter().enumerate() {
                let Some(rebuilt_element) = build_resume_tail_expr(
                    element,
                    frames,
                    source_expr,
                    resume_path,
                    resume_slot,
                    allocate_synthetic_symbol_id,
                ) else {
                    continue;
                };
                let mut rebuilt_elements = elements.clone();
                rebuilt_elements[element_index] = rebuilt_element;
                return Some(hir::Expr {
                    span: expr.span,
                    ty: expr.ty,
                    kind: hir::ExprKind::TupleLit {
                        elements: rebuilt_elements,
                    },
                });
            }
            None
        }
        hir::ExprKind::InterpolatedString { raw, parts } => {
            for (part_index, part) in parts.iter().enumerate() {
                let hir::InterpolatedStringPart::Expr { expr: part_expr } = part else {
                    continue;
                };
                let Some(rebuilt_expr) = build_resume_tail_expr(
                    part_expr,
                    frames,
                    source_expr,
                    resume_path,
                    resume_slot,
                    allocate_synthetic_symbol_id,
                ) else {
                    continue;
                };
                let mut rebuilt_parts = parts.clone();
                rebuilt_parts[part_index] =
                    hir::InterpolatedStringPart::Expr { expr: rebuilt_expr };
                return Some(hir::Expr {
                    span: expr.span,
                    ty: expr.ty,
                    kind: hir::ExprKind::InterpolatedString {
                        raw: *raw,
                        parts: rebuilt_parts,
                    },
                });
            }
            None
        }
        hir::ExprKind::Unary {
            op,
            op_span,
            expr: inner,
        } => build_resume_tail_expr(
            inner,
            frames,
            source_expr,
            resume_path,
            resume_slot,
            allocate_synthetic_symbol_id,
        )
        .map(|rewritten_inner| hir::Expr {
            span: expr.span,
            ty: expr.ty,
            kind: hir::ExprKind::Unary {
                op: *op,
                op_span: *op_span,
                expr: Box::new(rewritten_inner),
            },
        }),
        hir::ExprKind::TypeCheck {
            expr: inner,
            op,
            op_span,
            target_ty,
        } => build_resume_tail_expr(
            inner,
            frames,
            source_expr,
            resume_path,
            resume_slot,
            allocate_synthetic_symbol_id,
        )
        .map(|rewritten_inner| hir::Expr {
            span: expr.span,
            ty: expr.ty,
            kind: hir::ExprKind::TypeCheck {
                expr: Box::new(rewritten_inner),
                op: *op,
                op_span: *op_span,
                target_ty: *target_ty,
            },
        }),
        hir::ExprKind::Cast {
            expr: inner,
            op,
            op_span,
            target_ty,
        } => build_resume_tail_expr(
            inner,
            frames,
            source_expr,
            resume_path,
            resume_slot,
            allocate_synthetic_symbol_id,
        )
        .map(|rewritten_inner| hir::Expr {
            span: expr.span,
            ty: expr.ty,
            kind: hir::ExprKind::Cast {
                expr: Box::new(rewritten_inner),
                op: *op,
                op_span: *op_span,
                target_ty: *target_ty,
            },
        }),
        hir::ExprKind::Binary {
            lhs,
            op,
            op_span,
            rhs,
        } => {
            if let Some(rewritten_lhs) = build_resume_tail_expr(
                lhs,
                frames,
                source_expr,
                resume_path,
                resume_slot,
                allocate_synthetic_symbol_id,
            ) {
                return Some(hir::Expr {
                    span: expr.span,
                    ty: expr.ty,
                    kind: hir::ExprKind::Binary {
                        lhs: Box::new(rewritten_lhs),
                        op: *op,
                        op_span: *op_span,
                        rhs: rhs.clone(),
                    },
                });
            }

            build_resume_tail_expr(
                rhs,
                frames,
                source_expr,
                resume_path,
                resume_slot,
                allocate_synthetic_symbol_id,
            )
            .map(|rewritten_rhs| hir::Expr {
                span: expr.span,
                ty: expr.ty,
                kind: hir::ExprKind::Binary {
                    lhs: lhs.clone(),
                    op: *op,
                    op_span: *op_span,
                    rhs: Box::new(rewritten_rhs),
                },
            })
        }
        hir::ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => {
            if let Some(rewritten_cond) = build_resume_tail_expr(
                cond,
                frames,
                source_expr,
                resume_path,
                resume_slot,
                allocate_synthetic_symbol_id,
            ) {
                return Some(hir::Expr {
                    span: expr.span,
                    ty: expr.ty,
                    kind: hir::ExprKind::If {
                        cond: Box::new(rewritten_cond),
                        then_branch: then_branch.clone(),
                        else_branch: else_branch.clone(),
                    },
                });
            }

            if let Some(rewritten_then) = build_resume_tail_expr(
                then_branch,
                frames,
                source_expr,
                resume_path,
                resume_slot,
                allocate_synthetic_symbol_id,
            ) {
                return Some(hir::Expr {
                    span: expr.span,
                    ty: expr.ty,
                    kind: hir::ExprKind::If {
                        cond: cond.clone(),
                        then_branch: Box::new(rewritten_then),
                        else_branch: else_branch.clone(),
                    },
                });
            }

            let else_branch_expr = else_branch.as_deref()?;
            let rewritten_else = build_resume_tail_expr(
                else_branch_expr,
                frames,
                source_expr,
                resume_path,
                resume_slot,
                allocate_synthetic_symbol_id,
            )?;
            Some(hir::Expr {
                span: expr.span,
                ty: expr.ty,
                kind: hir::ExprKind::If {
                    cond: cond.clone(),
                    then_branch: then_branch.clone(),
                    else_branch: Some(Box::new(rewritten_else)),
                },
            })
        }
        hir::ExprKind::When { subject, arms } => {
            if let Some(rewritten_subject) = build_resume_tail_expr(
                subject,
                frames,
                source_expr,
                resume_path,
                resume_slot,
                allocate_synthetic_symbol_id,
            ) {
                return Some(hir::Expr {
                    span: expr.span,
                    ty: expr.ty,
                    kind: hir::ExprKind::When {
                        subject: Box::new(rewritten_subject),
                        arms: arms.clone(),
                    },
                });
            }

            for (arm_index, arm) in arms.iter().enumerate() {
                if let Some(guard) = arm.guard.as_ref()
                    && let Some(rewritten_guard) = build_resume_tail_expr(
                        guard,
                        frames,
                        source_expr,
                        resume_path,
                        resume_slot,
                        allocate_synthetic_symbol_id,
                    )
                {
                    let mut rebuilt_arms = arms.clone();
                    rebuilt_arms[arm_index].guard = Some(rewritten_guard);
                    return Some(hir::Expr {
                        span: expr.span,
                        ty: expr.ty,
                        kind: hir::ExprKind::When {
                            subject: subject.clone(),
                            arms: rebuilt_arms,
                        },
                    });
                }

                if let Some(rewritten_body) = build_resume_tail_expr(
                    &arm.body,
                    frames,
                    source_expr,
                    resume_path,
                    resume_slot,
                    allocate_synthetic_symbol_id,
                ) {
                    let mut rebuilt_arms = arms.clone();
                    rebuilt_arms[arm_index].body = rewritten_body;
                    return Some(hir::Expr {
                        span: expr.span,
                        ty: expr.ty,
                        kind: hir::ExprKind::When {
                            subject: subject.clone(),
                            arms: rebuilt_arms,
                        },
                    });
                }
            }
            None
        }
        hir::ExprKind::MemberAccess { receiver, member } => build_resume_tail_expr(
            receiver,
            frames,
            source_expr,
            resume_path,
            resume_slot,
            allocate_synthetic_symbol_id,
        )
        .map(|rewritten_receiver| hir::Expr {
            span: expr.span,
            ty: expr.ty,
            kind: hir::ExprKind::MemberAccess {
                receiver: Box::new(rewritten_receiver),
                member: member.clone(),
            },
        }),
        hir::ExprKind::Call { callee, args } => {
            if let Some(rewritten_callee) = build_resume_tail_expr(
                callee,
                frames,
                source_expr,
                resume_path,
                resume_slot,
                allocate_synthetic_symbol_id,
            ) {
                return Some(hir::Expr {
                    span: expr.span,
                    ty: expr.ty,
                    kind: hir::ExprKind::Call {
                        callee: Box::new(rewritten_callee),
                        args: args.clone(),
                    },
                });
            }

            for (arg_index, arg) in args.iter().enumerate() {
                let rebuilt = match arg {
                    hir::CallArg::Positional(arg_expr) => build_resume_tail_expr(
                        arg_expr,
                        frames,
                        source_expr,
                        resume_path,
                        resume_slot,
                        allocate_synthetic_symbol_id,
                    )
                    .map(hir::CallArg::Positional),
                    hir::CallArg::Named {
                        name,
                        name_span,
                        value,
                    } => build_resume_tail_expr(
                        value,
                        frames,
                        source_expr,
                        resume_path,
                        resume_slot,
                        allocate_synthetic_symbol_id,
                    )
                    .map(|rewritten_value| hir::CallArg::Named {
                        name: name.clone(),
                        name_span: *name_span,
                        value: rewritten_value,
                    }),
                };
                let Some(rewritten_arg) = rebuilt else {
                    continue;
                };
                let mut rebuilt_args = args.clone();
                rebuilt_args[arg_index] = rewritten_arg;
                return Some(hir::Expr {
                    span: expr.span,
                    ty: expr.ty,
                    kind: hir::ExprKind::Call {
                        callee: callee.clone(),
                        args: rebuilt_args,
                    },
                });
            }
            None
        }
        hir::ExprKind::Perform {
            effect_ty,
            op,
            args,
        } => {
            for (arg_index, arg) in args.iter().enumerate() {
                let rebuilt = match arg {
                    hir::CallArg::Positional(arg_expr) => build_resume_tail_expr(
                        arg_expr,
                        frames,
                        source_expr,
                        resume_path,
                        resume_slot,
                        allocate_synthetic_symbol_id,
                    )
                    .map(hir::CallArg::Positional),
                    hir::CallArg::Named {
                        name,
                        name_span,
                        value,
                    } => build_resume_tail_expr(
                        value,
                        frames,
                        source_expr,
                        resume_path,
                        resume_slot,
                        allocate_synthetic_symbol_id,
                    )
                    .map(|rewritten_value| hir::CallArg::Named {
                        name: name.clone(),
                        name_span: *name_span,
                        value: rewritten_value,
                    }),
                };
                let Some(rewritten_arg) = rebuilt else {
                    continue;
                };
                let mut rebuilt_args = args.clone();
                rebuilt_args[arg_index] = rewritten_arg;
                return Some(hir::Expr {
                    span: expr.span,
                    ty: expr.ty,
                    kind: hir::ExprKind::Perform {
                        effect_ty: *effect_ty,
                        op: op.clone(),
                        args: rebuilt_args,
                    },
                });
            }
            None
        }
        hir::ExprKind::Missing
        | hir::ExprKind::Literal(_)
        | hir::ExprKind::VarRef(_)
        | hir::ExprKind::UnresolvedIdent { .. }
        | hir::ExprKind::ClassLiteral(_)
        | hir::ExprKind::Block(_)
        | hir::ExprKind::Closure(_)
        | hir::ExprKind::Handle(_)
        | hir::ExprKind::Todo(_) => None,
    }
}

fn make_block_expr_with_original_span(original_expr: &hir::Expr, block: hir::Block) -> hir::Expr {
    hir::Expr {
        span: original_expr.span,
        ty: original_expr.ty,
        kind: hir::ExprKind::Block(block),
    }
}

fn make_block_expr(span: Span, ty: TypeId, block: hir::Block) -> hir::Expr {
    hir::Expr {
        span,
        ty,
        kind: hir::ExprKind::Block(block),
    }
}

fn make_local_var_expr(span: Span, ty: TypeId, id: hir::SymbolId, name: &str) -> hir::Expr {
    hir::Expr {
        span,
        ty,
        kind: hir::ExprKind::VarRef(hir::ValueRef::Local {
            id,
            name: name.to_string(),
            decl_span: span,
        }),
    }
}

fn make_bool_literal_expr(span: Span, ty: TypeId, value: bool) -> hir::Expr {
    hir::Expr {
        span,
        ty,
        kind: hir::ExprKind::Literal(hir::LiteralKind::Bool(value)),
    }
}

fn make_assign_stmt(span: Span, ty: TypeId, lhs: hir::Expr, rhs: hir::Expr) -> hir::Stmt {
    hir::Stmt {
        span,
        ty,
        kind: hir::StmtKind::Assign {
            lhs,
            eq_span: span,
            rhs,
        },
    }
}

// 对 `while` body 内部的 suspend source，resume 后必须先完成当前迭代尾部，
// 然后才回到原 loop 的后续迭代；不能重新从 cond 之前开始，也不能丢掉
// `break/continue` 对当前 loop 的控制流语义。
fn build_resume_tail_while_stmt(
    original_stmt: &hir::Stmt,
    cond: &hir::Expr,
    body: &hir::Block,
    current_iteration_tail: hir::Block,
    allocate_synthetic_symbol_id: &mut dyn FnMut() -> hir::SymbolId,
) -> hir::Stmt {
    let resume_first_id = allocate_synthetic_symbol_id();
    let resume_first_name = format!("__resume_loop_first{}", resume_first_id.as_u32());
    let bool_ty = cond.ty;

    let resume_first_decl = hir::Stmt {
        span: original_stmt.span,
        ty: original_stmt.ty,
        kind: hir::StmtKind::Val(hir::ValDecl {
            span: original_stmt.span,
            id: Some(resume_first_id),
            name: Some(resume_first_name.clone()),
            mutable: true,
            ty: bool_ty,
            init: Some(make_bool_literal_expr(original_stmt.span, bool_ty, true)),
        }),
    };

    let resume_first_var = make_local_var_expr(
        original_stmt.span,
        bool_ty,
        resume_first_id,
        &resume_first_name,
    );
    let loop_cond = hir::Expr {
        span: cond.span,
        ty: bool_ty,
        kind: hir::ExprKind::If {
            cond: Box::new(resume_first_var.clone()),
            then_branch: Box::new(make_bool_literal_expr(cond.span, bool_ty, true)),
            else_branch: Some(Box::new(cond.clone())),
        },
    };

    let clear_resume_first = make_assign_stmt(
        original_stmt.span,
        original_stmt.ty,
        resume_first_var.clone(),
        make_bool_literal_expr(original_stmt.span, bool_ty, false),
    );

    let mut first_iteration_stmts = vec![clear_resume_first];
    first_iteration_stmts.extend(current_iteration_tail.stmts);
    let first_iteration_block = hir::Block {
        span: current_iteration_tail.span,
        ty: body.ty,
        stmts: first_iteration_stmts,
    };

    let loop_body_if = hir::Expr {
        span: original_stmt.span,
        ty: body.ty,
        kind: hir::ExprKind::If {
            cond: Box::new(resume_first_var),
            then_branch: Box::new(make_block_expr(
                first_iteration_block.span,
                body.ty,
                first_iteration_block,
            )),
            else_branch: Some(Box::new(make_block_expr(body.span, body.ty, body.clone()))),
        },
    };
    let loop_body = hir::Block {
        span: body.span,
        ty: body.ty,
        stmts: vec![hir::Stmt {
            span: original_stmt.span,
            ty: body.ty,
            kind: hir::StmtKind::Expr(loop_body_if),
        }],
    };
    let resumed_loop = hir::Stmt {
        span: original_stmt.span,
        ty: original_stmt.ty,
        kind: hir::StmtKind::While {
            cond: loop_cond,
            body: loop_body,
        },
    };

    let wrapper_block = hir::Block {
        span: original_stmt.span,
        ty: original_stmt.ty,
        stmts: vec![resume_first_decl, resumed_loop],
    };
    hir::Stmt {
        span: original_stmt.span,
        ty: original_stmt.ty,
        kind: hir::StmtKind::Expr(make_block_expr(
            original_stmt.span,
            original_stmt.ty,
            wrapper_block,
        )),
    }
}

fn rewrite_state_terminator_with_resume_slot(
    terminator: &mut StateTerminator,
    source_expr: &hir::Expr,
    resume_path: &SuspendResumePath,
    resume_slot: &FrameSlot,
) {
    if let StateTerminator::Branch { condition, .. } = terminator {
        rewrite_branch_condition_with_resume_slot(condition, source_expr, resume_path, resume_slot);
    }
}

struct MaterializedWhenResumeRewrite {
    when_span: Span,
    when_index: usize,
    consumer_action_indices: Vec<usize>,
    rewrite_terminator: bool,
    replacement_expr: Option<hir::Expr>,
}

struct MaterializedWhenResumeInput<'a> {
    source_path: &'a SuspendSourcePath,
    source_expr: &'a hir::Expr,
    resume_path: &'a SuspendResumePath,
    resume_slot: &'a FrameSlot,
    allocate_synthetic_symbol_id: &'a mut dyn FnMut() -> hir::SymbolId,
}

fn prepare_materialized_when_resume_rewrite(
    actions: &[HandleStateOp],
    resume_after_index: usize,
    terminator: &StateTerminator,
    input: &mut MaterializedWhenResumeInput<'_>,
) -> Option<MaterializedWhenResumeRewrite> {
    let (when_frame_index, when_span) = input
        .source_path
        .frames
        .iter()
        .enumerate()
        .rev()
        .find_map(|(idx, op)| match op {
            SuspendSourceFramePath::WhenArm { when_span, .. } => Some((idx, *when_span)),
            _ => None,
        })?;

    let when_index = actions
        .iter()
        .enumerate()
        .skip(resume_after_index + 1)
        .find_map(|(idx, op)| match op {
            HandleStateOp::WhenExpr { expr } if expr.span == when_span => Some(idx),
            _ => None,
        })?;

    let consumer_action_indices = actions[when_index + 1..]
        .iter()
        .enumerate()
        .filter_map(|(offset, op)| {
            state_op_contains_expr_span(op, when_span).then_some(when_index + 1 + offset)
        })
        .collect::<Vec<_>>();
    let rewrite_terminator = state_terminator_contains_expr_span(terminator, when_span);

    if consumer_action_indices.is_empty() && !rewrite_terminator {
        return Some(MaterializedWhenResumeRewrite {
            when_span,
            when_index,
            consumer_action_indices,
            rewrite_terminator,
            replacement_expr: None,
        });
    }

    let HandleStateOp::WhenExpr { expr: when_expr } = &actions[when_index] else {
        return None;
    };
    let replacement_expr = build_resume_tail_expr(
        when_expr,
        &input.source_path.frames[when_frame_index..],
        input.source_expr,
        input.resume_path,
        input.resume_slot,
        input.allocate_synthetic_symbol_id,
    )?;

    debug_assert!(
        consumer_action_indices.len() + usize::from(rewrite_terminator) <= 1,
        "materialized when resume rewrite unexpectedly found multiple live consumers for span {:?}",
        when_span
    );

    Some(MaterializedWhenResumeRewrite {
        when_span,
        when_index,
        consumer_action_indices,
        rewrite_terminator,
        replacement_expr: Some(replacement_expr),
    })
}

fn resume_rewrite_candidate_spans(
    source_expr: &hir::Expr,
    resume_path: &SuspendResumePath,
) -> Vec<Span> {
    let mut spans = vec![source_expr.span];
    for frame in &resume_path.expr_frames {
        let span = frame.expr_span();
        if !spans.contains(&span) {
            spans.push(span);
        }
    }
    spans
}

fn suspend_site_kind_matches_source_path_expr_kind(
    site_kind: &SuspendSiteKind,
    expr_kind: &hir::ExprKind,
) -> bool {
    matches!(
        (site_kind, expr_kind),
        (
            SuspendSiteKind::Perform { .. },
            hir::ExprKind::Perform { .. }
        ) | (
            SuspendSiteKind::CallMaySuspend { .. }
                | SuspendSiteKind::CallStateMachineCallee { .. }
                | SuspendSiteKind::ClassCtorInit { .. },
            hir::ExprKind::Call { .. },
        ) | (
            SuspendSiteKind::NestedHandleBoundary { .. },
            hir::ExprKind::Handle(_),
        )
    )
}

fn suspend_site_kind_matches_resume_path_expr_kind(
    site_kind: &SuspendSiteKind,
    expr_kind: &hir::ExprKind,
) -> bool {
    matches!(
        (site_kind, expr_kind),
        (
            SuspendSiteKind::Perform { .. },
            hir::ExprKind::Perform { .. }
        ) | (
            SuspendSiteKind::CallMaySuspend { .. }
                | SuspendSiteKind::CallStateMachineCallee { .. }
                | SuspendSiteKind::ClassCtorInit { .. }
                | SuspendSiteKind::RuntimeRaise { .. },
            hir::ExprKind::Call { .. },
        ) | (
            SuspendSiteKind::NestedHandleBoundary { .. },
            hir::ExprKind::Handle(_),
        )
    )
}

fn state_contains_any_expr_span(state: &PlanState, candidate_spans: &[Span]) -> bool {
    candidate_spans.iter().copied().any(|expr_span| {
        state
            .actions
            .iter()
            .any(|op| state_op_contains_expr_span(op, expr_span))
            || state_terminator_contains_expr_span(&state.terminator, expr_span)
    })
}

fn state_op_contains_expr_span(op: &HandleStateOp, expr_span: Span) -> bool {
    match op {
        HandleStateOp::BindLocal { decl, .. } | HandleStateOp::DeclareAnonymousVal { decl, .. } => {
            decl.init
                .as_ref()
                .is_some_and(|init| expr_contains_span(init, expr_span))
        }
        HandleStateOp::Assign { stmt }
        | HandleStateOp::Return { stmt }
        | HandleStateOp::TodoStmt { stmt, .. }
        | HandleStateOp::StmtEmpty { stmt }
        | HandleStateOp::WhileCondHeader { stmt }
        | HandleStateOp::Break { stmt }
        | HandleStateOp::Continue { stmt } => stmt_contains_expr_span(stmt, expr_span),
        HandleStateOp::ExprMissing { expr }
        | HandleStateOp::Literal { expr }
        | HandleStateOp::ReadLocal { expr, .. }
        | HandleStateOp::ObjectInitAccessBoundary { expr, .. }
        | HandleStateOp::VarRef { expr }
        | HandleStateOp::StructLit { expr }
        | HandleStateOp::TupleLit { expr }
        | HandleStateOp::InterpolatedString { expr }
        | HandleStateOp::Expr { expr }
        | HandleStateOp::RuntimeRaiseBoundary { expr, .. }
        | HandleStateOp::BinaryExpr { expr }
        | HandleStateOp::WhenExpr { expr }
        | HandleStateOp::SuspendCall { expr, .. }
        | HandleStateOp::Call { expr }
        | HandleStateOp::Perform { expr, .. }
        | HandleStateOp::NestedHandleBoundary { expr, .. }
        | HandleStateOp::NestedHandle { expr, .. }
        | HandleStateOp::Closure { expr }
        | HandleStateOp::TodoExpr { expr, .. } => expr_contains_span(expr, expr_span),
        HandleStateOp::ResumeAfterSite { source_span, .. } => *source_span == expr_span,
        HandleStateOp::CleanupEdgeComplete
        | HandleStateOp::ReturnToEnclosingExpression
        | HandleStateOp::LoopReentry { .. }
        | HandleStateOp::ImplicitElseUnit { .. }
        | HandleStateOp::ExecuteArmBody { .. } => false,
    }
}

fn state_op_within_span(op: &HandleStateOp, container_span: Span) -> bool {
    let span_within_container =
        |span: Span| span.start >= container_span.start && span.end <= container_span.end;

    match op {
        HandleStateOp::BindLocal { decl, .. } | HandleStateOp::DeclareAnonymousVal { decl, .. } => {
            span_within_container(decl.span)
        }
        HandleStateOp::Assign { stmt }
        | HandleStateOp::Return { stmt }
        | HandleStateOp::TodoStmt { stmt, .. }
        | HandleStateOp::StmtEmpty { stmt }
        | HandleStateOp::WhileCondHeader { stmt }
        | HandleStateOp::Break { stmt }
        | HandleStateOp::Continue { stmt } => span_within_container(stmt.span),
        HandleStateOp::ExprMissing { expr }
        | HandleStateOp::Literal { expr }
        | HandleStateOp::ReadLocal { expr, .. }
        | HandleStateOp::ObjectInitAccessBoundary { expr, .. }
        | HandleStateOp::VarRef { expr }
        | HandleStateOp::StructLit { expr }
        | HandleStateOp::TupleLit { expr }
        | HandleStateOp::InterpolatedString { expr }
        | HandleStateOp::Expr { expr }
        | HandleStateOp::RuntimeRaiseBoundary { expr, .. }
        | HandleStateOp::BinaryExpr { expr }
        | HandleStateOp::WhenExpr { expr }
        | HandleStateOp::SuspendCall { expr, .. }
        | HandleStateOp::Call { expr }
        | HandleStateOp::Perform { expr, .. }
        | HandleStateOp::NestedHandleBoundary { expr, .. }
        | HandleStateOp::NestedHandle { expr, .. }
        | HandleStateOp::Closure { expr }
        | HandleStateOp::TodoExpr { expr, .. } => span_within_container(expr.span),
        HandleStateOp::ResumeAfterSite { source_span, .. } => span_within_container(*source_span),
        HandleStateOp::ImplicitElseUnit { span } => span_within_container(*span),
        HandleStateOp::ExecuteArmBody { arm, .. } => span_within_container(arm.span),
        HandleStateOp::CleanupEdgeComplete
        | HandleStateOp::ReturnToEnclosingExpression
        | HandleStateOp::LoopReentry { .. } => false,
    }
}

fn stmt_contains_expr_span(stmt: &hir::Stmt, expr_span: Span) -> bool {
    match &stmt.kind {
        hir::StmtKind::Expr(expr) => expr_contains_span(expr, expr_span),
        hir::StmtKind::Val(decl) => decl
            .init
            .as_ref()
            .is_some_and(|init| expr_contains_span(init, expr_span)),
        hir::StmtKind::Assign { lhs, rhs, .. } => {
            expr_contains_span(lhs, expr_span) || expr_contains_span(rhs, expr_span)
        }
        hir::StmtKind::While { cond, body } => {
            expr_contains_span(cond, expr_span)
                || body
                    .stmts
                    .iter()
                    .any(|stmt| stmt_contains_expr_span(stmt, expr_span))
        }
        hir::StmtKind::Return { value } => value
            .as_ref()
            .is_some_and(|expr| expr_contains_span(expr, expr_span)),
        hir::StmtKind::Empty
        | hir::StmtKind::Break { .. }
        | hir::StmtKind::Continue { .. }
        | hir::StmtKind::Todo(_) => false,
    }
}

fn expr_contains_span(expr: &hir::Expr, expr_span: Span) -> bool {
    if expr.span == expr_span {
        return true;
    }

    match &expr.kind {
        hir::ExprKind::Missing
        | hir::ExprKind::Literal(_)
        | hir::ExprKind::VarRef(_)
        | hir::ExprKind::UnresolvedIdent { .. }
        | hir::ExprKind::ClassLiteral(_)
        | hir::ExprKind::Closure(_)
        | hir::ExprKind::Todo(_) => false,
        hir::ExprKind::StructLit { fields, .. } => fields
            .iter()
            .any(|field| expr_contains_span(&field.value, expr_span)),
        hir::ExprKind::TupleLit { elements } => elements
            .iter()
            .any(|element| expr_contains_span(element, expr_span)),
        hir::ExprKind::InterpolatedString { parts, .. } => parts.iter().any(|part| {
            matches!(
                part,
                hir::InterpolatedStringPart::Expr { expr }
                    if expr_contains_span(expr, expr_span)
            )
        }),
        hir::ExprKind::Unary { expr: inner, .. }
        | hir::ExprKind::Cast { expr: inner, .. }
        | hir::ExprKind::TypeCheck { expr: inner, .. } => expr_contains_span(inner, expr_span),
        hir::ExprKind::Block(block) => block
            .stmts
            .iter()
            .any(|stmt| stmt_contains_expr_span(stmt, expr_span)),
        hir::ExprKind::Binary { lhs, rhs, .. } => {
            expr_contains_span(lhs, expr_span) || expr_contains_span(rhs, expr_span)
        }
        hir::ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => {
            expr_contains_span(cond, expr_span)
                || expr_contains_span(then_branch, expr_span)
                || else_branch
                    .as_deref()
                    .is_some_and(|else_branch| expr_contains_span(else_branch, expr_span))
        }
        hir::ExprKind::When { subject, arms } => {
            expr_contains_span(subject, expr_span)
                || arms.iter().any(|arm| {
                    arm.guard
                        .as_ref()
                        .is_some_and(|guard| expr_contains_span(guard, expr_span))
                        || expr_contains_span(&arm.body, expr_span)
                })
        }
        hir::ExprKind::MemberAccess { receiver, .. } => expr_contains_span(receiver, expr_span),
        hir::ExprKind::Call { callee, args } => {
            expr_contains_span(callee, expr_span)
                || args.iter().any(|arg| match arg {
                    hir::CallArg::Positional(arg_expr) => expr_contains_span(arg_expr, expr_span),
                    hir::CallArg::Named { value, .. } => expr_contains_span(value, expr_span),
                })
        }
        hir::ExprKind::Perform { args, .. } => args.iter().any(|arg| match arg {
            hir::CallArg::Positional(arg_expr) => expr_contains_span(arg_expr, expr_span),
            hir::CallArg::Named { value, .. } => expr_contains_span(value, expr_span),
        }),
        hir::ExprKind::Handle(handle) => {
            handle
                .body
                .stmts
                .iter()
                .any(|stmt| stmt_contains_expr_span(stmt, expr_span))
                || handle
                    .arms
                    .iter()
                    .any(|arm| expr_contains_span(&arm.body, expr_span))
                || handle.finally.as_ref().is_some_and(|finally_block| {
                    finally_block
                        .stmts
                        .iter()
                        .any(|stmt| stmt_contains_expr_span(stmt, expr_span))
                })
        }
    }
}

fn state_terminator_contains_expr_span(terminator: &StateTerminator, expr_span: Span) -> bool {
    match terminator {
        StateTerminator::Branch { condition, .. } => match condition {
            HandleBranchCondition::WhileCond { condition }
            | HandleBranchCondition::IfCond { condition } => {
                expr_contains_span(condition, expr_span)
            }
        },
        StateTerminator::Goto(_)
        | StateTerminator::Suspend { .. }
        | StateTerminator::ReturnHandle
        | StateTerminator::ReturnFromFunction
        | StateTerminator::CleanupEnter { .. }
        | StateTerminator::ArmExit(_) => false,
    }
}

fn rewrite_state_op_replacing_expr_span(
    op: &mut HandleStateOp,
    target_span: Span,
    replacement_expr: &hir::Expr,
) {
    match op {
        HandleStateOp::BindLocal { decl, .. } | HandleStateOp::DeclareAnonymousVal { decl, .. } => {
            if let Some(init) = decl.init.as_mut() {
                *init = rewrite_expr_replacing_span(init, target_span, replacement_expr);
            }
        }
        HandleStateOp::Assign { stmt }
        | HandleStateOp::Return { stmt }
        | HandleStateOp::TodoStmt { stmt, .. }
        | HandleStateOp::StmtEmpty { stmt }
        | HandleStateOp::WhileCondHeader { stmt }
        | HandleStateOp::Break { stmt }
        | HandleStateOp::Continue { stmt } => {
            rewrite_stmt_replacing_expr_span(stmt, target_span, replacement_expr);
        }
        HandleStateOp::ExprMissing { expr }
        | HandleStateOp::Literal { expr }
        | HandleStateOp::ReadLocal { expr, .. }
        | HandleStateOp::ObjectInitAccessBoundary { expr, .. }
        | HandleStateOp::VarRef { expr }
        | HandleStateOp::StructLit { expr }
        | HandleStateOp::TupleLit { expr }
        | HandleStateOp::InterpolatedString { expr }
        | HandleStateOp::Expr { expr }
        | HandleStateOp::RuntimeRaiseBoundary { expr, .. }
        | HandleStateOp::BinaryExpr { expr }
        | HandleStateOp::WhenExpr { expr }
        | HandleStateOp::SuspendCall { expr, .. }
        | HandleStateOp::Call { expr }
        | HandleStateOp::Perform { expr, .. }
        | HandleStateOp::NestedHandleBoundary { expr, .. }
        | HandleStateOp::NestedHandle { expr, .. }
        | HandleStateOp::Closure { expr }
        | HandleStateOp::TodoExpr { expr, .. } => {
            **expr = rewrite_expr_replacing_span(expr, target_span, replacement_expr);
        }
        HandleStateOp::ResumeAfterSite { .. }
        | HandleStateOp::CleanupEdgeComplete
        | HandleStateOp::ReturnToEnclosingExpression
        | HandleStateOp::LoopReentry { .. }
        | HandleStateOp::ImplicitElseUnit { .. }
        | HandleStateOp::ExecuteArmBody { .. } => {}
    }
}

fn rewrite_state_terminator_replacing_expr_span(
    terminator: &mut StateTerminator,
    target_span: Span,
    replacement_expr: &hir::Expr,
) {
    if let StateTerminator::Branch { condition, .. } = terminator {
        rewrite_branch_condition_replacing_expr_span(condition, target_span, replacement_expr);
    }
}

fn rewrite_stmt_replacing_expr_span(
    stmt: &mut hir::Stmt,
    target_span: Span,
    replacement_expr: &hir::Expr,
) {
    match &mut stmt.kind {
        hir::StmtKind::Expr(expr) => {
            *expr = rewrite_expr_replacing_span(expr, target_span, replacement_expr);
        }
        hir::StmtKind::Val(decl) => {
            if let Some(init) = decl.init.as_mut() {
                *init = rewrite_expr_replacing_span(init, target_span, replacement_expr);
            }
        }
        hir::StmtKind::Assign { lhs, rhs, .. } => {
            *lhs = rewrite_expr_replacing_span(lhs, target_span, replacement_expr);
            *rhs = rewrite_expr_replacing_span(rhs, target_span, replacement_expr);
        }
        hir::StmtKind::While { cond, body } => {
            *cond = rewrite_expr_replacing_span(cond, target_span, replacement_expr);
            for stmt in &mut body.stmts {
                rewrite_stmt_replacing_expr_span(stmt, target_span, replacement_expr);
            }
        }
        hir::StmtKind::Return { value } => {
            if let Some(expr) = value.as_mut() {
                *expr = rewrite_expr_replacing_span(expr, target_span, replacement_expr);
            }
        }
        hir::StmtKind::Empty
        | hir::StmtKind::Break { .. }
        | hir::StmtKind::Continue { .. }
        | hir::StmtKind::Todo(_) => {}
    }
}

fn rewrite_branch_condition_replacing_expr_span(
    condition: &mut HandleBranchCondition,
    target_span: Span,
    replacement_expr: &hir::Expr,
) {
    match condition {
        HandleBranchCondition::WhileCond { condition }
        | HandleBranchCondition::IfCond { condition } => {
            **condition = rewrite_expr_replacing_span(condition, target_span, replacement_expr);
        }
    }
}

fn rewrite_expr_replacing_span(
    expr: &hir::Expr,
    target_span: Span,
    replacement_expr: &hir::Expr,
) -> hir::Expr {
    if expr.span == target_span {
        return replacement_expr.clone();
    }

    let mut rewritten = expr.clone();
    match &mut rewritten.kind {
        hir::ExprKind::Missing
        | hir::ExprKind::Literal(_)
        | hir::ExprKind::VarRef(_)
        | hir::ExprKind::UnresolvedIdent { .. }
        | hir::ExprKind::ClassLiteral(_)
        | hir::ExprKind::Closure(_)
        | hir::ExprKind::Todo(_) => {}
        hir::ExprKind::StructLit { fields, .. } => {
            for field in fields {
                field.value =
                    rewrite_expr_replacing_span(&field.value, target_span, replacement_expr);
            }
        }
        hir::ExprKind::TupleLit { elements } => {
            for element in elements {
                *element = rewrite_expr_replacing_span(element, target_span, replacement_expr);
            }
        }
        hir::ExprKind::InterpolatedString { parts, .. } => {
            for part in parts {
                if let hir::InterpolatedStringPart::Expr { expr } = part {
                    *expr = rewrite_expr_replacing_span(expr, target_span, replacement_expr);
                }
            }
        }
        hir::ExprKind::Unary { expr: inner, .. }
        | hir::ExprKind::Cast { expr: inner, .. }
        | hir::ExprKind::TypeCheck { expr: inner, .. } => {
            **inner = rewrite_expr_replacing_span(inner, target_span, replacement_expr);
        }
        hir::ExprKind::Block(block) => {
            for stmt in &mut block.stmts {
                rewrite_stmt_replacing_expr_span(stmt, target_span, replacement_expr);
            }
        }
        hir::ExprKind::Binary { lhs, rhs, .. } => {
            **lhs = rewrite_expr_replacing_span(lhs, target_span, replacement_expr);
            **rhs = rewrite_expr_replacing_span(rhs, target_span, replacement_expr);
        }
        hir::ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => {
            **cond = rewrite_expr_replacing_span(cond, target_span, replacement_expr);
            **then_branch = rewrite_expr_replacing_span(then_branch, target_span, replacement_expr);
            if let Some(else_branch) = else_branch.as_mut() {
                **else_branch =
                    rewrite_expr_replacing_span(else_branch, target_span, replacement_expr);
            }
        }
        hir::ExprKind::When { subject, arms } => {
            **subject = rewrite_expr_replacing_span(subject, target_span, replacement_expr);
            for arm in arms {
                if let Some(guard) = arm.guard.as_mut() {
                    *guard = rewrite_expr_replacing_span(guard, target_span, replacement_expr);
                }
                arm.body = rewrite_expr_replacing_span(&arm.body, target_span, replacement_expr);
            }
        }
        hir::ExprKind::MemberAccess { receiver, .. } => {
            **receiver = rewrite_expr_replacing_span(receiver, target_span, replacement_expr);
        }
        hir::ExprKind::Call { callee, args } => {
            **callee = rewrite_expr_replacing_span(callee, target_span, replacement_expr);
            for arg in args {
                match arg {
                    hir::CallArg::Positional(arg_expr) => {
                        *arg_expr =
                            rewrite_expr_replacing_span(arg_expr, target_span, replacement_expr);
                    }
                    hir::CallArg::Named { value, .. } => {
                        *value = rewrite_expr_replacing_span(value, target_span, replacement_expr);
                    }
                }
            }
        }
        hir::ExprKind::Perform { args, .. } => {
            for arg in args {
                match arg {
                    hir::CallArg::Positional(arg_expr) => {
                        *arg_expr =
                            rewrite_expr_replacing_span(arg_expr, target_span, replacement_expr);
                    }
                    hir::CallArg::Named { value, .. } => {
                        *value = rewrite_expr_replacing_span(value, target_span, replacement_expr);
                    }
                }
            }
        }
        hir::ExprKind::Handle(handle) => {
            for stmt in &mut handle.body.stmts {
                rewrite_stmt_replacing_expr_span(stmt, target_span, replacement_expr);
            }
            for arm in &mut handle.arms {
                arm.body = rewrite_expr_replacing_span(&arm.body, target_span, replacement_expr);
            }
            if let Some(finally_block) = handle.finally.as_mut() {
                for stmt in &mut finally_block.stmts {
                    rewrite_stmt_replacing_expr_span(stmt, target_span, replacement_expr);
                }
            }
        }
    }

    rewritten
}

fn rewrite_stmt_with_resume_slot(
    stmt: &mut hir::Stmt,
    source_expr: &hir::Expr,
    resume_path: &SuspendResumePath,
    resume_slot: &FrameSlot,
) {
    match &mut stmt.kind {
        hir::StmtKind::Expr(expr) => {
            *expr = rewrite_expr_with_resume_slot(expr, source_expr, resume_path, resume_slot);
        }
        hir::StmtKind::Val(decl) => {
            if let Some(init) = decl.init.as_mut() {
                *init = rewrite_expr_with_resume_slot(init, source_expr, resume_path, resume_slot);
            }
        }
        hir::StmtKind::Assign { lhs, rhs, .. } => {
            *lhs = rewrite_expr_with_resume_slot(lhs, source_expr, resume_path, resume_slot);
            *rhs = rewrite_expr_with_resume_slot(rhs, source_expr, resume_path, resume_slot);
        }
        hir::StmtKind::While { cond, body } => {
            *cond = rewrite_expr_with_resume_slot(cond, source_expr, resume_path, resume_slot);
            for stmt in &mut body.stmts {
                rewrite_stmt_with_resume_slot(stmt, source_expr, resume_path, resume_slot);
            }
        }
        hir::StmtKind::Return { value } => {
            if let Some(expr) = value.as_mut() {
                *expr = rewrite_expr_with_resume_slot(expr, source_expr, resume_path, resume_slot);
            }
        }
        hir::StmtKind::Empty
        | hir::StmtKind::Break { .. }
        | hir::StmtKind::Continue { .. }
        | hir::StmtKind::Todo(_) => {}
    }
}

fn rewrite_branch_condition_with_resume_slot(
    condition: &mut HandleBranchCondition,
    source_expr: &hir::Expr,
    resume_path: &SuspendResumePath,
    resume_slot: &FrameSlot,
) {
    match condition {
        HandleBranchCondition::WhileCond { condition }
        | HandleBranchCondition::IfCond { condition } => {
            **condition =
                rewrite_expr_with_resume_slot(condition, source_expr, resume_path, resume_slot);
        }
    }
}

fn rewrite_expr_with_resume_slot(
    expr: &hir::Expr,
    source_expr: &hir::Expr,
    resume_path: &SuspendResumePath,
    resume_slot: &FrameSlot,
) -> hir::Expr {
    if expr.span == source_expr.span {
        return make_resume_slot_var_expr(source_expr, resume_slot);
    }

    for start in 0..resume_path.expr_frames.len() {
        if resume_path_frame_matches_expr(&resume_path.expr_frames[start], expr) {
            return rewrite_expr_from_resume_path(
                expr,
                source_expr,
                &resume_path.expr_frames[start..],
                resume_slot,
            );
        }
    }

    expr.clone()
}

fn rewrite_expr_from_resume_path(
    expr: &hir::Expr,
    source_expr: &hir::Expr,
    expr_frames: &[SuspendResumeExprFrame],
    resume_slot: &FrameSlot,
) -> hir::Expr {
    if expr.span == source_expr.span {
        return make_resume_slot_var_expr(source_expr, resume_slot);
    }
    let Some(frame) = expr_frames.first() else {
        return expr.clone();
    };

    let mut rewritten = expr.clone();
    match (frame, &mut rewritten.kind) {
        (SuspendResumeExprFrame::CallCallee { call_span }, hir::ExprKind::Call { callee, .. })
            if rewritten.span == *call_span =>
        {
            **callee =
                rewrite_expr_from_resume_path(callee, source_expr, &expr_frames[1..], resume_slot);
        }
        (
            SuspendResumeExprFrame::CallArg {
                call_span,
                arg_index,
            },
            hir::ExprKind::Call { args, .. },
        ) if rewritten.span == *call_span => {
            if let Some(hir::CallArg::Positional(arg_expr)) = args.get_mut(*arg_index) {
                *arg_expr = rewrite_expr_from_resume_path(
                    arg_expr,
                    source_expr,
                    &expr_frames[1..],
                    resume_slot,
                );
            }
        }
        (
            SuspendResumeExprFrame::NamedArgValue {
                call_span,
                arg_index,
                name_span,
            },
            hir::ExprKind::Call { args, .. },
        ) if rewritten.span == *call_span => {
            if let Some(hir::CallArg::Named {
                name_span: arg_name_span,
                value,
                ..
            }) = args.get_mut(*arg_index)
                && *arg_name_span == *name_span
            {
                *value = rewrite_expr_from_resume_path(
                    value,
                    source_expr,
                    &expr_frames[1..],
                    resume_slot,
                );
            }
        }
        (
            SuspendResumeExprFrame::PerformArg {
                perform_span,
                arg_index,
            },
            hir::ExprKind::Perform { args, .. },
        ) if rewritten.span == *perform_span => {
            if let Some(arg) = args.get_mut(*arg_index) {
                match arg {
                    hir::CallArg::Positional(arg_expr) => {
                        *arg_expr = rewrite_expr_from_resume_path(
                            arg_expr,
                            source_expr,
                            &expr_frames[1..],
                            resume_slot,
                        );
                    }
                    hir::CallArg::Named { value, .. } => {
                        *value = rewrite_expr_from_resume_path(
                            value,
                            source_expr,
                            &expr_frames[1..],
                            resume_slot,
                        );
                    }
                }
            }
        }
        (
            SuspendResumeExprFrame::MemberReceiver { access_span },
            hir::ExprKind::MemberAccess { receiver, .. },
        ) if rewritten.span == *access_span => {
            **receiver = rewrite_expr_from_resume_path(
                receiver,
                source_expr,
                &expr_frames[1..],
                resume_slot,
            );
        }
        (SuspendResumeExprFrame::BinaryLhs { binary_span }, hir::ExprKind::Binary { lhs, .. })
            if rewritten.span == *binary_span =>
        {
            **lhs = rewrite_expr_from_resume_path(lhs, source_expr, &expr_frames[1..], resume_slot);
        }
        (SuspendResumeExprFrame::BinaryRhs { binary_span }, hir::ExprKind::Binary { rhs, .. })
            if rewritten.span == *binary_span =>
        {
            **rhs = rewrite_expr_from_resume_path(rhs, source_expr, &expr_frames[1..], resume_slot);
        }
        (
            SuspendResumeExprFrame::StructField {
                struct_span,
                field_name,
            },
            hir::ExprKind::StructLit { fields, .. },
        ) if rewritten.span == *struct_span => {
            if let Some(field) = fields.iter_mut().find(|field| field.name == *field_name) {
                field.value = rewrite_expr_from_resume_path(
                    &field.value,
                    source_expr,
                    &expr_frames[1..],
                    resume_slot,
                );
            }
        }
        (
            SuspendResumeExprFrame::TupleElement {
                tuple_span,
                element_index,
            },
            hir::ExprKind::TupleLit { elements },
        ) if rewritten.span == *tuple_span => {
            if let Some(element) = elements.get_mut(*element_index) {
                *element = rewrite_expr_from_resume_path(
                    element,
                    source_expr,
                    &expr_frames[1..],
                    resume_slot,
                );
            }
        }
        (
            SuspendResumeExprFrame::InterpolatedExpr {
                string_span,
                part_index,
            },
            hir::ExprKind::InterpolatedString { parts, .. },
        ) if rewritten.span == *string_span => {
            if let Some(hir::InterpolatedStringPart::Expr { expr: part_expr }) =
                parts.get_mut(*part_index)
            {
                *part_expr = rewrite_expr_from_resume_path(
                    part_expr,
                    source_expr,
                    &expr_frames[1..],
                    resume_slot,
                );
            }
        }
        (
            SuspendResumeExprFrame::UnaryOperand { expr_span },
            hir::ExprKind::Unary { expr: inner, .. },
        ) if rewritten.span == *expr_span => {
            **inner =
                rewrite_expr_from_resume_path(inner, source_expr, &expr_frames[1..], resume_slot);
        }
        (
            SuspendResumeExprFrame::CastOperand { expr_span },
            hir::ExprKind::Cast { expr: inner, .. },
        ) if rewritten.span == *expr_span => {
            **inner =
                rewrite_expr_from_resume_path(inner, source_expr, &expr_frames[1..], resume_slot);
        }
        (
            SuspendResumeExprFrame::TypeCheckOperand { expr_span },
            hir::ExprKind::TypeCheck { expr: inner, .. },
        ) if rewritten.span == *expr_span => {
            **inner =
                rewrite_expr_from_resume_path(inner, source_expr, &expr_frames[1..], resume_slot);
        }
        (SuspendResumeExprFrame::IfCond { if_span }, hir::ExprKind::If { cond, .. })
            if rewritten.span == *if_span =>
        {
            **cond =
                rewrite_expr_from_resume_path(cond, source_expr, &expr_frames[1..], resume_slot);
        }
        (SuspendResumeExprFrame::IfThenExpr { if_span }, hir::ExprKind::If { then_branch, .. })
            if rewritten.span == *if_span =>
        {
            **then_branch = rewrite_expr_from_resume_path(
                then_branch,
                source_expr,
                &expr_frames[1..],
                resume_slot,
            );
        }
        (
            SuspendResumeExprFrame::IfElseExpr { if_span },
            hir::ExprKind::If {
                else_branch: Some(else_branch),
                ..
            },
        ) if rewritten.span == *if_span => {
            **else_branch = rewrite_expr_from_resume_path(
                else_branch,
                source_expr,
                &expr_frames[1..],
                resume_slot,
            );
        }
        (
            SuspendResumeExprFrame::WhenSubject { when_span },
            hir::ExprKind::When { subject, .. },
        ) if rewritten.span == *when_span => {
            **subject =
                rewrite_expr_from_resume_path(subject, source_expr, &expr_frames[1..], resume_slot);
        }
        (
            SuspendResumeExprFrame::WhenArmGuard {
                when_span,
                arm_index,
            },
            hir::ExprKind::When { arms, .. },
        ) if rewritten.span == *when_span => {
            if let Some(arm) = arms.get_mut(*arm_index)
                && let Some(guard) = arm.guard.as_mut()
            {
                *guard = rewrite_expr_from_resume_path(
                    guard,
                    source_expr,
                    &expr_frames[1..],
                    resume_slot,
                );
            }
        }
        (
            SuspendResumeExprFrame::WhenArmBody {
                when_span,
                arm_index,
            },
            hir::ExprKind::When { arms, .. },
        ) if rewritten.span == *when_span => {
            if let Some(arm) = arms.get_mut(*arm_index) {
                arm.body = rewrite_expr_from_resume_path(
                    &arm.body,
                    source_expr,
                    &expr_frames[1..],
                    resume_slot,
                );
            }
        }
        _ => {}
    }

    rewritten
}

fn resume_path_frame_matches_expr(frame: &SuspendResumeExprFrame, expr: &hir::Expr) -> bool {
    match (frame, &expr.kind) {
        (SuspendResumeExprFrame::CallCallee { call_span }, hir::ExprKind::Call { .. })
        | (SuspendResumeExprFrame::CallArg { call_span, .. }, hir::ExprKind::Call { .. })
        | (SuspendResumeExprFrame::NamedArgValue { call_span, .. }, hir::ExprKind::Call { .. }) => {
            expr.span == *call_span
        }
        (
            SuspendResumeExprFrame::PerformArg { perform_span, .. },
            hir::ExprKind::Perform { .. },
        ) => expr.span == *perform_span,
        (
            SuspendResumeExprFrame::MemberReceiver { access_span },
            hir::ExprKind::MemberAccess { .. },
        ) => expr.span == *access_span,
        (SuspendResumeExprFrame::BinaryLhs { binary_span }, hir::ExprKind::Binary { .. })
        | (SuspendResumeExprFrame::BinaryRhs { binary_span }, hir::ExprKind::Binary { .. }) => {
            expr.span == *binary_span
        }
        (
            SuspendResumeExprFrame::StructField { struct_span, .. },
            hir::ExprKind::StructLit { .. },
        ) => expr.span == *struct_span,
        (
            SuspendResumeExprFrame::TupleElement { tuple_span, .. },
            hir::ExprKind::TupleLit { .. },
        ) => expr.span == *tuple_span,
        (
            SuspendResumeExprFrame::InterpolatedExpr { string_span, .. },
            hir::ExprKind::InterpolatedString { .. },
        ) => expr.span == *string_span,
        (SuspendResumeExprFrame::UnaryOperand { expr_span }, hir::ExprKind::Unary { .. })
        | (SuspendResumeExprFrame::CastOperand { expr_span }, hir::ExprKind::Cast { .. })
        | (
            SuspendResumeExprFrame::TypeCheckOperand { expr_span },
            hir::ExprKind::TypeCheck { .. },
        ) => expr.span == *expr_span,
        (SuspendResumeExprFrame::IfCond { if_span }, hir::ExprKind::If { .. })
        | (SuspendResumeExprFrame::IfThenExpr { if_span }, hir::ExprKind::If { .. })
        | (SuspendResumeExprFrame::IfElseExpr { if_span }, hir::ExprKind::If { .. }) => {
            expr.span == *if_span
        }
        (SuspendResumeExprFrame::WhenSubject { when_span }, hir::ExprKind::When { .. })
        | (SuspendResumeExprFrame::WhenArmGuard { when_span, .. }, hir::ExprKind::When { .. })
        | (SuspendResumeExprFrame::WhenArmBody { when_span, .. }, hir::ExprKind::When { .. }) => {
            expr.span == *when_span
        }
        _ => false,
    }
}

fn make_resume_slot_var_expr(source_expr: &hir::Expr, resume_slot: &FrameSlot) -> hir::Expr {
    hir::Expr {
        span: source_expr.span,
        ty: source_expr.ty,
        kind: hir::ExprKind::VarRef(hir::ValueRef::Local {
            id: resume_slot.id,
            name: resume_slot.name.clone(),
            decl_span: source_expr.span,
        }),
    }
}

fn next_synthetic_symbol_seed(
    handle: &hir::HandleExpr,
    known_local_metadata: &HashMap<hir::SymbolId, KnownLocalMetadata>,
) -> u32 {
    let mut ids = known_local_metadata.keys().copied().collect::<HashSet<_>>();
    for stmt in &handle.body.stmts {
        collect_declared_local_ids_in_stmt(stmt, &mut ids);
        collect_used_locals_in_stmt_static(stmt, &mut ids);
    }

    for arm in &handle.arms {
        for binder in &arm.op.binders {
            ids.insert(binder.id);
        }
        match arm.kind {
            hir::HandleArmKind::NonResuming => {}
            hir::HandleArmKind::EscapeContinuation { continuation } => {
                ids.insert(continuation);
            }
        }
        collect_declared_local_ids_in_expr(&arm.body, &mut ids);
        collect_used_locals_in_expr_static(&arm.body, &mut ids);
    }

    if let Some(finally_block) = handle.finally.as_ref() {
        for stmt in &finally_block.stmts {
            collect_declared_local_ids_in_stmt(stmt, &mut ids);
            collect_used_locals_in_stmt_static(stmt, &mut ids);
        }
    }

    ids.into_iter()
        .map(hir::SymbolId::as_u32)
        .max()
        .unwrap_or(0)
        .saturating_add(1)
}

fn matching_arms(arms: &[ArmPlan], kind: &SuspendSiteKind) -> Vec<ArmPlanId> {
    match kind {
        SuspendSiteKind::Perform { op_fqn } => arms
            .iter()
            .filter(|arm| arm.op_fqn == *op_fqn)
            .map(|arm| arm.id)
            .collect(),
        SuspendSiteKind::RuntimeRaise { .. } => arms
            .iter()
            .filter(|arm| arm.op_fqn == "scoop.core.Raise.raise")
            .map(|arm| arm.id)
            .collect(),
        SuspendSiteKind::CallMaySuspend { .. }
        | SuspendSiteKind::CallStateMachineCallee { .. }
        | SuspendSiteKind::ObjectInitAccess { .. }
        | SuspendSiteKind::TopLevelValueInitAccess { .. }
        | SuspendSiteKind::ClassCtorInit { .. }
        | SuspendSiteKind::NestedHandleBoundary { .. } => Vec::new(),
    }
}

fn build_successor_map(states: &[PlanState]) -> HashMap<PlanStateId, Vec<PlanStateId>> {
    states
        .iter()
        .map(|state| {
            let succs = match &state.terminator {
                StateTerminator::Goto(next) => vec![*next],
                StateTerminator::Branch {
                    then_state,
                    else_state,
                    ..
                } => vec![*then_state, *else_state],
                StateTerminator::CleanupEnter { next_state, .. } => vec![*next_state],
                StateTerminator::Suspend { site_id } => {
                    let _ = site_id;
                    Vec::new()
                }
                StateTerminator::ReturnHandle
                | StateTerminator::ReturnFromFunction
                | StateTerminator::ArmExit(_) => Vec::new(),
            };
            (state.id, succs)
        })
        .collect()
}

fn reachable_states(
    start: PlanStateId,
    successors: &HashMap<PlanStateId, Vec<PlanStateId>>,
) -> HashSet<PlanStateId> {
    let mut seen = HashSet::new();
    let mut stack = vec![start];
    while let Some(state) = stack.pop() {
        if !seen.insert(state) {
            continue;
        }
        if let Some(nexts) = successors.get(&state) {
            stack.extend(nexts.iter().copied());
        }
    }
    seen
}

fn extract_tail_resume_payload_expr(
    expr: &hir::Expr,
    continuation_symbol: hir::SymbolId,
) -> Option<&hir::Expr> {
    let hir::ExprKind::Call { callee, args } = &expr.kind else {
        return None;
    };
    let hir::ExprKind::MemberAccess { receiver, member } = &callee.kind else {
        return None;
    };
    let hir::ExprKind::VarRef(hir::ValueRef::Local { id, .. }) = &receiver.kind else {
        return None;
    };
    if *id != continuation_symbol || member.name != "resume" {
        return None;
    }

    match args.as_slice() {
        [hir::CallArg::Positional(payload)] => Some(payload),
        [hir::CallArg::Named { value, .. }] => Some(value),
        _ => None,
    }
}

fn tail_resume_arm_matches_static(expr: &hir::Expr, continuation_symbol: hir::SymbolId) -> bool {
    if extract_tail_resume_payload_expr(expr, continuation_symbol).is_some() {
        return true;
    }

    match &expr.kind {
        hir::ExprKind::Block(block) => block
            .stmts
            .last()
            .is_some_and(|stmt| matches!(&stmt.kind, hir::StmtKind::Expr(expr) if tail_resume_arm_matches_static(expr, continuation_symbol))),
        hir::ExprKind::If {
            then_branch,
            else_branch,
            ..
        } => {
            tail_resume_arm_matches_static(then_branch, continuation_symbol)
                && else_branch
                    .as_deref()
                    .is_some_and(|expr| tail_resume_arm_matches_static(expr, continuation_symbol))
        }
        hir::ExprKind::When { arms, .. } => {
            !arms.is_empty()
                && arms
                    .iter()
                    .all(|arm| tail_resume_arm_matches_static(&arm.body, continuation_symbol))
        }
        _ => false,
    }
}

fn try_extract_callee_fqn(callee: &hir::Expr) -> Option<String> {
    match &callee.kind {
        hir::ExprKind::VarRef(hir::ValueRef::TopLevel { fqn, .. }) => Some(fqn.clone()),
        hir::ExprKind::MemberAccess { member, .. } => match member.resolved.as_ref()? {
            hir::MemberRef::Fun { fqn, .. } | hir::MemberRef::ExtensionFun { fqn, .. } => {
                Some(fqn.clone())
            }
            _ => None,
        },
        _ => None,
    }
}

fn resolve_plan_expr_concrete_type(
    context: &HandlePlanContext,
    types: &TypeStore,
    expr: &hir::Expr,
    known_local_metadata: &HashMap<hir::SymbolId, KnownLocalMetadata>,
) -> Option<TypeId> {
    ExprFactResolver::new(types, context.program_facts.as_ref(), |id| {
        known_local_metadata.get(&id).map(|metadata| metadata.ty)
    })
    .resolve_expr_concrete_type(expr)
}

fn collect_outer_scope_slots(
    handle: &hir::HandleExpr,
    known_local_metadata: &HashMap<hir::SymbolId, KnownLocalMetadata>,
) -> Vec<FrameSlot> {
    let mut declared = HashSet::new();
    for stmt in &handle.body.stmts {
        collect_declared_local_ids_in_stmt(stmt, &mut declared);
    }
    for arm in &handle.arms {
        for binder in &arm.op.binders {
            declared.insert(binder.id);
        }
        match arm.kind {
            hir::HandleArmKind::NonResuming => {}
            hir::HandleArmKind::EscapeContinuation { continuation } => {
                declared.insert(continuation);
            }
        }
        collect_declared_local_ids_in_expr(&arm.body, &mut declared);
    }
    if let Some(finally_block) = handle.finally.as_ref() {
        for stmt in &finally_block.stmts {
            collect_declared_local_ids_in_stmt(stmt, &mut declared);
        }
    }

    let mut used = HashMap::new();
    for stmt in &handle.body.stmts {
        collect_local_refs_in_stmt(stmt, &mut used);
    }
    for arm in &handle.arms {
        collect_local_refs_in_expr(&arm.body, &mut used);
    }
    if let Some(finally_block) = handle.finally.as_ref() {
        for stmt in &finally_block.stmts {
            collect_local_refs_in_stmt(stmt, &mut used);
        }
    }

    let mut slots = used
        .into_iter()
        .filter(|(id, _)| !declared.contains(id))
        .map(|(id, (name, ty))| {
            let metadata = known_local_metadata.get(&id).copied();
            FrameSlot {
                id,
                name,
                ty: metadata.map_or(ty, |meta| meta.ty),
                mutable: metadata.is_some_and(|meta| meta.mutable),
                seed_from_outer_scope: true,
                owner_arm: None,
            }
        })
        .collect::<Vec<_>>();
    slots.sort_by_key(|slot| slot.id.as_u32());
    slots
}

pub(crate) fn function_ty_declared_effectful(types: &TypeStore, ty: TypeId) -> bool {
    matches!(
        types.kind(ty),
        TypeKind::Ref(RefTypeKind::Function(fun_ty)) if !fun_ty.effects.is_pure()
    )
}

fn function_ty_may_suspend(types: &TypeStore, ty: TypeId) -> bool {
    function_ty_declared_effectful(types, ty)
}

pub(crate) fn hir_ty_is_function_value(types: &TypeStore, ty: TypeId) -> bool {
    matches!(types.kind(ty), TypeKind::Ref(RefTypeKind::Function(_)))
}

pub(crate) struct SuspendCallAnalysis<'a> {
    pub(crate) types: &'a TypeStore,
    pub(crate) context: &'a EffectAnalysisCtx,
}

impl<'a> SuspendCallAnalysis<'a> {
    fn call_site(&self, span: Span) -> hir::CallSite {
        self.context.call_site(span)
    }

    fn resolve_expr_concrete_type(&self, expr: &hir::Expr) -> Option<TypeId> {
        ExprFactResolver::new(self.types, self.context.program_facts.as_ref(), |id| {
            self.context
                .known_local_metadata
                .get(&id)
                .map(|metadata| metadata.ty)
        })
        .resolve_expr_concrete_type(expr)
    }

    fn block_may_suspend(
        &self,
        block: &hir::Block,
        seed_locals: &HashMap<hir::SymbolId, bool>,
    ) -> bool {
        let known_locals = self.solve_local_fun_effects_in_block(block, seed_locals);
        self.block_may_suspend_with_locals(block, &known_locals)
    }

    fn solve_local_fun_effects_in_block(
        &self,
        block: &hir::Block,
        seed_locals: &HashMap<hir::SymbolId, bool>,
    ) -> HashMap<hir::SymbolId, bool> {
        let mut known_locals = seed_locals.clone();
        loop {
            let before = known_locals.clone();
            self.collect_local_fun_effects_in_block(block, &mut known_locals);
            if known_locals == before {
                break;
            }
        }
        known_locals
    }

    fn collect_local_fun_effects_in_block(
        &self,
        block: &hir::Block,
        out: &mut HashMap<hir::SymbolId, bool>,
    ) {
        for stmt in &block.stmts {
            self.collect_local_fun_effects_in_stmt(stmt, out);
        }
    }

    fn collect_local_fun_effects_in_stmt(
        &self,
        stmt: &hir::Stmt,
        out: &mut HashMap<hir::SymbolId, bool>,
    ) {
        match &stmt.kind {
            hir::StmtKind::Empty
            | hir::StmtKind::Break { .. }
            | hir::StmtKind::Continue { .. }
            | hir::StmtKind::Todo(_) => {}
            hir::StmtKind::Expr(expr) => self.collect_local_fun_effects_in_expr(expr, out),
            hir::StmtKind::Val(decl) => {
                if let Some(init) = decl.init.as_ref() {
                    self.collect_local_fun_effects_in_expr(init, out);
                }
                if let Some(id) = decl.id
                    && hir_ty_is_function_value(self.types, decl.ty)
                {
                    let may_suspend = decl.init.as_ref().map_or_else(
                        || function_ty_declared_effectful(self.types, decl.ty),
                        |expr| self.function_value_may_suspend_when_called(expr, out),
                    );
                    out.insert(id, may_suspend);
                }
            }
            hir::StmtKind::Assign { lhs, rhs, .. } => {
                self.collect_local_fun_effects_in_expr(lhs, out);
                self.collect_local_fun_effects_in_expr(rhs, out);
                if let hir::ExprKind::VarRef(hir::ValueRef::Local { id, .. }) = &lhs.kind
                    && (hir_ty_is_function_value(self.types, lhs.ty)
                        || hir_ty_is_function_value(self.types, rhs.ty)
                        || out.contains_key(id))
                {
                    let may_suspend = self.function_value_may_suspend_when_called(rhs, out);
                    let entry = out.entry(*id).or_insert(false);
                    *entry |= may_suspend;
                }
            }
            hir::StmtKind::While { cond, body } => {
                self.collect_local_fun_effects_in_expr(cond, out);
                self.collect_local_fun_effects_in_block(body, out);
            }
            hir::StmtKind::Return { value } => {
                if let Some(expr) = value {
                    self.collect_local_fun_effects_in_expr(expr, out);
                }
            }
        }
    }

    fn collect_local_fun_effects_in_expr(
        &self,
        expr: &hir::Expr,
        out: &mut HashMap<hir::SymbolId, bool>,
    ) {
        match &expr.kind {
            hir::ExprKind::Missing
            | hir::ExprKind::Literal(_)
            | hir::ExprKind::VarRef(_)
            | hir::ExprKind::UnresolvedIdent { .. }
            | hir::ExprKind::ClassLiteral(_)
            | hir::ExprKind::Todo(_) => {}
            hir::ExprKind::Block(block) => self.collect_local_fun_effects_in_block(block, out),
            hir::ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => {
                self.collect_local_fun_effects_in_expr(cond, out);
                self.collect_local_fun_effects_in_expr(then_branch, out);
                if let Some(else_branch) = else_branch.as_deref() {
                    self.collect_local_fun_effects_in_expr(else_branch, out);
                }
            }
            hir::ExprKind::When { subject, arms } => {
                self.collect_local_fun_effects_in_expr(subject, out);
                for arm in arms {
                    if let Some(guard) = arm.guard.as_ref() {
                        self.collect_local_fun_effects_in_expr(guard, out);
                    }
                    self.collect_local_fun_effects_in_expr(&arm.body, out);
                }
            }
            hir::ExprKind::Call { callee, args } => {
                self.collect_local_fun_effects_in_expr(callee, out);
                for arg in args {
                    match arg {
                        hir::CallArg::Positional(expr) => {
                            self.collect_local_fun_effects_in_expr(expr, out)
                        }
                        hir::CallArg::Named { value, .. } => {
                            self.collect_local_fun_effects_in_expr(value, out)
                        }
                    }
                }
            }
            hir::ExprKind::StructLit { fields, .. } => {
                for field in fields {
                    self.collect_local_fun_effects_in_expr(&field.value, out);
                }
            }
            hir::ExprKind::TupleLit { elements } => {
                for element in elements {
                    self.collect_local_fun_effects_in_expr(element, out);
                }
            }
            hir::ExprKind::InterpolatedString { parts, .. } => {
                for part in parts {
                    if let hir::InterpolatedStringPart::Expr { expr } = part {
                        self.collect_local_fun_effects_in_expr(expr, out);
                    }
                }
            }
            hir::ExprKind::Unary { expr: inner, .. }
            | hir::ExprKind::Cast { expr: inner, .. }
            | hir::ExprKind::TypeCheck { expr: inner, .. }
            | hir::ExprKind::MemberAccess {
                receiver: inner, ..
            } => self.collect_local_fun_effects_in_expr(inner, out),
            hir::ExprKind::Binary { lhs, rhs, .. } => {
                self.collect_local_fun_effects_in_expr(lhs, out);
                self.collect_local_fun_effects_in_expr(rhs, out);
            }
            hir::ExprKind::Closure(closure) => {
                self.collect_local_fun_effects_in_expr(&closure.body, out);
            }
            hir::ExprKind::Perform { args, .. } => {
                for arg in args {
                    match arg {
                        hir::CallArg::Positional(expr) => {
                            self.collect_local_fun_effects_in_expr(expr, out)
                        }
                        hir::CallArg::Named { value, .. } => {
                            self.collect_local_fun_effects_in_expr(value, out)
                        }
                    }
                }
            }
            hir::ExprKind::Handle(handle) => {
                self.collect_local_fun_effects_in_block(&handle.body, out);
                for arm in &handle.arms {
                    self.collect_local_fun_effects_in_expr(&arm.body, out);
                }
                if let Some(finally_block) = &handle.finally {
                    self.collect_local_fun_effects_in_block(finally_block, out);
                }
            }
        }
    }

    fn block_may_suspend_with_locals(
        &self,
        block: &hir::Block,
        known_locals: &HashMap<hir::SymbolId, bool>,
    ) -> bool {
        block
            .stmts
            .iter()
            .any(|stmt| self.stmt_may_suspend(stmt, known_locals))
    }

    fn stmt_may_suspend(
        &self,
        stmt: &hir::Stmt,
        known_locals: &HashMap<hir::SymbolId, bool>,
    ) -> bool {
        match &stmt.kind {
            hir::StmtKind::Empty
            | hir::StmtKind::Break { .. }
            | hir::StmtKind::Continue { .. }
            | hir::StmtKind::Todo(_) => false,
            hir::StmtKind::Expr(expr) => self.expr_may_suspend(expr, known_locals),
            hir::StmtKind::Val(decl) => decl
                .init
                .as_ref()
                .is_some_and(|expr| self.expr_may_suspend(expr, known_locals)),
            hir::StmtKind::Assign { lhs, rhs, .. } => {
                self.expr_may_suspend(lhs, known_locals) || self.expr_may_suspend(rhs, known_locals)
            }
            hir::StmtKind::While { cond, body } => {
                self.expr_may_suspend(cond, known_locals)
                    || self.block_may_suspend(body, known_locals)
            }
            hir::StmtKind::Return { value } => value
                .as_ref()
                .is_some_and(|expr| self.expr_may_suspend(expr, known_locals)),
        }
    }

    fn expr_may_suspend(
        &self,
        expr: &hir::Expr,
        known_locals: &HashMap<hir::SymbolId, bool>,
    ) -> bool {
        match &expr.kind {
            hir::ExprKind::Missing
            | hir::ExprKind::Literal(_)
            | hir::ExprKind::UnresolvedIdent { .. }
            | hir::ExprKind::ClassLiteral(_)
            | hir::ExprKind::Closure(_)
            | hir::ExprKind::Todo(_) => false,
            hir::ExprKind::VarRef(hir::ValueRef::TopLevel { fqn, .. }) => {
                self.context.program_facts.object_value_fqns.contains(fqn)
                    || self
                        .context
                        .program_facts
                        .top_level_immutable_value_fqns
                        .contains(fqn)
            }
            hir::ExprKind::VarRef(hir::ValueRef::Local { .. }) => false,
            hir::ExprKind::StructLit { fields, .. } => fields
                .iter()
                .any(|field| self.expr_may_suspend(&field.value, known_locals)),
            hir::ExprKind::TupleLit { elements } => elements
                .iter()
                .any(|element| self.expr_may_suspend(element, known_locals)),
            hir::ExprKind::InterpolatedString { parts, .. } => parts.iter().any(|part| {
                matches!(
                    part,
                    hir::InterpolatedStringPart::Expr { expr }
                        if self.expr_may_suspend(expr, known_locals)
                )
            }),
            hir::ExprKind::Unary { expr: inner, .. }
            | hir::ExprKind::TypeCheck { expr: inner, .. } => {
                self.expr_may_suspend(inner, known_locals)
            }
            hir::ExprKind::Cast {
                expr: inner, op, ..
            } => matches!(op, ast::CastOp::As) || self.expr_may_suspend(inner, known_locals),
            hir::ExprKind::Block(block) => self.block_may_suspend(block, known_locals),
            hir::ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => {
                self.expr_may_suspend(cond, known_locals)
                    || self.expr_may_suspend(then_branch, known_locals)
                    || else_branch
                        .as_deref()
                        .is_some_and(|expr| self.expr_may_suspend(expr, known_locals))
            }
            hir::ExprKind::When { subject, arms } => {
                self.expr_may_suspend(subject, known_locals)
                    || arms.iter().any(|arm| {
                        arm.guard
                            .as_ref()
                            .is_some_and(|guard| self.expr_may_suspend(guard, known_locals))
                            || self.expr_may_suspend(&arm.body, known_locals)
                    })
            }
            hir::ExprKind::MemberAccess { receiver, member } => {
                self.expr_may_suspend(receiver, known_locals)
                    || matches!(
                        member.resolved.as_ref(),
                        Some(hir::MemberRef::Value { fqn, .. })
                            if self.context.program_facts.object_value_fqns.contains(fqn)
                                || self.context.program_facts.object_property_fqns.contains(fqn)
                    )
            }
            hir::ExprKind::Binary { lhs, rhs, .. } => {
                self.expr_may_suspend(lhs, known_locals) || self.expr_may_suspend(rhs, known_locals)
            }
            hir::ExprKind::Call { callee, args } => {
                self.context
                    .program_facts
                    .continuation_resume_call_sites
                    .contains(&self.call_site(expr.span))
                    || self
                        .context
                        .program_facts
                        .ctor_call_targets
                        .contains_key(&self.call_site(expr.span))
                    || self.function_value_may_suspend_when_called(callee, known_locals)
                    || self.expr_may_suspend(callee, known_locals)
                    || args.iter().any(|arg| match arg {
                        hir::CallArg::Positional(expr) => self.expr_may_suspend(expr, known_locals),
                        hir::CallArg::Named { value, .. } => {
                            self.expr_may_suspend(value, known_locals)
                        }
                    })
            }
            hir::ExprKind::Perform { .. } => true,
            hir::ExprKind::Handle(handle) => self.handle_may_suspend_outward(handle, known_locals),
        }
    }

    fn handle_may_suspend_outward(
        &self,
        handle: &hir::HandleExpr,
        known_locals: &HashMap<hir::SymbolId, bool>,
    ) -> bool {
        let mut known_local_metadata = self.context.known_local_metadata.clone();
        collect_known_local_metadata_in_handle(handle, &mut known_local_metadata);
        let context = HandlePlanContext::new(
            self.context.known_fun_effects.clone(),
            known_locals.clone(),
            known_local_metadata,
            self.context.current_source_path().to_path_buf(),
            Rc::clone(&self.context.program_facts),
        )
        .with_continuation_escape_facts(self.context.continuation_escape_facts().clone());

        HandleStateMachinePlan::build_with_context(self.types, handle, &context)
            .may_suspend_outward()
    }

    pub(crate) fn function_value_may_suspend_when_called(
        &self,
        expr: &hir::Expr,
        known_locals: &HashMap<hir::SymbolId, bool>,
    ) -> bool {
        let declared_effectful = self
            .resolve_expr_concrete_type(expr)
            .is_some_and(|ty| function_ty_declared_effectful(self.types, ty));
        match &expr.kind {
            hir::ExprKind::VarRef(hir::ValueRef::TopLevel { fqn, .. }) => self
                .context
                .known_fun_effects
                .get(fqn)
                .copied()
                .unwrap_or(declared_effectful),
            hir::ExprKind::VarRef(hir::ValueRef::Local { id, .. }) => {
                known_locals.get(id).copied().unwrap_or(declared_effectful)
            }
            hir::ExprKind::MemberAccess { member, .. } => match member.resolved.as_ref() {
                Some(hir::MemberRef::Fun { fqn, .. })
                | Some(hir::MemberRef::ExtensionFun { fqn, .. }) => self
                    .context
                    .known_fun_effects
                    .get(fqn)
                    .copied()
                    .unwrap_or(declared_effectful),
                _ => declared_effectful,
            },
            hir::ExprKind::Closure(closure) => {
                let mut seed_locals = known_locals.clone();
                for param in &closure.params {
                    seed_locals.insert(
                        param.id,
                        function_ty_declared_effectful(self.types, param.ty),
                    );
                }
                self.expr_may_suspend(&closure.body, &seed_locals)
            }
            hir::ExprKind::Block(block) => block
                .stmts
                .last()
                .and_then(|stmt| match &stmt.kind {
                    hir::StmtKind::Expr(expr) => Some(expr),
                    _ => None,
                })
                .is_some_and(|expr| {
                    self.function_value_may_suspend_when_called(expr, known_locals)
                }),
            hir::ExprKind::If {
                then_branch,
                else_branch,
                ..
            } => {
                self.function_value_may_suspend_when_called(then_branch, known_locals)
                    || else_branch.as_deref().is_some_and(|expr| {
                        self.function_value_may_suspend_when_called(expr, known_locals)
                    })
            }
            hir::ExprKind::When { arms, .. } => arms
                .iter()
                .any(|arm| self.function_value_may_suspend_when_called(&arm.body, known_locals)),
            _ => declared_effectful,
        }
    }
}

pub(crate) fn collect_known_fun_call_suspendability(
    types: &TypeStore,
    fun_index: &HashMap<String, &hir::FunDecl>,
    program_facts: Rc<ProgramFacts>,
    materialized_pass_view: Option<&crate::mir::MaterializedMirPassView<'_>>,
) -> HashMap<String, bool> {
    let mut pass_summary_effects = HashMap::new();
    let mut pass_controlled_fqns = HashSet::new();
    if let Some(pass_view) = materialized_pass_view {
        for family in pass_view.instances() {
            if !family.summary_is_overridden() {
                continue;
            }
            let may_outward_effect = family.summary().may_outward_effect;
            pass_summary_effects.insert(family.root_fqn().to_string(), may_outward_effect);
            pass_controlled_fqns.insert(family.root_fqn().to_string());
            for fqn in family.callable_fqns() {
                pass_summary_effects.insert(fqn.to_string(), may_outward_effect);
                pass_controlled_fqns.insert(fqn.to_string());
            }
        }
    }

    let mut known_fun_effects = fun_index
        .iter()
        .map(|(fqn, fun)| {
            (
                fqn.clone(),
                pass_summary_effects
                    .get(fqn.as_str())
                    .copied()
                    .unwrap_or_else(|| {
                        fun.body.is_none() && function_ty_declared_effectful(types, fun.ty)
                    }),
            )
        })
        .collect::<HashMap<_, _>>();

    loop {
        let snapshot = known_fun_effects.clone();
        let mut newly_effectful = Vec::new();
        let mut changed = false;
        for (fqn, fun) in fun_index {
            if known_fun_effects.get(fqn).copied().unwrap_or(false) {
                continue;
            }
            if pass_controlled_fqns.contains(fqn.as_str()) {
                continue;
            }
            let Some(body) = &fun.body else {
                continue;
            };
            let mut known_local_metadata = HashMap::new();
            collect_known_local_metadata_in_fun(fun, &mut known_local_metadata);
            let context = EffectAnalysisCtx::new(
                snapshot.clone(),
                HashMap::new(),
                known_local_metadata,
                fun.source_path.clone(),
                Rc::clone(&program_facts),
            )
            .with_continuation_escape_facts(
                ContinuationEscapeFacts::from_pass_view_for_callable(
                    materialized_pass_view,
                    Some(fqn.as_str()),
                    fun.source_path.as_path(),
                ),
            );
            let analysis = SuspendCallAnalysis {
                types,
                context: &context,
            };
            let seed_locals = fun
                .params
                .iter()
                .map(|param| (param.id, function_ty_declared_effectful(types, param.ty)))
                .collect::<HashMap<_, _>>();
            if analysis.block_may_suspend(body, &seed_locals) {
                newly_effectful.push(fqn.clone());
            }
        }
        if !newly_effectful.is_empty() {
            changed = true;
            for fqn in newly_effectful {
                known_fun_effects.insert(fqn, true);
            }
        }
        if !changed {
            break;
        }
    }

    known_fun_effects
}

#[cfg(test)]
fn collect_known_local_fun_call_suspendability_in_fun(
    fun: &hir::FunDecl,
    analysis: &SuspendCallAnalysis<'_>,
) -> HashMap<hir::SymbolId, bool> {
    let seed_locals = fun
        .params
        .iter()
        .map(|param| {
            (
                param.id,
                function_ty_declared_effectful(analysis.types, param.ty),
            )
        })
        .collect::<HashMap<_, _>>();
    fun.body
        .as_ref()
        .map(|body| analysis.solve_local_fun_effects_in_block(body, &seed_locals))
        .unwrap_or(seed_locals)
}

#[cfg(test)]
fn collect_effect_analysis_context_for_fun(
    lowered: &hir::LoweredHir,
    owner_fun: &hir::FunDecl,
) -> EffectAnalysisCtx {
    collect_effect_analysis_context_for_fun_with_pass_view(lowered, owner_fun, None)
}

#[cfg(test)]
fn collect_effect_analysis_context_for_fun_with_pass_view(
    lowered: &hir::LoweredHir,
    owner_fun: &hir::FunDecl,
    materialized_pass_view: Option<&crate::mir::MaterializedMirPassView<'_>>,
) -> EffectAnalysisCtx {
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

    let program_facts = Rc::new(ProgramFacts::from_lowered(lowered));
    let known_fun_effects = collect_known_fun_call_suspendability(
        &lowered.types,
        &fun_index,
        Rc::clone(&program_facts),
        materialized_pass_view,
    );

    let mut known_local_metadata = HashMap::new();
    collect_known_local_metadata_in_fun(owner_fun, &mut known_local_metadata);
    let continuation_escape_facts = ContinuationEscapeFacts::from_pass_view_for_callable(
        materialized_pass_view,
        Some(owner_fun.fqn.as_str()),
        owner_fun.source_path.as_path(),
    );
    let analysis_seed = EffectAnalysisCtx::new(
        known_fun_effects.clone(),
        HashMap::new(),
        known_local_metadata.clone(),
        owner_fun.source_path.clone(),
        Rc::clone(&program_facts),
    )
    .with_continuation_escape_facts(continuation_escape_facts.clone());
    let analysis = SuspendCallAnalysis {
        types: &lowered.types,
        context: &analysis_seed,
    };
    let known_local_fun_effects =
        collect_known_local_fun_call_suspendability_in_fun(owner_fun, &analysis);

    EffectAnalysisCtx::new(
        known_fun_effects,
        known_local_fun_effects,
        known_local_metadata,
        owner_fun.source_path.clone(),
        program_facts,
    )
    .with_continuation_escape_facts(continuation_escape_facts)
}

fn collect_declared_local_ids_in_stmt(stmt: &hir::Stmt, out: &mut HashSet<hir::SymbolId>) {
    match &stmt.kind {
        hir::StmtKind::Val(decl) => {
            if let Some(id) = decl.id {
                out.insert(id);
            }
            if let Some(init) = decl.init.as_ref() {
                collect_declared_local_ids_in_expr(init, out);
            }
        }
        hir::StmtKind::Expr(expr) => collect_declared_local_ids_in_expr(expr, out),
        hir::StmtKind::Assign { lhs, rhs, .. } => {
            collect_declared_local_ids_in_expr(lhs, out);
            collect_declared_local_ids_in_expr(rhs, out);
        }
        hir::StmtKind::While { cond, body } => {
            collect_declared_local_ids_in_expr(cond, out);
            for stmt in &body.stmts {
                collect_declared_local_ids_in_stmt(stmt, out);
            }
        }
        hir::StmtKind::Return { value } => {
            if let Some(expr) = value {
                collect_declared_local_ids_in_expr(expr, out);
            }
        }
        hir::StmtKind::Empty
        | hir::StmtKind::Break { .. }
        | hir::StmtKind::Continue { .. }
        | hir::StmtKind::Todo(_) => {}
    }
}

fn collect_declared_local_ids_in_expr(expr: &hir::Expr, out: &mut HashSet<hir::SymbolId>) {
    match &expr.kind {
        hir::ExprKind::Block(block) => {
            for stmt in &block.stmts {
                collect_declared_local_ids_in_stmt(stmt, out);
            }
        }
        hir::ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => {
            collect_declared_local_ids_in_expr(cond, out);
            collect_declared_local_ids_in_expr(then_branch, out);
            if let Some(else_branch) = else_branch.as_deref() {
                collect_declared_local_ids_in_expr(else_branch, out);
            }
        }
        hir::ExprKind::When { subject, arms } => {
            collect_declared_local_ids_in_expr(subject, out);
            for arm in arms {
                collect_declared_local_ids_in_when_pat(&arm.pat, out);
                if let Some(guard) = arm.guard.as_ref() {
                    collect_declared_local_ids_in_expr(guard, out);
                }
                collect_declared_local_ids_in_expr(&arm.body, out);
            }
        }
        hir::ExprKind::StructLit { fields, .. } => {
            for field in fields {
                collect_declared_local_ids_in_expr(&field.value, out);
            }
        }
        hir::ExprKind::TupleLit { elements } => {
            for element in elements {
                collect_declared_local_ids_in_expr(element, out);
            }
        }
        hir::ExprKind::InterpolatedString { parts, .. } => {
            for part in parts {
                if let hir::InterpolatedStringPart::Expr { expr } = part {
                    collect_declared_local_ids_in_expr(expr, out);
                }
            }
        }
        hir::ExprKind::Unary { expr: inner, .. }
        | hir::ExprKind::Cast { expr: inner, .. }
        | hir::ExprKind::TypeCheck { expr: inner, .. }
        | hir::ExprKind::MemberAccess {
            receiver: inner, ..
        } => collect_declared_local_ids_in_expr(inner, out),
        hir::ExprKind::Binary { lhs, rhs, .. } => {
            collect_declared_local_ids_in_expr(lhs, out);
            collect_declared_local_ids_in_expr(rhs, out);
        }
        hir::ExprKind::Call { callee, args } => {
            collect_declared_local_ids_in_expr(callee, out);
            for arg in args {
                match arg {
                    hir::CallArg::Positional(expr) => collect_declared_local_ids_in_expr(expr, out),
                    hir::CallArg::Named { value, .. } => {
                        collect_declared_local_ids_in_expr(value, out)
                    }
                }
            }
        }
        hir::ExprKind::Closure(closure) => {
            collect_declared_local_ids_in_closure(closure, out);
        }
        hir::ExprKind::Handle(handle) => {
            for stmt in &handle.body.stmts {
                collect_declared_local_ids_in_stmt(stmt, out);
            }
            for arm in &handle.arms {
                for binder in &arm.op.binders {
                    out.insert(binder.id);
                }
                match arm.kind {
                    hir::HandleArmKind::NonResuming => {}
                    hir::HandleArmKind::EscapeContinuation { continuation } => {
                        out.insert(continuation);
                    }
                }
                collect_declared_local_ids_in_expr(&arm.body, out);
            }
            if let Some(finally) = handle.finally.as_ref() {
                for stmt in &finally.stmts {
                    collect_declared_local_ids_in_stmt(stmt, out);
                }
            }
        }
        hir::ExprKind::Perform { args, .. } => {
            for arg in args {
                match arg {
                    hir::CallArg::Positional(expr) => collect_declared_local_ids_in_expr(expr, out),
                    hir::CallArg::Named { value, .. } => {
                        collect_declared_local_ids_in_expr(value, out)
                    }
                }
            }
        }
        hir::ExprKind::Missing
        | hir::ExprKind::Literal(_)
        | hir::ExprKind::VarRef(_)
        | hir::ExprKind::UnresolvedIdent { .. }
        | hir::ExprKind::ClassLiteral(_)
        | hir::ExprKind::Todo(_) => {}
    }
}

fn collect_declared_local_ids_in_closure(
    closure: &hir::ClosureExpr,
    out: &mut HashSet<hir::SymbolId>,
) {
    for param in &closure.params {
        out.insert(param.id);
    }

    // Resolver always introduces the implicit single-argument lambda binder
    // `it` at a synthetic zero-width span anchored to the lambda start.
    // When outer-scope slot collection walks into the closure body, treat that
    // synthetic binder like any other declared local so enclosing handle/try
    // frames do not attempt to seed it from the outer env.
    if closure.params.is_empty() {
        let implicit_it_decl_span = crate::span::Span::new(closure.span.start, closure.span.start);
        for capture in &closure.captures {
            if capture.name == "it" && capture.decl_span == implicit_it_decl_span {
                out.insert(capture.id);
            }
        }
    }

    collect_declared_local_ids_in_expr(&closure.body, out);
}

fn collect_declared_local_ids_in_when_pat(pat: &hir::WhenPat, out: &mut HashSet<hir::SymbolId>) {
    match pat {
        hir::WhenPat::Or { pats, .. } => {
            for pat in pats {
                collect_declared_local_ids_in_when_pat(pat, out);
            }
        }
        hir::WhenPat::Bind { id, .. } => {
            out.insert(*id);
        }
        hir::WhenPat::Tuple { elements, .. } => {
            for pat in elements {
                collect_declared_local_ids_in_when_pat(pat, out);
            }
        }
        hir::WhenPat::Variant { args, .. } => {
            for pat in args {
                collect_declared_local_ids_in_when_pat(pat, out);
            }
        }
        hir::WhenPat::Else { .. }
        | hir::WhenPat::Wildcard { .. }
        | hir::WhenPat::Rest { .. }
        | hir::WhenPat::Is { .. }
        | hir::WhenPat::IntLit { .. }
        | hir::WhenPat::CharLit { .. }
        | hir::WhenPat::StringLit { .. }
        | hir::WhenPat::BoolLit { .. } => {}
    }
}

fn collect_local_refs_in_stmt(
    stmt: &hir::Stmt,
    out: &mut HashMap<hir::SymbolId, (String, TypeId)>,
) {
    match &stmt.kind {
        hir::StmtKind::Expr(expr) => collect_local_refs_in_expr(expr, out),
        hir::StmtKind::Val(decl) => {
            if let Some(init) = decl.init.as_ref() {
                collect_local_refs_in_expr(init, out);
            }
        }
        hir::StmtKind::Assign { lhs, rhs, .. } => {
            collect_local_refs_in_expr(lhs, out);
            collect_local_refs_in_expr(rhs, out);
        }
        hir::StmtKind::While { cond, body } => {
            collect_local_refs_in_expr(cond, out);
            for stmt in &body.stmts {
                collect_local_refs_in_stmt(stmt, out);
            }
        }
        hir::StmtKind::Return { value } => {
            if let Some(expr) = value {
                collect_local_refs_in_expr(expr, out);
            }
        }
        hir::StmtKind::Empty
        | hir::StmtKind::Break { .. }
        | hir::StmtKind::Continue { .. }
        | hir::StmtKind::Todo(_) => {}
    }
}

fn collect_local_refs_in_expr(
    expr: &hir::Expr,
    out: &mut HashMap<hir::SymbolId, (String, TypeId)>,
) {
    match &expr.kind {
        hir::ExprKind::VarRef(hir::ValueRef::Local { id, name, .. }) => {
            out.entry(*id).or_insert_with(|| (name.clone(), expr.ty));
        }
        hir::ExprKind::Block(block) => {
            for stmt in &block.stmts {
                collect_local_refs_in_stmt(stmt, out);
            }
        }
        hir::ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => {
            collect_local_refs_in_expr(cond, out);
            collect_local_refs_in_expr(then_branch, out);
            if let Some(else_branch) = else_branch.as_deref() {
                collect_local_refs_in_expr(else_branch, out);
            }
        }
        hir::ExprKind::When { subject, arms } => {
            collect_local_refs_in_expr(subject, out);
            for arm in arms {
                if let Some(guard) = arm.guard.as_ref() {
                    collect_local_refs_in_expr(guard, out);
                }
                collect_local_refs_in_expr(&arm.body, out);
            }
        }
        hir::ExprKind::Call { callee, args } => {
            collect_local_refs_in_expr(callee, out);
            for arg in args {
                match arg {
                    hir::CallArg::Positional(expr) => collect_local_refs_in_expr(expr, out),
                    hir::CallArg::Named { value, .. } => collect_local_refs_in_expr(value, out),
                }
            }
        }
        hir::ExprKind::StructLit { fields, .. } => {
            for field in fields {
                collect_local_refs_in_expr(&field.value, out);
            }
        }
        hir::ExprKind::TupleLit { elements } => {
            for element in elements {
                collect_local_refs_in_expr(element, out);
            }
        }
        hir::ExprKind::InterpolatedString { parts, .. } => {
            for part in parts {
                if let hir::InterpolatedStringPart::Expr { expr } = part {
                    collect_local_refs_in_expr(expr, out);
                }
            }
        }
        hir::ExprKind::Unary { expr: inner, .. }
        | hir::ExprKind::Cast { expr: inner, .. }
        | hir::ExprKind::TypeCheck { expr: inner, .. }
        | hir::ExprKind::MemberAccess {
            receiver: inner, ..
        } => collect_local_refs_in_expr(inner, out),
        hir::ExprKind::Binary { lhs, rhs, .. } => {
            collect_local_refs_in_expr(lhs, out);
            collect_local_refs_in_expr(rhs, out);
        }
        hir::ExprKind::Closure(closure) => {
            collect_local_refs_in_expr(&closure.body, out);
        }
        hir::ExprKind::Perform { args, .. } => {
            for arg in args {
                match arg {
                    hir::CallArg::Positional(expr) => collect_local_refs_in_expr(expr, out),
                    hir::CallArg::Named { value, .. } => collect_local_refs_in_expr(value, out),
                }
            }
        }
        hir::ExprKind::Handle(handle) => {
            for stmt in &handle.body.stmts {
                collect_local_refs_in_stmt(stmt, out);
            }
            for arm in &handle.arms {
                collect_local_refs_in_expr(&arm.body, out);
            }
            if let Some(finally) = handle.finally.as_ref() {
                for stmt in &finally.stmts {
                    collect_local_refs_in_stmt(stmt, out);
                }
            }
        }
        hir::ExprKind::Missing
        | hir::ExprKind::Literal(_)
        | hir::ExprKind::VarRef(_)
        | hir::ExprKind::UnresolvedIdent { .. }
        | hir::ExprKind::ClassLiteral(_)
        | hir::ExprKind::Todo(_) => {}
    }
}

fn collect_used_locals_in_block_static(block: &hir::Block, out: &mut HashSet<hir::SymbolId>) {
    for stmt in &block.stmts {
        collect_used_locals_in_stmt_static(stmt, out);
    }
}

fn collect_used_locals_in_call_args_static(
    args: &[hir::CallArg],
    out: &mut HashSet<hir::SymbolId>,
) {
    for arg in args {
        match arg {
            hir::CallArg::Positional(expr) => collect_used_locals_in_expr_static(expr, out),
            hir::CallArg::Named { value, .. } => collect_used_locals_in_expr_static(value, out),
        }
    }
}

fn collect_used_locals_in_handle_static(
    handle: &hir::HandleExpr,
    out: &mut HashSet<hir::SymbolId>,
) {
    collect_used_locals_in_block_static(&handle.body, out);
    for arm in &handle.arms {
        collect_used_locals_in_expr_static(&arm.body, out);
    }
    if let Some(finally) = &handle.finally {
        collect_used_locals_in_block_static(finally, out);
    }
}

fn collect_used_locals_in_stmt_static(stmt: &hir::Stmt, out: &mut HashSet<hir::SymbolId>) {
    match &stmt.kind {
        hir::StmtKind::Empty
        | hir::StmtKind::Break { .. }
        | hir::StmtKind::Continue { .. }
        | hir::StmtKind::Todo(_) => {}
        hir::StmtKind::Expr(expr) => collect_used_locals_in_expr_static(expr, out),
        hir::StmtKind::Val(decl) => {
            if let Some(init) = &decl.init {
                collect_used_locals_in_expr_static(init, out);
            }
        }
        hir::StmtKind::Assign { lhs, rhs, .. } => {
            collect_used_locals_in_expr_static(lhs, out);
            collect_used_locals_in_expr_static(rhs, out);
        }
        hir::StmtKind::Return { value } => {
            if let Some(expr) = value {
                collect_used_locals_in_expr_static(expr, out);
            }
        }
        hir::StmtKind::While { cond, body } => {
            collect_used_locals_in_expr_static(cond, out);
            collect_used_locals_in_block_static(body, out);
        }
    }
}

fn collect_used_locals_in_expr_static(expr: &hir::Expr, out: &mut HashSet<hir::SymbolId>) {
    match &expr.kind {
        hir::ExprKind::Missing
        | hir::ExprKind::Literal(_)
        | hir::ExprKind::UnresolvedIdent { .. }
        | hir::ExprKind::ClassLiteral(_)
        | hir::ExprKind::Todo(_) => {}
        hir::ExprKind::VarRef(hir::ValueRef::Local { id, .. }) => {
            out.insert(*id);
        }
        hir::ExprKind::VarRef(hir::ValueRef::TopLevel { .. }) => {}
        hir::ExprKind::Call { callee, args } => {
            collect_used_locals_in_expr_static(callee, out);
            collect_used_locals_in_call_args_static(args, out);
        }
        hir::ExprKind::Perform { args, .. } => {
            collect_used_locals_in_call_args_static(args, out);
        }
        hir::ExprKind::Block(block) => {
            collect_used_locals_in_block_static(block, out);
        }
        hir::ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => {
            collect_used_locals_in_expr_static(cond, out);
            collect_used_locals_in_expr_static(then_branch, out);
            if let Some(else_branch) = else_branch {
                collect_used_locals_in_expr_static(else_branch, out);
            }
        }
        hir::ExprKind::Binary { lhs, rhs, .. } => {
            collect_used_locals_in_expr_static(lhs, out);
            collect_used_locals_in_expr_static(rhs, out);
        }
        hir::ExprKind::Unary { expr: inner, .. }
        | hir::ExprKind::Cast { expr: inner, .. }
        | hir::ExprKind::TypeCheck { expr: inner, .. }
        | hir::ExprKind::MemberAccess {
            receiver: inner, ..
        } => {
            collect_used_locals_in_expr_static(inner, out);
        }
        hir::ExprKind::InterpolatedString { parts, .. } => {
            for part in parts {
                if let hir::InterpolatedStringPart::Expr { expr } = part {
                    collect_used_locals_in_expr_static(expr, out);
                }
            }
        }
        hir::ExprKind::StructLit { fields, .. } => {
            for field in fields {
                collect_used_locals_in_expr_static(&field.value, out);
            }
        }
        hir::ExprKind::TupleLit { elements } => {
            for element in elements {
                collect_used_locals_in_expr_static(element, out);
            }
        }
        hir::ExprKind::When { subject, arms } => {
            collect_used_locals_in_expr_static(subject, out);
            for arm in arms {
                if let Some(guard) = &arm.guard {
                    collect_used_locals_in_expr_static(guard, out);
                }
                collect_used_locals_in_expr_static(&arm.body, out);
            }
        }
        hir::ExprKind::Closure(closure) => {
            for capture in &closure.captures {
                out.insert(capture.id);
            }
            collect_used_locals_in_expr_static(&closure.body, out);
        }
        hir::ExprKind::Handle(handle) => {
            collect_used_locals_in_handle_static(handle, out);
        }
    }
}

#[cfg(test)]
fn render_symbol_list(ids: &[hir::SymbolId], slots: &HashMap<hir::SymbolId, FrameSlot>) -> String {
    let mut labels = ids
        .iter()
        .map(|id| {
            slots.get(id).map_or_else(
                || format!("unknown#{}", id.as_u32()),
                FrameSlot::display_name,
            )
        })
        .collect::<Vec<_>>();
    labels.sort();
    labels.join(", ")
}

#[cfg(test)]
fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

fn expr_payload_signature(expr: &hir::Expr) -> usize {
    expr.span.start
        ^ (expr.span.end << 1)
        ^ ((expr.ty.as_u32() as usize) << 2)
        ^ expr_kind_signature(&expr.kind)
}

fn expr_kind_signature(kind: &hir::ExprKind) -> usize {
    match kind {
        hir::ExprKind::Missing => 1,
        hir::ExprKind::Literal(_) => 2,
        hir::ExprKind::VarRef(_) => 3,
        hir::ExprKind::UnresolvedIdent { .. } => 4,
        hir::ExprKind::StructLit { .. } => 5,
        hir::ExprKind::TupleLit { .. } => 6,
        hir::ExprKind::InterpolatedString { .. } => 7,
        hir::ExprKind::Unary { .. } => 8,
        hir::ExprKind::Binary { .. } => 9,
        hir::ExprKind::TypeCheck { .. } => 10,
        hir::ExprKind::Cast { .. } => 11,
        hir::ExprKind::Block(_) => 12,
        hir::ExprKind::Closure(_) => 13,
        hir::ExprKind::If { .. } => 14,
        hir::ExprKind::When { .. } => 15,
        hir::ExprKind::MemberAccess { .. } => 16,
        hir::ExprKind::Call { .. } => 17,
        hir::ExprKind::Perform { .. } => 18,
        hir::ExprKind::Handle(_) => 19,
        hir::ExprKind::ClassLiteral(_) => 20,
        hir::ExprKind::Todo(_) => 21,
    }
}

fn stmt_payload_signature(stmt: &hir::Stmt) -> usize {
    stmt.span.start
        ^ (stmt.span.end << 1)
        ^ ((stmt.ty.as_u32() as usize) << 2)
        ^ stmt_kind_signature(&stmt.kind)
}

fn stmt_kind_signature(kind: &hir::StmtKind) -> usize {
    match kind {
        hir::StmtKind::Empty => 1,
        hir::StmtKind::Expr(_) => 2,
        hir::StmtKind::Val(_) => 3,
        hir::StmtKind::Assign { .. } => 4,
        hir::StmtKind::While { .. } => 5,
        hir::StmtKind::Break { .. } => 6,
        hir::StmtKind::Continue { .. } => 7,
        hir::StmtKind::Return { .. } => 8,
        hir::StmtKind::Todo(_) => 9,
    }
}

fn decl_payload_signature(decl: &hir::ValDecl) -> usize {
    decl.span.start
        ^ (decl.span.end << 1)
        ^ decl.id.map(|id| id.as_u32() as usize).unwrap_or(0)
        ^ ((decl.ty.as_u32() as usize) << 2)
        ^ ((usize::from(decl.mutable)) << 3)
}

fn handle_arm_payload_signature(arm: &hir::HandleArm) -> usize {
    arm.span.start
        ^ (arm.span.end << 1)
        ^ arm.op.op.fqn.len()
        ^ handle_arm_kind_signature(arm.kind)
        ^ expr_payload_signature(&arm.body)
}

fn handle_arm_kind_signature(kind: hir::HandleArmKind) -> usize {
    match kind {
        hir::HandleArmKind::NonResuming => 1,
        hir::HandleArmKind::EscapeContinuation { continuation } => {
            2 ^ (continuation.as_u32() as usize)
        }
    }
}

/// Build the shared ordinary callee suspend/resume plan from a function or closure body.
pub(crate) fn build_ordinary_callee_suspend_plan_with_context(
    types: &TypeStore,
    body: &hir::Block,
    declared_return_ty: TypeId,
    context: &mut EffectAnalysisCtx,
) -> Option<CalleeSuspendPlan> {
    let synthetic_handle = hir::HandleExpr {
        body: body.clone(),
        arms: Vec::new(),
        finally: None,
    };

    context.extend_known_local_metadata_from_handle(&synthetic_handle);

    let mut builder = HandlePlanBuilder::new(types, &synthetic_handle, context);
    let outer_slots = collect_outer_scope_slots(&synthetic_handle, &context.known_local_metadata);
    let mut env = ScopeEnv::with_outer(outer_slots.clone());
    for slot in &outer_slots {
        builder.frame_slots.insert(slot.id, slot.clone());
    }

    let entry_state = builder.new_state("ordinary.body.entry");
    let _body_end_state = builder.build_block(&synthetic_handle.body, entry_state, &mut env);
    builder.attach_suspend_source_paths();
    builder.attach_suspend_resume_paths();

    if builder.suspend_sites.is_empty() {
        return None;
    }

    let mut allocate_synthetic_symbol_id = || context.allocate_synthetic_symbol_id();
    let mut resume_sites = Vec::new();

    for site in &builder.suspend_sites {
        if !matches!(site.kind, SuspendSiteKind::Perform { .. }) {
            return None;
        }

        let source_path = site.source_path.as_ref()?;
        let resume_path = site.resume_path.as_ref()?;
        let source_expr = builder.resume_source_exprs.get(&site.id)?;
        let resume_slot = builder.resume_slot_for_site(site.id)?;
        let resume_slot_ty = ordinary_callee_resume_slot_type(
            body,
            source_path,
            resume_path,
            declared_return_ty,
            &resume_slot,
        );
        let resume_tail = build_ordinary_callee_resume_tail_block(
            &synthetic_handle.body,
            source_path,
            source_expr,
            resume_path,
            &resume_slot,
            &mut allocate_synthetic_symbol_id,
        )?;

        let saved_locals = site
            .available_locals
            .iter()
            .filter_map(|id| builder.frame_slots.get(id))
            .map(|slot| CalleeSuspendSavedLocal {
                id: slot.id(),
                name: slot.name().to_string(),
                ty: slot.ty(),
                mutable: slot.mutable(),
            })
            .collect::<Vec<_>>();

        resume_sites.push(CalleeSuspendResumeSite {
            site_id: site.id,
            span: site.span,
            saved_locals,
            resume_slot_id: resume_slot.id(),
            resume_slot_name: resume_slot.name().to_string(),
            resume_slot_ty,
            resume_tail,
        });
    }

    let mut seen_local_ids = HashSet::new();
    let mut saved_locals = Vec::new();
    for site in &resume_sites {
        for local in &site.saved_locals {
            if seen_local_ids.insert(local.id) {
                saved_locals.push(local.clone());
            }
        }
    }

    Some(CalleeSuspendPlan {
        saved_locals,
        resume_sites,
    })
}

/// `T4008b1`：为当前 `handle` 中的 escape continuation arm 计算 resumed-step effect row。
pub(crate) fn compute_escape_continuation_direct_step_effect_rows_for_handle(
    types: &TypeStore,
    handle: &hir::HandleExpr,
) -> HashMap<hir::SymbolId, EffectRow> {
    compute_escape_continuation_direct_step_effect_rows_for_handle_with_program(types, handle, None)
}

pub(crate) fn compute_escape_continuation_direct_step_effect_rows_for_handle_in_program(
    types: &TypeStore,
    handle: &hir::HandleExpr,
    object_inits: &hir::ObjectInitIndex,
    top_level_immutable_values: &hir::TopLevelImmutableValueIndex,
) -> HashMap<hir::SymbolId, EffectRow> {
    compute_escape_continuation_direct_step_effect_rows_for_handle_with_program(
        types,
        handle,
        Some(DirectStepProgramInfo {
            object_inits,
            top_level_immutable_values,
        }),
    )
}

fn compute_escape_continuation_direct_step_effect_rows_for_handle_with_program<'a>(
    types: &TypeStore,
    handle: &hir::HandleExpr,
    program: Option<DirectStepProgramInfo<'a>>,
) -> HashMap<hir::SymbolId, EffectRow> {
    let mut by_binder: HashMap<hir::SymbolId, Vec<TypeId>> = HashMap::new();
    for site_summary in compute_escape_continuation_direct_step_rows_by_site(types, handle, program)
    {
        by_binder
            .entry(site_summary.continuation)
            .or_default()
            .extend(site_summary.effects.terms);
    }

    by_binder
        .into_iter()
        .map(|(continuation, effects)| (continuation, EffectRow::new(effects)))
        .collect()
}

#[derive(Debug, Clone)]
struct EscapeSiteDirectStepRow {
    site_id: SuspendSiteId,
    continuation: hir::SymbolId,
    effects: EffectRow,
}

fn compute_escape_continuation_direct_step_rows_by_site(
    types: &TypeStore,
    handle: &hir::HandleExpr,
    program: Option<DirectStepProgramInfo<'_>>,
) -> Vec<EscapeSiteDirectStepRow> {
    let mut context = direct_step_analysis_context_for_handle(handle);
    context.extend_known_local_metadata_from_handle(handle);

    let mut builder = HandlePlanBuilder::new(types, handle, &context);
    let outer_slots = collect_outer_scope_slots(handle, &context.known_local_metadata);
    let mut env = ScopeEnv::with_outer(outer_slots.clone());
    for slot in &outer_slots {
        builder.frame_slots.insert(slot.id, slot.clone());
    }

    let entry_state = builder.new_state("body.entry");
    let exit_state = builder.new_state("body.exit");
    let body_end_state = builder.build_block(&handle.body, entry_state, &mut env);

    if let Some(finally_block) = &handle.finally {
        let cleanup_entry = builder.new_state("cleanup.finally.entry");
        let cleanup_exit = builder.new_state("cleanup.finally.exit");
        let cleanup_scope_id = builder.next_cleanup_id;
        builder.next_cleanup_id = builder.next_cleanup_id.saturating_add(1);
        builder.cleanup_scopes.push(CleanupScopePlan {
            id: cleanup_scope_id,
            kind: CleanupScopeKind::Finally,
            entry_state: cleanup_entry,
            exit_state: cleanup_exit,
            note: "normal/raise edges converge through a shared finally scope".to_string(),
        });
        builder.set_terminator(
            body_end_state,
            StateTerminator::CleanupEnter {
                scope_id: cleanup_scope_id,
                next_state: cleanup_entry,
            },
        );
        let mut cleanup_env = ScopeEnv::with_outer(outer_slots);
        let cleanup_end = builder.build_block(finally_block, cleanup_entry, &mut cleanup_env);
        builder
            .state_mut(cleanup_end)
            .actions
            .push(HandleStateOp::CleanupEdgeComplete);
        builder.set_terminator(cleanup_end, StateTerminator::Goto(cleanup_exit));
        builder.set_terminator(cleanup_exit, StateTerminator::Goto(exit_state));
    } else {
        builder.set_terminator(body_end_state, StateTerminator::Goto(exit_state));
    }

    builder
        .state_mut(exit_state)
        .actions
        .push(HandleStateOp::ReturnToEnclosingExpression);
    builder.set_terminator(exit_state, StateTerminator::ReturnHandle);

    let _dispatch_plan = builder.build_dispatch_plan();
    builder.build_arm_states();
    builder.compute_capture_sets();
    builder.attach_suspend_source_paths();
    builder.attach_suspend_resume_paths();
    builder.materialize_resume_fragments();
    builder.attach_escape_resume_targets();
    builder.compute_capture_sets();

    let mut rows = Vec::new();
    for site in &builder.suspend_sites {
        let SuspendSiteKind::Perform { op_fqn } = &site.kind else {
            continue;
        };
        let Some(source_expr) = builder.resume_source_exprs.get(&site.id) else {
            continue;
        };
        let hir::ExprKind::Perform { effect_ty, .. } = &source_expr.kind else {
            continue;
        };
        let Some(continuation) =
            select_escape_continuation_for_direct_site(handle, op_fqn, *effect_ty)
        else {
            continue;
        };
        let Some(source_path) = site.source_path.as_ref() else {
            continue;
        };
        let Some(resume_path) = site.resume_path.as_ref() else {
            continue;
        };
        let Some(resume_slot) = builder.resume_slot_for_site(site.id) else {
            continue;
        };
        let mut allocate_synthetic_symbol_id = || context.allocate_synthetic_symbol_id();
        let Some(resume_tail) = build_ordinary_callee_resume_tail_block(
            &handle.body,
            source_path,
            source_expr,
            resume_path,
            &resume_slot,
            &mut allocate_synthetic_symbol_id,
        ) else {
            continue;
        };
        let effects = summarize_direct_step_effects_in_block(
            types,
            &resume_tail,
            handle,
            &context.known_local_metadata,
            program,
        );
        rows.push(EscapeSiteDirectStepRow {
            site_id: site.id,
            continuation,
            effects,
        });
    }

    rows
}

fn direct_step_analysis_context_for_handle(handle: &hir::HandleExpr) -> HandlePlanContext {
    let mut known_local_metadata = HashMap::new();
    collect_known_local_metadata_in_handle(handle, &mut known_local_metadata);
    HandlePlanContext::new(
        HashMap::new(),
        HashMap::new(),
        known_local_metadata,
        PathBuf::from("<t4008b1a>"),
        Rc::new(ProgramFacts::default()),
    )
}

fn select_escape_continuation_for_direct_site(
    handle: &hir::HandleExpr,
    op_fqn: &str,
    effect_ty: TypeId,
) -> Option<hir::SymbolId> {
    let mut same_op_fallback = None;
    for arm in &handle.arms {
        if arm.op.op.fqn != op_fqn {
            continue;
        }
        if same_op_fallback.is_none()
            && let hir::HandleArmKind::EscapeContinuation { continuation } = arm.kind
        {
            same_op_fallback = Some(continuation);
        }

        if arm.op.effect_ty != effect_ty {
            continue;
        }
        match arm.kind {
            hir::HandleArmKind::EscapeContinuation { continuation } => return Some(continuation),
            hir::HandleArmKind::NonResuming => return None,
        }
    }
    same_op_fallback
}

fn summarize_direct_step_effects_in_block(
    types: &TypeStore,
    block: &hir::Block,
    handle: &hir::HandleExpr,
    known_local_metadata: &HashMap<hir::SymbolId, KnownLocalMetadata>,
    program: Option<DirectStepProgramInfo<'_>>,
) -> EffectRow {
    let analysis = DirectStepAnalysis::new(program);
    let summary = summarize_direct_step_resume_tail_block(
        types,
        block,
        handle,
        known_local_metadata,
        &analysis,
    );
    EffectRow::new(summary.effects)
}

#[derive(Debug, Clone, Copy)]
struct DirectStepProgramInfo<'a> {
    object_inits: &'a hir::ObjectInitIndex,
    top_level_immutable_values: &'a hir::TopLevelImmutableValueIndex,
}

#[derive(Debug, Clone)]
struct DirectStepAnalysis<'a> {
    program: Option<DirectStepProgramInfo<'a>>,
    hidden_boundary_stack: HashSet<String>,
}

impl<'a> DirectStepAnalysis<'a> {
    fn new(program: Option<DirectStepProgramInfo<'a>>) -> Self {
        Self {
            program,
            hidden_boundary_stack: HashSet::new(),
        }
    }

    fn for_hidden_boundary(&self, key: &str) -> Option<Self> {
        if self.hidden_boundary_stack.contains(key) {
            return None;
        }
        let mut next = self.clone();
        next.hidden_boundary_stack.insert(key.to_string());
        Some(next)
    }
}

#[derive(Debug, Clone, Copy)]
enum DirectStepHandleRole {
    ResumeStep,
    HandleExpression,
}

#[derive(Debug, Clone, Copy)]
struct ActiveDirectStepHandleContext<'a> {
    handle: &'a hir::HandleExpr,
    role: DirectStepHandleRole,
}

#[derive(Debug, Clone, Copy)]
enum DirectStepMode<'a> {
    OutsideHandle,
    ActiveHandle(ActiveDirectStepHandleContext<'a>),
}

#[derive(Debug, Clone, Copy)]
enum DirectStepTerminalKind {
    HandleCompletion,
    TerminalStop,
}

#[derive(Debug, Clone)]
struct DirectStepSummary {
    effects: Vec<TypeId>,
    may_continue: bool,
    may_stop: bool,
}

impl DirectStepSummary {
    fn empty() -> Self {
        Self {
            effects: Vec::new(),
            may_continue: false,
            may_stop: false,
        }
    }

    fn continue_pure() -> Self {
        Self {
            effects: Vec::new(),
            may_continue: true,
            may_stop: false,
        }
    }

    fn stop_pure() -> Self {
        Self {
            effects: Vec::new(),
            may_continue: false,
            may_stop: true,
        }
    }

    fn outward(mut effects: Vec<TypeId>) -> Self {
        effects.sort();
        effects.dedup();
        Self {
            effects,
            may_continue: false,
            may_stop: false,
        }
    }

    fn merge_effects(&mut self, mut more: Vec<TypeId>) {
        self.effects.append(&mut more);
        self.effects.sort();
        self.effects.dedup();
    }

    fn merge_paths(&mut self, other: Self) {
        self.merge_effects(other.effects);
        self.may_continue |= other.may_continue;
        self.may_stop |= other.may_stop;
    }

    fn without_continue(&self) -> Self {
        Self {
            effects: self.effects.clone(),
            may_continue: false,
            may_stop: self.may_stop,
        }
    }
}

#[derive(Debug, Clone)]
enum DirectStepHiddenBoundary<'a> {
    TopLevelImmutable {
        fqn: String,
        value: &'a hir::TopLevelImmutableValue,
    },
    ObjectInit {
        fqn: String,
        init: &'a hir::ObjectInit,
    },
}

impl<'a> DirectStepHiddenBoundary<'a> {
    fn key(&self) -> &str {
        match self {
            DirectStepHiddenBoundary::TopLevelImmutable { fqn, .. }
            | DirectStepHiddenBoundary::ObjectInit { fqn, .. } => fqn,
        }
    }
}

fn summarize_direct_step_resume_tail_block(
    types: &TypeStore,
    block: &hir::Block,
    handle: &hir::HandleExpr,
    known_local_metadata: &HashMap<hir::SymbolId, KnownLocalMetadata>,
    analysis: &DirectStepAnalysis<'_>,
) -> DirectStepSummary {
    let ctx = ActiveDirectStepHandleContext {
        handle,
        role: DirectStepHandleRole::ResumeStep,
    };
    let summary = summarize_direct_step_stmt_seq(
        types,
        &block.stmts,
        DirectStepMode::ActiveHandle(ctx),
        known_local_metadata,
        analysis,
    );
    let mut out = summary.without_continue();
    if summary.may_continue {
        out.merge_paths(finalize_handle_terminal(
            types,
            ctx,
            analysis,
            DirectStepTerminalKind::HandleCompletion,
        ));
    }
    out
}

fn summarize_direct_step_handle_execution(
    types: &TypeStore,
    handle: &hir::HandleExpr,
    role: DirectStepHandleRole,
    analysis: &DirectStepAnalysis<'_>,
) -> DirectStepSummary {
    let mut known_local_metadata = HashMap::new();
    collect_known_local_metadata_in_handle(handle, &mut known_local_metadata);
    let ctx = ActiveDirectStepHandleContext { handle, role };
    let summary = summarize_direct_step_stmt_seq(
        types,
        &handle.body.stmts,
        DirectStepMode::ActiveHandle(ctx),
        &known_local_metadata,
        analysis,
    );
    let mut out = summary.without_continue();
    if summary.may_continue {
        out.merge_paths(finalize_handle_terminal(
            types,
            ctx,
            analysis,
            DirectStepTerminalKind::HandleCompletion,
        ));
    }
    out
}

fn summarize_direct_step_stmt_seq(
    types: &TypeStore,
    stmts: &[hir::Stmt],
    mode: DirectStepMode<'_>,
    known_local_metadata: &HashMap<hir::SymbolId, KnownLocalMetadata>,
    analysis: &DirectStepAnalysis<'_>,
) -> DirectStepSummary {
    let mut out = DirectStepSummary::continue_pure();
    for stmt in stmts {
        if !out.may_continue {
            break;
        }
        let step = summarize_direct_step_stmt(types, stmt, mode, known_local_metadata, analysis);
        out.merge_effects(step.effects);
        out.may_stop |= step.may_stop;
        out.may_continue = step.may_continue;
    }
    out
}

fn summarize_direct_step_stmt(
    types: &TypeStore,
    stmt: &hir::Stmt,
    mode: DirectStepMode<'_>,
    known_local_metadata: &HashMap<hir::SymbolId, KnownLocalMetadata>,
    analysis: &DirectStepAnalysis<'_>,
) -> DirectStepSummary {
    match &stmt.kind {
        hir::StmtKind::Empty => DirectStepSummary::continue_pure(),
        hir::StmtKind::Expr(expr) => {
            summarize_direct_step_expr(types, expr, mode, known_local_metadata, analysis)
        }
        hir::StmtKind::Val(decl) => decl
            .init
            .as_ref()
            .map(|expr| {
                summarize_direct_step_expr(types, expr, mode, known_local_metadata, analysis)
            })
            .unwrap_or_else(DirectStepSummary::continue_pure),
        hir::StmtKind::Assign { lhs, rhs, .. } => {
            let lhs_summary =
                summarize_direct_step_expr(types, lhs, mode, known_local_metadata, analysis);
            let mut out = lhs_summary.without_continue();
            if lhs_summary.may_continue {
                let rhs_summary =
                    summarize_direct_step_expr(types, rhs, mode, known_local_metadata, analysis);
                out.merge_paths(rhs_summary);
            }
            out
        }
        hir::StmtKind::Return { value } => summarize_direct_step_return_stmt(
            types,
            value.as_ref(),
            mode,
            known_local_metadata,
            analysis,
        ),
        hir::StmtKind::While { cond, body } => {
            let cond_summary =
                summarize_direct_step_expr(types, cond, mode, known_local_metadata, analysis);
            let mut out = cond_summary.without_continue();
            if cond_summary.may_continue {
                let body_summary = summarize_direct_step_stmt_seq(
                    types,
                    &body.stmts,
                    mode,
                    known_local_metadata,
                    analysis,
                );
                out.merge_effects(body_summary.effects);
                out.may_stop |= body_summary.may_stop;
                // 仍保留保守 loop union 近似；更细的 break/continue
                // path-sensitive 语义不在 T4008b1b 范围。
                out.may_continue = true;
            }
            out
        }
        hir::StmtKind::Break { .. } | hir::StmtKind::Continue { .. } => {
            DirectStepSummary::stop_pure()
        }
        hir::StmtKind::Todo(_) => DirectStepSummary::continue_pure(),
    }
}

fn summarize_direct_step_return_stmt(
    types: &TypeStore,
    value: Option<&hir::Expr>,
    mode: DirectStepMode<'_>,
    known_local_metadata: &HashMap<hir::SymbolId, KnownLocalMetadata>,
    analysis: &DirectStepAnalysis<'_>,
) -> DirectStepSummary {
    let value_summary = value
        .map(|expr| summarize_direct_step_expr(types, expr, mode, known_local_metadata, analysis))
        .unwrap_or_else(DirectStepSummary::continue_pure);
    let mut out = value_summary.without_continue();
    if value_summary.may_continue {
        match mode {
            DirectStepMode::OutsideHandle => out.may_stop = true,
            DirectStepMode::ActiveHandle(ctx) => out.merge_paths(finalize_handle_terminal(
                types,
                ctx,
                analysis,
                DirectStepTerminalKind::TerminalStop,
            )),
        }
    }
    out
}

fn summarize_direct_step_expr(
    types: &TypeStore,
    expr: &hir::Expr,
    mode: DirectStepMode<'_>,
    known_local_metadata: &HashMap<hir::SymbolId, KnownLocalMetadata>,
    analysis: &DirectStepAnalysis<'_>,
) -> DirectStepSummary {
    match &expr.kind {
        hir::ExprKind::Missing
        | hir::ExprKind::Literal(_)
        | hir::ExprKind::UnresolvedIdent { .. }
        | hir::ExprKind::ClassLiteral(_)
        | hir::ExprKind::Closure(_)
        | hir::ExprKind::Todo(_) => DirectStepSummary::continue_pure(),
        hir::ExprKind::VarRef(value_ref) => {
            if let Some(boundary) =
                classify_direct_step_hidden_boundary_for_value_ref(analysis.program, value_ref)
            {
                summarize_hidden_boundary_access(types, boundary, mode, analysis)
            } else {
                DirectStepSummary::continue_pure()
            }
        }
        hir::ExprKind::Block(block) => summarize_direct_step_stmt_seq(
            types,
            &block.stmts,
            mode,
            known_local_metadata,
            analysis,
        ),
        hir::ExprKind::Unary { expr: inner, .. }
        | hir::ExprKind::Cast { expr: inner, .. }
        | hir::ExprKind::TypeCheck { expr: inner, .. } => {
            summarize_direct_step_expr(types, inner, mode, known_local_metadata, analysis)
        }
        hir::ExprKind::MemberAccess { receiver, member } => {
            let receiver_summary =
                summarize_direct_step_expr(types, receiver, mode, known_local_metadata, analysis);
            let mut out = receiver_summary.without_continue();
            if !receiver_summary.may_continue {
                return out;
            }
            if let Some(boundary) =
                classify_direct_step_hidden_boundary_for_member_access(analysis.program, member)
            {
                out.merge_paths(summarize_hidden_boundary_access(
                    types, boundary, mode, analysis,
                ));
            } else {
                out.may_continue = true;
            }
            out
        }
        hir::ExprKind::StructLit { fields, .. } => summarize_direct_step_exprs(
            types,
            fields.iter().map(|field| &field.value),
            mode,
            known_local_metadata,
            analysis,
        ),
        hir::ExprKind::TupleLit { elements } => summarize_direct_step_exprs(
            types,
            elements.iter(),
            mode,
            known_local_metadata,
            analysis,
        ),
        hir::ExprKind::InterpolatedString { parts, .. } => summarize_direct_step_exprs(
            types,
            parts.iter().filter_map(|part| match part {
                hir::InterpolatedStringPart::Expr { expr } => Some(expr),
                hir::InterpolatedStringPart::Text { .. } => None,
            }),
            mode,
            known_local_metadata,
            analysis,
        ),
        hir::ExprKind::Binary { lhs, rhs, .. } => {
            let lhs_summary =
                summarize_direct_step_expr(types, lhs, mode, known_local_metadata, analysis);
            let mut out = lhs_summary.without_continue();
            if lhs_summary.may_continue {
                let rhs_summary =
                    summarize_direct_step_expr(types, rhs, mode, known_local_metadata, analysis);
                out.merge_paths(rhs_summary);
            }
            out
        }
        hir::ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => {
            let cond_summary =
                summarize_direct_step_expr(types, cond, mode, known_local_metadata, analysis);
            let mut out = cond_summary.without_continue();
            if cond_summary.may_continue {
                let then_summary = summarize_direct_step_expr(
                    types,
                    then_branch,
                    mode,
                    known_local_metadata,
                    analysis,
                );
                let else_summary = else_branch
                    .as_deref()
                    .map(|expr| {
                        summarize_direct_step_expr(
                            types,
                            expr,
                            mode,
                            known_local_metadata,
                            analysis,
                        )
                    })
                    .unwrap_or_else(DirectStepSummary::continue_pure);
                out.merge_paths(then_summary);
                out.merge_paths(else_summary);
            }
            out
        }
        hir::ExprKind::When { subject, arms } => {
            let subject_summary =
                summarize_direct_step_expr(types, subject, mode, known_local_metadata, analysis);
            let mut out = subject_summary.without_continue();
            if !subject_summary.may_continue {
                return out;
            }
            if arms.is_empty() {
                out.may_continue = true;
                return out;
            }
            let mut branch_union = DirectStepSummary::empty();
            for arm in arms {
                let guard_summary = arm
                    .guard
                    .as_ref()
                    .map(|guard| {
                        summarize_direct_step_expr(
                            types,
                            guard,
                            mode,
                            known_local_metadata,
                            analysis,
                        )
                    })
                    .unwrap_or_else(DirectStepSummary::continue_pure);
                let mut branch = guard_summary.without_continue();
                if guard_summary.may_continue {
                    branch.merge_paths(summarize_direct_step_expr(
                        types,
                        &arm.body,
                        mode,
                        known_local_metadata,
                        analysis,
                    ));
                }
                branch_union.merge_paths(branch);
            }
            out.merge_paths(branch_union);
            out
        }
        hir::ExprKind::Call { callee, args } => summarize_direct_step_call_expr(
            types,
            callee,
            args,
            mode,
            known_local_metadata,
            analysis,
        ),
        hir::ExprKind::Perform {
            effect_ty,
            op,
            args,
        } => summarize_direct_step_perform_expr(
            types,
            *effect_ty,
            &op.fqn,
            args,
            mode,
            known_local_metadata,
            analysis,
        ),
        hir::ExprKind::Handle(handle) => match mode {
            DirectStepMode::OutsideHandle => summarize_direct_step_handle_execution(
                types,
                handle,
                DirectStepHandleRole::HandleExpression,
                analysis,
            ),
            DirectStepMode::ActiveHandle(ctx) => finalize_boundary_summary_in_mode(
                types,
                summarize_direct_step_handle_execution(
                    types,
                    handle,
                    DirectStepHandleRole::HandleExpression,
                    analysis,
                ),
                DirectStepMode::ActiveHandle(ctx),
                analysis,
            ),
        },
    }
}

fn summarize_direct_step_call_expr(
    types: &TypeStore,
    callee: &hir::Expr,
    args: &[hir::CallArg],
    mode: DirectStepMode<'_>,
    known_local_metadata: &HashMap<hir::SymbolId, KnownLocalMetadata>,
    analysis: &DirectStepAnalysis<'_>,
) -> DirectStepSummary {
    let callee_summary =
        summarize_direct_step_expr(types, callee, mode, known_local_metadata, analysis);
    let mut out = callee_summary.without_continue();
    if !callee_summary.may_continue {
        return out;
    }

    let args_summary =
        summarize_direct_step_call_args(types, args, mode, known_local_metadata, analysis);
    out.merge_effects(args_summary.effects.clone());
    out.may_stop |= args_summary.may_stop;
    if !args_summary.may_continue {
        return out;
    }

    let direct_effects =
        direct_effect_terms_from_callable_expr(types, callee, known_local_metadata);
    match mode {
        DirectStepMode::OutsideHandle => {
            out.merge_effects(direct_effects.clone());
            out.may_continue = direct_effects.is_empty();
            out
        }
        DirectStepMode::ActiveHandle(_) => {
            let mut boundary = DirectStepSummary::continue_pure();
            boundary.merge_effects(direct_effects);
            out.merge_paths(finalize_boundary_summary_in_mode(
                types, boundary, mode, analysis,
            ));
            out
        }
    }
}

fn summarize_direct_step_perform_expr(
    types: &TypeStore,
    effect_ty: TypeId,
    op_fqn: &str,
    args: &[hir::CallArg],
    mode: DirectStepMode<'_>,
    known_local_metadata: &HashMap<hir::SymbolId, KnownLocalMetadata>,
    analysis: &DirectStepAnalysis<'_>,
) -> DirectStepSummary {
    let args_summary =
        summarize_direct_step_call_args(types, args, mode, known_local_metadata, analysis);
    let mut out = args_summary.without_continue();
    if !args_summary.may_continue {
        return out;
    }

    match mode {
        DirectStepMode::OutsideHandle => {
            out.merge_paths(DirectStepSummary::outward(vec![effect_ty]));
            out
        }
        DirectStepMode::ActiveHandle(ctx) => {
            if let Some(arm) = first_matching_arm_for_direct_perform(ctx.handle, op_fqn, effect_ty)
            {
                out.merge_paths(summarize_direct_step_dispatch_arm(
                    types, arm, ctx, analysis,
                ));
            } else {
                out.merge_paths(finalize_handle_outward(
                    types,
                    ctx,
                    analysis,
                    vec![effect_ty],
                ));
            }
            out
        }
    }
}

fn summarize_hidden_boundary_access(
    types: &TypeStore,
    boundary: DirectStepHiddenBoundary<'_>,
    mode: DirectStepMode<'_>,
    analysis: &DirectStepAnalysis<'_>,
) -> DirectStepSummary {
    let boundary_summary = summarize_hidden_boundary(types, boundary, analysis);
    finalize_boundary_summary_in_mode(types, boundary_summary, mode, analysis)
}

fn summarize_hidden_boundary(
    types: &TypeStore,
    boundary: DirectStepHiddenBoundary<'_>,
    analysis: &DirectStepAnalysis<'_>,
) -> DirectStepSummary {
    let Some(next_analysis) = analysis.for_hidden_boundary(boundary.key()) else {
        return DirectStepSummary::stop_pure();
    };
    match boundary {
        DirectStepHiddenBoundary::TopLevelImmutable { value, .. } => value
            .init
            .as_ref()
            .map(|init| {
                let mut known_local_metadata = HashMap::new();
                collect_known_local_metadata_in_expr(init, &mut known_local_metadata);
                summarize_direct_step_expr(
                    types,
                    init,
                    DirectStepMode::OutsideHandle,
                    &known_local_metadata,
                    &next_analysis,
                )
            })
            .unwrap_or_else(DirectStepSummary::continue_pure),
        DirectStepHiddenBoundary::ObjectInit { init, .. } => {
            let mut known_local_metadata = HashMap::new();
            for step in &init.steps {
                match step {
                    hir::ObjectInitStep::PropertyInit { init, .. } => {
                        collect_known_local_metadata_in_expr(init, &mut known_local_metadata);
                    }
                    hir::ObjectInitStep::InitBlock { block } => {
                        collect_known_local_metadata_in_block(block, &mut known_local_metadata);
                    }
                }
            }

            let mut out = DirectStepSummary::continue_pure();
            for step in &init.steps {
                if !out.may_continue {
                    break;
                }
                let step_summary = match step {
                    hir::ObjectInitStep::PropertyInit { init, .. } => summarize_direct_step_expr(
                        types,
                        init,
                        DirectStepMode::OutsideHandle,
                        &known_local_metadata,
                        &next_analysis,
                    ),
                    hir::ObjectInitStep::InitBlock { block } => summarize_direct_step_stmt_seq(
                        types,
                        &block.stmts,
                        DirectStepMode::OutsideHandle,
                        &known_local_metadata,
                        &next_analysis,
                    ),
                };
                out.merge_effects(step_summary.effects);
                out.may_stop |= step_summary.may_stop;
                out.may_continue = step_summary.may_continue;
            }
            out
        }
    }
}

fn finalize_boundary_summary_in_mode(
    types: &TypeStore,
    boundary_summary: DirectStepSummary,
    mode: DirectStepMode<'_>,
    analysis: &DirectStepAnalysis<'_>,
) -> DirectStepSummary {
    match mode {
        DirectStepMode::OutsideHandle => boundary_summary,
        DirectStepMode::ActiveHandle(ctx) => {
            let mut out = DirectStepSummary::empty();
            if boundary_summary.may_continue {
                out.may_continue = true;
            }
            if boundary_summary.may_stop {
                out.merge_paths(finalize_handle_terminal(
                    types,
                    ctx,
                    analysis,
                    DirectStepTerminalKind::TerminalStop,
                ));
            }
            if !boundary_summary.effects.is_empty() {
                out.merge_paths(dispatch_boundary_effects_through_active_handle(
                    types,
                    &boundary_summary.effects,
                    ctx,
                    analysis,
                ));
            }
            out
        }
    }
}

fn dispatch_boundary_effects_through_active_handle(
    types: &TypeStore,
    effects: &[TypeId],
    ctx: ActiveDirectStepHandleContext<'_>,
    analysis: &DirectStepAnalysis<'_>,
) -> DirectStepSummary {
    let mut out = DirectStepSummary::empty();
    for effect_ty in effects {
        let matching_arms = ctx
            .handle
            .arms
            .iter()
            .filter(|arm| arm.op.effect_ty == *effect_ty)
            .collect::<Vec<_>>();
        if matching_arms.is_empty() {
            out.merge_paths(finalize_handle_outward(
                types,
                ctx,
                analysis,
                vec![*effect_ty],
            ));
            continue;
        }
        for arm in matching_arms {
            out.merge_paths(summarize_direct_step_dispatch_arm(
                types, arm, ctx, analysis,
            ));
        }
    }
    out
}

fn summarize_direct_step_dispatch_arm(
    types: &TypeStore,
    arm: &hir::HandleArm,
    ctx: ActiveDirectStepHandleContext<'_>,
    analysis: &DirectStepAnalysis<'_>,
) -> DirectStepSummary {
    let known_local_metadata = collect_known_local_metadata_in_handle_arm(arm);
    let arm_summary = summarize_direct_step_expr(
        types,
        &arm.body,
        DirectStepMode::OutsideHandle,
        &known_local_metadata,
        analysis,
    );

    let mut out = DirectStepSummary::empty();
    if !arm_summary.effects.is_empty() {
        out.merge_paths(finalize_handle_outward(
            types,
            ctx,
            analysis,
            arm_summary.effects.clone(),
        ));
    }
    if arm_summary.may_stop {
        out.merge_paths(finalize_handle_terminal(
            types,
            ctx,
            analysis,
            DirectStepTerminalKind::TerminalStop,
        ));
    }
    if arm_summary.may_continue {
        match arm.kind {
            hir::HandleArmKind::EscapeContinuation { continuation }
                if tail_resume_arm_matches_static(&arm.body, continuation) =>
            {
                out.may_continue = true
            }
            hir::HandleArmKind::NonResuming | hir::HandleArmKind::EscapeContinuation { .. } => {
                out.merge_paths(finalize_handle_terminal(
                    types,
                    ctx,
                    analysis,
                    DirectStepTerminalKind::HandleCompletion,
                ));
            }
        }
    }
    out
}

fn finalize_handle_terminal(
    types: &TypeStore,
    ctx: ActiveDirectStepHandleContext<'_>,
    analysis: &DirectStepAnalysis<'_>,
    kind: DirectStepTerminalKind,
) -> DirectStepSummary {
    let cleanup = summarize_direct_step_handle_finally(types, ctx.handle, analysis);
    let mut out = cleanup.without_continue();
    if cleanup.may_continue {
        match kind {
            DirectStepTerminalKind::HandleCompletion => match ctx.role {
                DirectStepHandleRole::ResumeStep => out.may_stop = true,
                DirectStepHandleRole::HandleExpression => out.may_continue = true,
            },
            DirectStepTerminalKind::TerminalStop => out.may_stop = true,
        }
    }
    out
}

fn finalize_handle_outward(
    types: &TypeStore,
    ctx: ActiveDirectStepHandleContext<'_>,
    analysis: &DirectStepAnalysis<'_>,
    effects: Vec<TypeId>,
) -> DirectStepSummary {
    let cleanup = summarize_direct_step_handle_finally(types, ctx.handle, analysis);
    let mut out = cleanup.without_continue();
    if cleanup.may_continue {
        out.merge_effects(effects);
    }
    out
}

fn summarize_direct_step_handle_finally(
    types: &TypeStore,
    handle: &hir::HandleExpr,
    analysis: &DirectStepAnalysis<'_>,
) -> DirectStepSummary {
    let Some(finally_block) = handle.finally.as_ref() else {
        return DirectStepSummary::continue_pure();
    };
    let mut known_local_metadata = HashMap::new();
    collect_known_local_metadata_in_block(finally_block, &mut known_local_metadata);
    summarize_direct_step_stmt_seq(
        types,
        &finally_block.stmts,
        DirectStepMode::OutsideHandle,
        &known_local_metadata,
        analysis,
    )
}

fn classify_direct_step_hidden_boundary_for_value_ref<'a>(
    program: Option<DirectStepProgramInfo<'a>>,
    value_ref: &hir::ValueRef,
) -> Option<DirectStepHiddenBoundary<'a>> {
    let program = program?;
    let hir::ValueRef::TopLevel { fqn, .. } = value_ref else {
        return None;
    };
    if let Some(init) = program.object_inits.get(fqn) {
        return Some(DirectStepHiddenBoundary::ObjectInit {
            fqn: fqn.clone(),
            init,
        });
    }
    program.top_level_immutable_values.get(fqn).map(|value| {
        DirectStepHiddenBoundary::TopLevelImmutable {
            fqn: fqn.clone(),
            value,
        }
    })
}

fn classify_direct_step_hidden_boundary_for_member_access<'a>(
    program: Option<DirectStepProgramInfo<'a>>,
    member: &hir::MemberAccess,
) -> Option<DirectStepHiddenBoundary<'a>> {
    let program = program?;
    let hir::MemberRef::Value { fqn, .. } = member.resolved.as_ref()? else {
        return None;
    };
    let (owner_fqn, _) = fqn.rsplit_once('.')?;
    program
        .object_inits
        .get(owner_fqn)
        .map(|init| DirectStepHiddenBoundary::ObjectInit {
            fqn: owner_fqn.to_string(),
            init,
        })
}

fn summarize_direct_step_call_args<'a>(
    types: &TypeStore,
    args: impl IntoIterator<Item = &'a hir::CallArg>,
    mode: DirectStepMode<'_>,
    known_local_metadata: &HashMap<hir::SymbolId, KnownLocalMetadata>,
    analysis: &DirectStepAnalysis<'_>,
) -> DirectStepSummary {
    let mut out = DirectStepSummary::continue_pure();
    for arg in args {
        if !out.may_continue {
            break;
        }
        let summary = match arg {
            hir::CallArg::Positional(expr) => {
                summarize_direct_step_expr(types, expr, mode, known_local_metadata, analysis)
            }
            hir::CallArg::Named { value, .. } => {
                summarize_direct_step_expr(types, value, mode, known_local_metadata, analysis)
            }
        };
        out.merge_effects(summary.effects);
        out.may_stop |= summary.may_stop;
        out.may_continue = summary.may_continue;
    }
    out
}

fn summarize_direct_step_exprs<'a>(
    types: &TypeStore,
    exprs: impl IntoIterator<Item = &'a hir::Expr>,
    mode: DirectStepMode<'_>,
    known_local_metadata: &HashMap<hir::SymbolId, KnownLocalMetadata>,
    analysis: &DirectStepAnalysis<'_>,
) -> DirectStepSummary {
    let mut out = DirectStepSummary::continue_pure();
    for expr in exprs {
        if !out.may_continue {
            break;
        }
        let summary = summarize_direct_step_expr(types, expr, mode, known_local_metadata, analysis);
        out.merge_effects(summary.effects);
        out.may_stop |= summary.may_stop;
        out.may_continue = summary.may_continue;
    }
    out
}

fn direct_effect_terms_from_callable_expr(
    types: &TypeStore,
    callee: &hir::Expr,
    known_local_metadata: &HashMap<hir::SymbolId, KnownLocalMetadata>,
) -> Vec<TypeId> {
    let callee_ty = match types.kind(callee.ty) {
        TypeKind::Ref(RefTypeKind::Function(_)) => callee.ty,
        _ => match &callee.kind {
            hir::ExprKind::VarRef(hir::ValueRef::Local { id, .. }) => {
                known_local_metadata.get(id).map(|metadata| metadata.ty)
            }
            _ => None,
        }
        .unwrap_or(callee.ty),
    };

    match types.kind(callee_ty) {
        TypeKind::Ref(RefTypeKind::Function(fun_ty)) => fun_ty.effects.terms.clone(),
        _ => Vec::new(),
    }
}

fn first_matching_arm_for_direct_perform<'a>(
    handle: &'a hir::HandleExpr,
    op_fqn: &str,
    effect_ty: TypeId,
) -> Option<&'a hir::HandleArm> {
    let mut same_op_fallback = None;
    for arm in &handle.arms {
        if arm.op.op.fqn != op_fqn {
            continue;
        }
        if same_op_fallback.is_none() {
            same_op_fallback = Some(arm);
        }
        if arm.op.effect_ty == effect_ty {
            return Some(arm);
        }
    }
    same_op_fallback
}

fn ordinary_callee_resume_slot_type(
    body: &hir::Block,
    source_path: &SuspendSourcePath,
    resume_path: &SuspendResumePath,
    declared_return_ty: TypeId,
    resume_slot: &FrameSlot,
) -> TypeId {
    match resume_path.consumer {
        SuspendResumeConsumer::ExprStmt
            if source_path.frames.is_empty()
                && source_path
                    .handle_body_stmt_idx()
                    .is_some_and(|stmt_idx| stmt_idx + 1 == body.stmts.len()) =>
        {
            declared_return_ty
        }
        SuspendResumeConsumer::ReturnValue
            if source_path.frames.is_empty() && source_path.handle_body_stmt_idx().is_some() =>
        {
            declared_return_ty
        }
        _ => resume_slot.ty(),
    }
}

#[cfg(test)]
mod plan_tests {
    use crate::parser::parse_file;
    use crate::resolve::Index;
    use crate::session::{EffectPipelineMode, Session, SessionOptions};
    use crate::source::SourceFile;
    use crate::typecheck;

    use super::*;

    #[test]
    fn continuation_escape_facts_enter_handle_planning_input() {
        let source_text = r#"
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
"#;
        let lowered = lower_typed_single_source(source_text);
        let source = SourceFile::new_virtual("<mem>", source_text);
        let session = legacy_session();
        let materialized = crate::mir::materialize_for_dump(&session, &source)
            .expect("materialized MIR should be available");
        let pass_view = materialized.pass_view();

        let (fun, handle) = first_handle_in_file(&lowered.file).expect("expected a handle");
        let resume_call_site = lowered
            .continuation_resume_call_sites
            .iter()
            .next()
            .expect("expected a Continuation.resume call site");

        let context_without_facts = collect_effect_analysis_context_for_fun(&lowered, fun);
        assert_eq!(
            context_without_facts.continuation_escape_state_for_call_span(resume_call_site.span),
            ContinuationEscapeState::Unknown,
            "missing MIR escape facts must stay conservative"
        );

        let context =
            collect_effect_analysis_context_for_fun_with_pass_view(&lowered, fun, Some(&pass_view));
        assert_eq!(
            context.continuation_escape_state_for_call_span(resume_call_site.span),
            ContinuationEscapeState::LocalResumeOnly,
            "MIR escape facts should be projected into EffectAnalysisCtx by call site"
        );

        let plan = HandleStateMachinePlan::build_with_context(&lowered.types, handle, &context);
        let resume_site = plan
            .suspend_sites
            .iter()
            .find(|site| site.kind.is_continuation_resume_boundary())
            .expect("Continuation.resume should create a hidden suspend site");
        assert_eq!(
            resume_site.continuation_escape,
            ContinuationEscapeState::LocalResumeOnly,
            "handle planning should record the continuation escape state on the suspend site"
        );
    }

    #[test]
    fn escaping_continuation_facts_enter_handle_planning_input() {
        let source_text = r#"
package a

import scoop.core.*

fun consume(k: Continuation<Int, Int>) {}

fun demo(k: Continuation<Int, Int>): Int {
    consume(k)
    val result: Int = try {
        k.resume(1)
        11
    } catch (e: RuntimeError) {
        22
    }
    result
}
"#;
        let lowered = lower_typed_single_source(source_text);
        let source = SourceFile::new_virtual("<mem>", source_text);
        let session = legacy_session();
        let materialized = crate::mir::materialize_for_dump(&session, &source)
            .expect("materialized MIR should be available");
        let pass_view = materialized.pass_view();

        let (fun, handle) = first_handle_in_file(&lowered.file).expect("expected a handle");
        let resume_call_site = lowered
            .continuation_resume_call_sites
            .iter()
            .next()
            .expect("expected a Continuation.resume call site");
        let context =
            collect_effect_analysis_context_for_fun_with_pass_view(&lowered, fun, Some(&pass_view));
        assert_eq!(
            context.continuation_escape_state_for_call_span(resume_call_site.span),
            ContinuationEscapeState::Escaping,
            "a continuation passed across a call boundary should project as escaping"
        );

        let plan = HandleStateMachinePlan::build_with_context(&lowered.types, handle, &context);
        let resume_site = plan
            .suspend_sites
            .iter()
            .find(|site| site.kind.is_continuation_resume_boundary())
            .expect("Continuation.resume should create a hidden suspend site");
        assert_eq!(
            resume_site.continuation_escape,
            ContinuationEscapeState::Escaping,
            "handle planning should retain escaping continuation facts"
        );
    }

    #[test]
    fn non_tail_escape_arm_with_outward_suspend_builds_inner_resume_site() {
        let lowered = lower_typed_single_source(
            r#"
package a

import scoop.core.*

effect Ask {
    fun current(): Int
}

effect Inner {
    fun enter(): Int
}

effect Boom {
    fun next(): Int
}

class Cell(var saved: Continuation<Int, Int, eff Boom>?, var total: Int)

fun start(cell: Cell): Int / Boom {
    return handle {
        val seed: Int = Ask.current()
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
        cell.total = seed + nested
        seed + nested
    } with {
        Ask.current(), k -> {
            cell.saved = Some(k)
            0 - 1
        }
    }
}
"#,
        );

        let (fun, handle) = first_handle_in_file(&lowered.file).expect("expected outer handle");
        let context = collect_effect_analysis_context_for_fun_with_pass_view(&lowered, fun, None);
        let plan = HandleStateMachinePlan::build_with_context(&lowered.types, handle, &context);
        fn has_resume_site(plan: &HandleStateMachinePlan) -> bool {
            plan.suspend_sites
                .iter()
                .any(|site| site.kind.is_continuation_resume_boundary())
                || plan.nested_handles.iter().any(has_resume_site)
        }

        let nested = plan
            .nested_handles
            .first()
            .expect("expected nested handle plan inside start");
        assert!(
            has_resume_site(nested),
            "inner handle arm body should materialize a first-class Continuation.resume suspend site instead of staying opaque"
        );
    }

    #[test]
    fn non_tail_escape_arm_nested_handle_boundary_escape_replay_keeps_arm_tail() {
        let source_text = r#"
package a

import scoop.core.*

effect Inner {
    fun enter(): Int
}

effect Boom {
    fun next(): Int
}

class Cell(var saved: Continuation<Int, Int>?)

fun demo(cell: Cell): Int {
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
                println("inner_arm_after_resume")
                resumed + 1
            }
        }
        println("after_nested")
        println(nested)
        nested
    } with {
        Boom.next(), k -> {
            cell.saved = Some(k)
            18
        }
    }
}
"#;
        let source = SourceFile::new_virtual("<mem>", source_text);
        let lowered = lower_typed_single_source(source_text);
        let (fun, handle) = first_handle_in_file(&lowered.file).expect("expected outer handle");
        let context = collect_effect_analysis_context_for_fun_with_pass_view(&lowered, fun, None);
        let plan = HandleStateMachinePlan::build_with_context(&lowered.types, handle, &context);
        let inner = plan
            .nested_handles
            .first()
            .expect("expected nested handle plan for Inner.enter arm");

        let boundary_site = inner
            .suspend_sites
            .iter()
            .find(|site| {
                matches!(site.kind, SuspendSiteKind::NestedHandleBoundary { .. })
                    && site
                        .source_path
                        .as_ref()
                        .is_some_and(|path| path.label().starts_with("arm#0"))
            })
            .expect("arm-body try/catch boundary should keep an arm-rooted source path");

        assert_eq!(
            boundary_site
                .source_path
                .as_ref()
                .expect("source path should exist")
                .label(),
            "arm#0 -> block[0]",
            "nested-handle boundary inside escape arm should be rooted at the arm body instead of falling back to opaque top-level replay"
        );

        let replay_state = inner
            .states
            .iter()
            .find(|state| state.id == boundary_site.resume_target)
            .expect("nested-handle boundary resume target should exist");
        let replay_snippets = replay_state
            .actions
            .iter()
            .filter_map(|op| state_action_source_snippet(op, &source))
            .collect::<Vec<_>>();

        assert!(
            replay_snippets
                .iter()
                .any(|snippet| snippet.contains("inner_arm_after_resume")),
            "nested-handle boundary resume fragment should keep the arm-body post-resume print instead of stopping at the inner try/nested-handle result: {replay_snippets:#?}"
        );
        assert!(
            replay_snippets
                .iter()
                .any(|snippet| snippet.contains("resumed + 1")),
            "nested-handle boundary resume fragment should keep the arm tail expression after nested-handle replay: {replay_snippets:#?}"
        );
    }

    #[test]
    fn direct_step_effect_rows_include_direct_effectful_call_after_escape_site() {
        let lowered = lower_typed_single_source(
            r#"
package a

import scoop.core.*

effect Ask {
    fun current(): Int
}

effect Boom {
    fun boom(code: Int): Int
}

fun demo(): Int / (Boom) {
    return handle {
        val burst: (Int) -> Int / (Boom) = { seed: Int ->
            Boom.boom(seed)
        }
        val value: Int = Ask.current()
        burst(value)
    } with {
        Ask.current(), k -> 7
    }
}
"#,
        );
        let handle = first_handle_in_file(&lowered.file)
            .map(|(_, handle)| handle)
            .expect("expected a handle");
        let continuation = only_escape_continuation_symbol(handle);

        let rows =
            compute_escape_continuation_direct_step_effect_rows_for_handle(&lowered.types, handle);
        let row = rows
            .get(&continuation)
            .expect("expected a direct-step effect row for escape continuation binder");

        assert_eq!(effect_row_terms(&lowered.types, row), ["a.Boom"]);
    }

    #[test]
    fn direct_step_rows_stop_at_next_escape_boundary() {
        let lowered = lower_typed_single_source(
            r#"
package a

import scoop.core.*

effect Ask {
    fun current(): Int
}

effect Boom {
    fun boom(code: Int): Int
}

fun demo(): Int / (Boom) {
    return handle {
        val first: Int = Ask.current()
        val second: Int = Ask.current()
        Boom.boom(second)
    } with {
        Ask.current(), k -> 7
    }
}
"#,
        );
        let handle = first_handle_in_file(&lowered.file)
            .map(|(_, handle)| handle)
            .expect("expected a handle");
        let continuation = only_escape_continuation_symbol(handle);

        let mut rows =
            compute_escape_continuation_direct_step_rows_by_site(&lowered.types, handle, None);
        rows.sort_by_key(|row| row.site_id);

        assert_eq!(
            rows.len(),
            2,
            "expected two handled Ask.current escape sites"
        );
        assert_eq!(rows[0].continuation, continuation);
        assert_eq!(rows[1].continuation, continuation);
        assert!(
            rows[0].effects.is_pure(),
            "first site should stop before the second escape boundary, found {:?}",
            effect_row_terms(&lowered.types, &rows[0].effects)
        );
        assert_eq!(
            effect_row_terms(&lowered.types, &rows[1].effects),
            ["a.Boom"]
        );
    }

    #[test]
    fn direct_step_rows_include_immediate_resume_arm_body_effects() {
        let lowered = lower_typed_single_source(
            r#"
package a

import scoop.core.*

effect Ask {
    fun current(): Int
}

effect Yield {
    fun next(): Int
}

effect Boom {
    fun boom(code: Int): Int
}

fun demo(): Int / (Boom) {
    return handle {
        val first: Int = Ask.current()
        val second: Int = Yield.next()
        first + second
    } with {
        Ask.current(), k -> 7
        Yield.next() , k -> {
            val _: Int = Boom.boom(41)
            k.resume(3)
        }
    }
}
"#,
        );
        let handle = first_handle_in_file(&lowered.file)
            .map(|(_, handle)| handle)
            .expect("expected a handle");
        let continuation = only_escape_continuation_symbol(handle);

        let rows =
            compute_escape_continuation_direct_step_effect_rows_for_handle(&lowered.types, handle);
        let row = rows
            .get(&continuation)
            .expect("expected a direct-step effect row for escape continuation binder");

        assert_eq!(effect_row_terms(&lowered.types, row), ["a.Boom"]);
    }

    #[test]
    fn direct_step_rows_include_escape_arm_body_effects_at_next_boundary() {
        let lowered = lower_typed_single_source(
            r#"
package a

import scoop.core.*

effect Ask {
    fun current(): Int
}

effect Boom {
    fun boom(code: Int): Int
}

fun demo(): Int / (Boom) {
    return handle {
        val first: Int = Ask.current()
        val second: Int = Ask.current()
        first + second
    } with {
        Ask.current(), k -> {
            val _: Int = Boom.boom(9)
            7
        }
    }
}
"#,
        );
        let handle = first_handle_in_file(&lowered.file)
            .map(|(_, handle)| handle)
            .expect("expected a handle");
        let mut rows =
            compute_escape_continuation_direct_step_rows_by_site(&lowered.types, handle, None);
        rows.sort_by_key(|row| row.site_id);

        assert_eq!(
            rows.len(),
            2,
            "expected two handled Ask.current escape sites"
        );
        assert_eq!(
            effect_row_terms(&lowered.types, &rows[0].effects),
            ["a.Boom"]
        );
        assert!(
            rows[1].effects.is_pure(),
            "second site should not count its own arm body as resumed tail, found {:?}",
            effect_row_terms(&lowered.types, &rows[1].effects)
        );
    }

    #[test]
    fn direct_step_rows_include_finally_effects_after_resumed_tail_completion() {
        let lowered = lower_typed_single_source(
            r#"
package a

import scoop.core.*

effect Ask {
    fun current(): Int
}

effect Boom {
    fun boom(code: Int): Int
}

fun demo(): Int / (Boom) {
    return handle {
        val value: Int = Ask.current()
        value + 1
    } with {
        Ask.current(), k -> 7
    } finally {
        val _: Int = Boom.boom(5)
    }
}
"#,
        );
        let handle = first_handle_in_file(&lowered.file)
            .map(|(_, handle)| handle)
            .expect("expected a handle");
        let continuation = only_escape_continuation_symbol(handle);

        let rows =
            compute_escape_continuation_direct_step_effect_rows_for_handle(&lowered.types, handle);
        let row = rows
            .get(&continuation)
            .expect("expected a direct-step effect row for escape continuation binder");

        assert_eq!(effect_row_terms(&lowered.types, row), ["a.Boom"]);
    }

    #[test]
    fn direct_step_rows_include_nested_handle_boundary_effects() {
        let lowered = lower_typed_single_source(
            r#"
package a

import scoop.core.*

effect Ask {
    fun current(): Int
}

effect Yield {
    fun next(): Int
}

effect Boom {
    fun boom(code: Int): Int
}

fun demo(): Int / (Boom) {
    return handle {
        val seed: Int = Ask.current()
        val nested: Int = handle {
            Yield.next()
        } with {
            Raise.raise(err: RuntimeError) -> 0
        }
        seed + nested
    } with {
        Ask.current(), k -> 7
        Yield.next() , k -> {
            val _: Int = Boom.boom(11)
            k.resume(5)
        }
    }
}
"#,
        );
        let handle = first_handle_in_file(&lowered.file)
            .map(|(_, handle)| handle)
            .expect("expected a handle");
        let continuation = only_escape_continuation_symbol(handle);

        let rows =
            compute_escape_continuation_direct_step_effect_rows_for_handle(&lowered.types, handle);
        let row = rows
            .get(&continuation)
            .expect("expected a direct-step effect row for escape continuation binder");

        assert_eq!(effect_row_terms(&lowered.types, row), ["a.Boom"]);
    }

    #[test]
    fn direct_step_rows_include_hidden_top_level_once_init_effects() {
        let lowered = lower_typed_single_source(
            r#"
package a

import scoop.core.*

effect Ask {
    fun current(): Int
}

effect Boom {
    fun boom(code: Int): Int
}

val hidden: Int = Boom.boom(13)

fun demo(): Int / (Boom) {
    return handle {
        val seed: Int = Ask.current()
        seed + hidden
    } with {
        Ask.current(), k -> 7
    }
}
"#,
        );
        let handle = first_handle_in_file(&lowered.file)
            .map(|(_, handle)| handle)
            .expect("expected a handle");
        let continuation = only_escape_continuation_symbol(handle);

        let rows = compute_escape_continuation_direct_step_effect_rows_for_handle_with_program(
            &lowered.types,
            handle,
            Some(direct_step_program_info(&lowered)),
        );
        let row = rows
            .get(&continuation)
            .expect("expected a direct-step effect row for escape continuation binder");

        assert_eq!(effect_row_terms(&lowered.types, row), ["a.Boom"]);
    }

    fn effect_row_terms(types: &TypeStore, row: &EffectRow) -> Vec<String> {
        row.terms
            .iter()
            .map(|ty| types.display(*ty).to_string())
            .collect()
    }

    fn direct_step_program_info(lowered: &hir::LoweredHir) -> DirectStepProgramInfo<'_> {
        DirectStepProgramInfo {
            object_inits: &lowered.object_inits,
            top_level_immutable_values: &lowered.top_level_immutable_values,
        }
    }

    fn only_escape_continuation_symbol(handle: &hir::HandleExpr) -> hir::SymbolId {
        handle
            .arms
            .iter()
            .find_map(|arm| match arm.kind {
                hir::HandleArmKind::EscapeContinuation { continuation } => Some(continuation),
                hir::HandleArmKind::NonResuming => None,
            })
            .expect("expected an escape continuation arm")
    }

    fn state_action_source_snippet(op: &HandleStateOp, source: &SourceFile) -> Option<String> {
        match op {
            HandleStateOp::StmtEmpty { stmt }
            | HandleStateOp::Assign { stmt }
            | HandleStateOp::Break { stmt }
            | HandleStateOp::Continue { stmt }
            | HandleStateOp::Return { stmt }
            | HandleStateOp::TodoStmt { stmt, .. }
            | HandleStateOp::WhileCondHeader { stmt } => Some(source.slice(stmt.span).to_string()),
            HandleStateOp::BindLocal { decl, .. }
            | HandleStateOp::DeclareAnonymousVal { decl, .. } => decl
                .init
                .as_ref()
                .map(|init| source.slice(init.span).to_string()),
            HandleStateOp::ExprMissing { expr }
            | HandleStateOp::Literal { expr }
            | HandleStateOp::ReadLocal { expr, .. }
            | HandleStateOp::ObjectInitAccessBoundary { expr, .. }
            | HandleStateOp::VarRef { expr }
            | HandleStateOp::StructLit { expr }
            | HandleStateOp::TupleLit { expr }
            | HandleStateOp::InterpolatedString { expr }
            | HandleStateOp::Expr { expr }
            | HandleStateOp::RuntimeRaiseBoundary { expr, .. }
            | HandleStateOp::BinaryExpr { expr }
            | HandleStateOp::WhenExpr { expr }
            | HandleStateOp::SuspendCall { expr, .. }
            | HandleStateOp::Call { expr }
            | HandleStateOp::Perform { expr, .. }
            | HandleStateOp::NestedHandleBoundary { expr, .. }
            | HandleStateOp::NestedHandle { expr, .. }
            | HandleStateOp::Closure { expr }
            | HandleStateOp::TodoExpr { expr, .. } => Some(source.slice(expr.span).to_string()),
            HandleStateOp::ResumeAfterSite { source_span, .. } => {
                Some(source.slice(*source_span).to_string())
            }
            HandleStateOp::ImplicitElseUnit { span } => Some(source.slice(*span).to_string()),
            HandleStateOp::CleanupEdgeComplete
            | HandleStateOp::ReturnToEnclosingExpression
            | HandleStateOp::LoopReentry { .. }
            | HandleStateOp::ExecuteArmBody { .. } => None,
        }
    }

    fn lower_typed_single_source(source_text: &str) -> hir::LoweredHir {
        let session = legacy_session();
        let source = SourceFile::new_virtual("<mem>", source_text);
        let mut ast = parse_file(&source).expect("parse");

        let index = {
            let mut pairs: Vec<(&SourceFile, &crate::ast::File)> = Vec::new();
            for file in &session.sysroot().files {
                pairs.push((&file.source, &file.ast));
            }
            pairs.push((&source, &ast));
            Index::build(&pairs).expect("index")
        };

        let headers =
            crate::resolve::check_file_headers(&source, &ast, &index).expect("resolve headers");
        crate::resolve::check_file_bodies(&source, &mut ast, &index, &headers)
            .expect("resolve bodies");

        let mut typecheck_types = TypeStore::new();
        let builtins = typecheck_types.intern_builtins();
        let mut env = typecheck::TypeEnv::from_sysroot(session.sysroot(), &index).expect("env");
        env.extend_from_file(&source, &ast, &index)
            .expect("extend type env");

        typecheck::check_file_annotations(
            &source,
            &ast,
            &index,
            &headers.imports,
            &env,
            &mut typecheck_types,
            builtins,
        )
        .expect("check annotations");
        typecheck::check_file_type_refs(
            &source,
            &ast,
            &index,
            &headers.imports,
            &env,
            &mut typecheck_types,
            builtins,
        )
        .expect("check type refs");
        typecheck::check_file_exprs(
            &source,
            &ast,
            &index,
            &headers.imports,
            &env,
            &mut typecheck_types,
            builtins,
        )
        .expect("check exprs");

        let mut unit: Vec<(&SourceFile, &crate::ast::File)> = Vec::new();
        for file in &session.sysroot().files {
            unit.push((&file.source, &file.ast));
        }
        unit.push((&source, &ast));

        hir::lower_for_compilation_unit_multi_files(
            &source,
            &index,
            &unit,
            &[(&source, &ast)],
            &[],
            &typecheck_types,
        )
        .expect("lower")
    }

    fn legacy_session() -> Session {
        Session::with_options(SessionOptions::new(EffectPipelineMode::Legacy)).expect("session")
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
            hir::ExprKind::When { subject, arms } => first_handle_in_expr(subject).or_else(|| {
                arms.iter()
                    .find_map(|arm| arm.guard.as_ref().and_then(first_handle_in_expr))
                    .or_else(|| arms.iter().find_map(|arm| first_handle_in_expr(&arm.body)))
            }),
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
                let hir::InterpolatedStringPart::Expr { expr } = part else {
                    return None;
                };
                first_handle_in_expr(expr)
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
}
