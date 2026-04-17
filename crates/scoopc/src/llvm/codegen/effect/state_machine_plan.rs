use crate::ast;
use std::cell::Cell;
use std::collections::{HashMap, HashSet};

use crate::hir;
use crate::span::Span;
use crate::ty::{RefTypeKind, TypeId, TypeKind, TypeStore};

type PlanStateId = u32;
type SuspendSiteId = u32;
type ArmPlanId = u32;
type CleanupScopeId = u32;

#[derive(Debug, Clone, Default)]
struct HandlePlanContext {
    known_fun_effects: HashMap<String, bool>,
    known_local_fun_effects: HashMap<hir::SymbolId, bool>,
    known_local_metadata: HashMap<hir::SymbolId, KnownLocalMetadata>,
    next_synthetic_symbol_raw: Cell<u32>,
    ctor_call_targets: HashMap<Span, Vec<String>>,
    continuation_resume_call_sites: HashSet<Span>,
    object_value_fqns: HashSet<String>,
    object_property_fqns: HashSet<String>,
}

#[derive(Debug, Clone, Copy)]
struct KnownLocalMetadata {
    ty: TypeId,
    mutable: bool,
}

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
            out.push_str(&format!(
                "{pad}  {} => [{}]\n",
                entry.op_fqn,
                arm_ids
            ));
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
                let available = render_symbol_list(&site.available_locals, &self.frame_layout.slots);
                let captures = render_symbol_list(&site.capture_locals, &self.frame_layout.slots);
                let matching = site
                    .matching_arms
                    .iter()
                    .map(|id| format!("arm{id}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                out.push_str(&format!(
                    "{pad}  site{} kind={} span={:?} resume=s{} arms=[{}]\n",
                    site.id,
                    site.kind.label(),
                    site.span,
                    site.resume_target,
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
    },
    DeclareAnonymousVal {
        decl: Box<hir::ValDecl>,
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
    LoopReentry { cond_state: PlanStateId },
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
            StateTerminator::CleanupEnter { scope_id, next_state } => {
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
            StateTerminator::CleanupEnter { scope_id, next_state } => {
                0x2000 ^ (*scope_id as usize) ^ ((*next_state as usize) << 1)
            }
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
            HandleStateOp::ExecuteArmBody { op_fqn, arm, .. } => {
                format!("execute arm body op={op_fqn} span={:?}", arm.body.span)
            }
        }
    }

    fn structural_signature(&self) -> usize {
        match self {
            HandleStateOp::StmtEmpty { stmt } => 1 ^ stmt_payload_signature(stmt),
            HandleStateOp::BindLocal { id, decl } => {
                2 ^ (id.as_u32() as usize) ^ decl_payload_signature(decl)
            }
            HandleStateOp::DeclareAnonymousVal { decl } => 3 ^ decl_payload_signature(decl),
            HandleStateOp::Assign { stmt } => 4 ^ stmt_payload_signature(stmt),
            HandleStateOp::Break { stmt } => 5 ^ stmt_payload_signature(stmt),
            HandleStateOp::Continue { stmt } => 6 ^ stmt_payload_signature(stmt),
            HandleStateOp::Return { stmt } => 7 ^ stmt_payload_signature(stmt),
            HandleStateOp::TodoStmt { stmt, kind } => {
                8 ^ stmt_payload_signature(stmt) ^ kind.len()
            }
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
            } => {
                30 ^ (*site_id as usize) ^ *nested_id ^ expr_payload_signature(expr)
            }
            HandleStateOp::NestedHandle { nested_id, expr } => {
                31 ^ *nested_id ^ expr_payload_signature(expr)
            }
            HandleStateOp::Closure { expr } => 32 ^ expr_payload_signature(expr),
            HandleStateOp::TodoExpr { expr, kind } => {
                33 ^ kind.len() ^ expr_payload_signature(expr)
            }
            HandleStateOp::ExecuteArmBody { arm_id, op_fqn, arm } => {
                34 ^ (*arm_id as usize) ^ op_fqn.len() ^ handle_arm_payload_signature(arm)
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
            HandleBranchCondition::WhileCond { condition } => {
                1 ^ expr_payload_signature(condition)
            }
            HandleBranchCondition::IfCond { condition } => 2 ^ expr_payload_signature(condition),
        }
    }
}

#[derive(Debug, Clone)]
struct SuspendSitePlan {
    id: SuspendSiteId,
    span: Span,
    kind: SuspendSiteKind,
    resume_target: PlanStateId,
    matching_arms: Vec<ArmPlanId>,
    available_locals: Vec<hir::SymbolId>,
    capture_locals: Vec<hir::SymbolId>,
    source_path: Option<SuspendSourcePath>,
    resume_path: Option<SuspendResumePath>,
}

/// statement-position `val` 绑定 suspend site 在 `handle` body 中的源码路径。
///
/// 该路径只描述源码中的语句位置与嵌套控制流层级，供统一 state-machine
/// 构建、重建与验证阶段使用。
#[derive(Debug, Clone)]
struct SuspendSourcePath {
    top_level_stmt_idx: usize,
    frames: Vec<SuspendSourceFramePath>,
}

impl SuspendSourcePath {
    #[cfg(test)]
    fn label(&self) -> String {
        let mut parts = vec![format!("top[{}]", self.top_level_stmt_idx)];
        parts.extend(self.frames.iter().map(SuspendSourceFramePath::label));
        parts.join(" -> ")
    }

    fn structural_signature(&self) -> usize {
        let mut acc = self.top_level_stmt_idx;
        for frame in &self.frames {
            acc ^= frame.structural_signature();
        }
        acc
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
    CallCallee { call_span: Span },
    CallArg { call_span: Span, arg_index: usize },
    NamedArgValue {
        call_span: Span,
        arg_index: usize,
        name_span: Span,
    },
    PerformArg { perform_span: Span, arg_index: usize },
    MemberReceiver { access_span: Span },
    BinaryLhs { binary_span: Span },
    BinaryRhs { binary_span: Span },
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
    UnaryOperand { expr_span: Span },
    CastOperand { expr_span: Span },
    TypeCheckOperand { expr_span: Span },
    IfCond { if_span: Span },
    IfThenExpr { if_span: Span },
    IfElseExpr { if_span: Span },
    WhenSubject { when_span: Span },
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
            SuspendResumeExprFrame::TypeCheckOperand { .. } => {
                "typecheck-operand".to_string()
            }
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
                arm_index, stmt_idx, ..
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
            SuspendSourceFramePath::Block { block_span, stmt_idx } => {
                0x101 ^ block_span.start ^ (block_span.end << 1) ^ stmt_idx
            }
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
            | SuspendSiteKind::ClassCtorInit { class_name: op_fqn }
            | SuspendSiteKind::NestedHandleBoundary { detail: op_fqn } => {
                Some(op_fqn.clone())
            }
        }
    }

    fn structural_signature(&self) -> usize {
        match self {
            SuspendSiteKind::Perform { op_fqn } => 0x11 ^ op_fqn.len(),
            SuspendSiteKind::CallMaySuspend { callee } => 0x22 ^ callee.len(),
            SuspendSiteKind::CallStateMachineCallee { callee } => 0x33 ^ callee.len(),
            SuspendSiteKind::RuntimeRaise { reason } => 0x44 ^ reason.len(),
            SuspendSiteKind::ObjectInitAccess { target } => 0x55 ^ target.len(),
            SuspendSiteKind::ClassCtorInit { class_name } => 0x66 ^ class_name.len(),
            SuspendSiteKind::NestedHandleBoundary { detail } => 0x77 ^ detail.len(),
        }
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
            ^ self.resume_target as usize
            ^ self.kind.structural_signature();
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
}

impl ArmPlan {
    fn structural_signature(&self) -> usize {
        let mut acc = self.id as usize
            ^ self.op_fqn.len()
            ^ self.effect_ty.as_u32() as usize
            ^ self.body_entry_state as usize;
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
        self.entries
            .iter()
            .fold(self.entries.len(), |acc, entry| acc ^ entry.structural_signature())
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
}

impl<'a, 'hir> HandlePlanBuilder<'a, 'hir> {
    fn local_function_value_may_suspend_when_called(&self, expr: &hir::Expr) -> bool {
        SuspendCallAnalysis {
            types: self.types,
            known_fun_effects: &self.context.known_fun_effects,
            ctor_call_targets: &self.context.ctor_call_targets,
            continuation_resume_call_sites: &self.context.continuation_resume_call_sites,
            object_value_fqns: &self.context.object_value_fqns,
            object_property_fqns: &self.context.object_property_fqns,
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
        let may_suspend = function_ty_may_suspend(self.types, decl.ty)
            || decl
                .init
                .as_ref()
                .is_some_and(|expr| self.local_function_value_may_suspend_when_called(expr));
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
        let may_suspend = function_ty_may_suspend(self.types, rhs.ty)
            || self.local_function_value_may_suspend_when_called(rhs);
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
        }
    }

    fn build(mut self) -> HandleStateMachinePlan {
        let outer_slots = collect_outer_scope_slots(self.handle, &self.context.known_local_metadata);
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
                let mut state = current_state;
                if let Some(init) = decl.init.as_ref() {
                    state = self.build_expr_for_consumer(init, state, env);
                }
                self.record_local_fun_binding_if_needed(decl);
                if let Some(id) = decl.id {
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
                    // Declarations are the authoritative source of slot
                    // metadata. If an earlier fallback path pre-seeded this
                    // symbol as immutable / outer-scope, overwrite it here.
                    self.frame_slots.insert(id, slot.clone());
                    env.push(slot.clone());
                    self.push_action(
                        state,
                        HandleStateOp::BindLocal {
                            id,
                            decl: Box::new(decl.clone()),
                        },
                    );
                } else {
                    self.push_action(
                        state,
                        HandleStateOp::DeclareAnonymousVal {
                            decl: Box::new(decl.clone()),
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
            hir::StmtKind::While { cond, body } => self.build_while(stmt, cond, body, current_state, env),
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
                        self.new_suspend_site(expr.span, kind, env.available_ids());
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
            hir::ExprKind::Cast { expr: inner, op, .. } => {
                let state = self.build_expr_if_suspend_subtree(inner, current_state, env);
                if matches!(op, ast::CastOp::As) {
                    self.record_expr_reads(state, expr);
                    let site_id = self.new_suspend_site(
                        expr.span,
                        SuspendSiteKind::RuntimeRaise {
                            reason: "ClassCastFailed".to_string(),
                        },
                        env.available_ids(),
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
                    let site_id = self.new_suspend_site(expr.span, kind, env.available_ids());
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
                    let site_id = self.new_suspend_site(expr.span, kind, env.available_ids());
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
                let nested = HandleStateMachinePlan::build_with_context(self.types, handle, self.context);
                let nested_may_suspend = nested.contains_suspend_subtree();
                self.nested_handles.push(nested);
                if nested_may_suspend {
                    self.record_expr_reads(current_state, expr);
                    let site_id = self.new_suspend_site(
                        expr.span,
                        SuspendSiteKind::NestedHandleBoundary {
                            detail: format!("nested#{nested_id}"),
                        },
                        env.available_ids(),
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
            | hir::ExprKind::VarRef(_)
            | hir::ExprKind::UnresolvedIdent { .. }
            | hir::ExprKind::Closure(_)
            | hir::ExprKind::Todo(_) => false,
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
            hir::ExprKind::Cast { expr: inner, op, .. } => {
                matches!(op, ast::CastOp::As) || self.expr_contains_suspend_subtree(inner)
            }
            hir::ExprKind::MemberAccess { receiver, member } => {
                self.expr_contains_suspend_subtree(receiver)
                    || self.classify_hidden_suspend_member_access(member).is_some()
            }
            hir::ExprKind::Binary { lhs, rhs, .. } => {
                self.expr_contains_suspend_subtree(lhs)
                    || self.expr_contains_suspend_subtree(rhs)
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
            hir::ExprKind::Handle(handle) => self.handle_contains_suspend_subtree(handle),
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
        block.stmts
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
                self.expr_contains_suspend_subtree(lhs)
                    || self.expr_contains_suspend_subtree(rhs)
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

        if let Some(targets) = self.context.ctor_call_targets.get(&callee.span) {
            let mut stable_targets = targets.clone();
            stable_targets.sort();
            stable_targets.dedup();
            let class_name = if stable_targets.is_empty() {
                format!("ctor@{:?}", callee.span)
            } else {
                stable_targets.join("|")
            };
            return Some(SuspendSiteKind::ClassCtorInit { class_name });
        }

        if let TypeKind::Ref(RefTypeKind::Function(fun_ty)) = self.types.kind(callee.ty) {
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
        // `Continuation.resume` 的 builtin 语义只来自上游 typecheck 已确认的调用点 side table；
        // segmentation 本身不再按成员名、receiver 类型或其它代码形状做推断。
        self.context
            .continuation_resume_call_sites
            .contains(&call_span)
            .then(|| SuspendSiteKind::RuntimeRaise {
                reason: "Continuation.resume".to_string(),
            })
    }

    fn classify_hidden_suspend_var_ref(
        &self,
        value_ref: &hir::ValueRef,
    ) -> Option<SuspendSiteKind> {
        let hir::ValueRef::TopLevel { fqn, .. } = value_ref else {
            return None;
        };
        self.context
            .object_value_fqns
            .contains(fqn)
            .then(|| SuspendSiteKind::ObjectInitAccess {
                target: fqn.clone(),
            })
    }

    fn classify_hidden_suspend_member_access(
        &self,
        member: &hir::MemberAccess,
    ) -> Option<SuspendSiteKind> {
        let hir::MemberRef::Value { fqn, .. } = member.resolved.as_ref()? else {
            return None;
        };
        (self.context.object_value_fqns.contains(fqn)
            || self.context.object_property_fqns.contains(fqn))
        .then(|| SuspendSiteKind::ObjectInitAccess {
            target: fqn.clone(),
        })
    }

    fn build_dispatch_plan(&self) -> DispatchPlan {
        let mut by_op: HashMap<String, Vec<ArmPlanId>> = HashMap::new();
        for (idx, arm) in self.handle.arms.iter().enumerate() {
            by_op.entry(arm.op.op.fqn.clone()).or_default().push(idx as u32);
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
                hir::HandleArmKind::ImmediateResume { resume } => {
                    declared.insert(resume);
                }
                hir::HandleArmKind::EscapeContinuation { continuation } => {
                    declared.insert(continuation);
                }
            }
            collect_declared_local_ids_in_expr(&arm.body, &mut declared);

            let mut used = HashMap::new();
            collect_local_refs_in_expr(&arm.body, &mut used);
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

            let body_entry_state = self.new_state(format!("arm{arm_id}.body"));
            self.push_action(
                body_entry_state,
                HandleStateOp::ExecuteArmBody {
                    arm_id,
                    op_fqn: arm.op.op.fqn.clone(),
                    arm: Box::new(arm.clone()),
                },
            );

            let arm_exit = match arm.kind {
                hir::HandleArmKind::NonResuming => ArmBodyExit::ReturnHandle,
                hir::HandleArmKind::ImmediateResume { .. } => ArmBodyExit::ResumeMatchedSite,
                hir::HandleArmKind::EscapeContinuation { .. } => {
                    ArmBodyExit::MaterializeContinuation
                }
            };
            self.set_terminator(body_entry_state, StateTerminator::ArmExit(arm_exit));

            self.arm_plans.push(ArmPlan {
                id: arm_id,
                op_fqn: arm.op.op.fqn.clone(),
                effect_ty: arm.op.effect_ty,
                binder_slots,
                capture_locals,
                body_entry_state,
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
            let reachable = reachable_states(site.resume_target, &successors);
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
        for (top_level_stmt_idx, stmt) in self.handle.body.stmts.iter().enumerate() {
            self.attach_suspend_source_paths_in_stmt(stmt, top_level_stmt_idx, &mut path);
        }
    }

    fn attach_suspend_source_paths_in_stmt(
        &mut self,
        stmt: &'hir hir::Stmt,
        top_level_stmt_idx: usize,
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
                self.attach_suspend_source_paths_in_expr(init, top_level_stmt_idx, path);
            }
            hir::StmtKind::Expr(expr) => {
                self.attach_suspend_source_paths_in_expr(expr, top_level_stmt_idx, path);
            }
            hir::StmtKind::Assign { lhs, rhs, .. } => {
                self.attach_suspend_source_paths_in_expr(lhs, top_level_stmt_idx, path);
                self.attach_suspend_source_paths_in_expr(rhs, top_level_stmt_idx, path);
            }
            hir::StmtKind::Return { value } => {
                if let Some(value) = value.as_ref() {
                    self.attach_suspend_source_paths_in_expr(value, top_level_stmt_idx, path);
                }
            }
            hir::StmtKind::While { cond, body } => {
                self.attach_suspend_source_paths_in_expr(cond, top_level_stmt_idx, path);
                for (stmt_idx, body_stmt) in body.stmts.iter().enumerate() {
                    path.push(SuspendSourceFramePath::WhileBody {
                        while_cond_span: cond.span,
                        while_body_span: body.span,
                        stmt_idx,
                    });
                    self.attach_suspend_source_paths_in_stmt(
                        body_stmt,
                        top_level_stmt_idx,
                        path,
                    );
                    let _ = path.pop();
                }
            }
        }
    }

    fn attach_suspend_source_paths_in_expr(
        &mut self,
        expr: &'hir hir::Expr,
        top_level_stmt_idx: usize,
        path: &mut Vec<SuspendSourceFramePath>,
    ) {
        match &expr.kind {
            hir::ExprKind::Missing
            | hir::ExprKind::Literal(_)
            | hir::ExprKind::VarRef(_)
            | hir::ExprKind::UnresolvedIdent { .. }
            | hir::ExprKind::Closure(_)
            | hir::ExprKind::Handle(_)
            | hir::ExprKind::Todo(_) => {}
            hir::ExprKind::StructLit { fields, .. } => {
                for field in fields {
                    self.attach_suspend_source_paths_in_expr(
                        &field.value,
                        top_level_stmt_idx,
                        path,
                    );
                }
            }
            hir::ExprKind::TupleLit { elements } => {
                for element in elements {
                    self.attach_suspend_source_paths_in_expr(element, top_level_stmt_idx, path);
                }
            }
            hir::ExprKind::InterpolatedString { parts, .. } => {
                for part in parts {
                    let hir::InterpolatedStringPart::Expr { expr: part_expr } = part else {
                        continue;
                    };
                    self.attach_suspend_source_paths_in_expr(
                        part_expr,
                        top_level_stmt_idx,
                        path,
                    );
                }
            }
            hir::ExprKind::Unary { expr: inner, .. }
            | hir::ExprKind::TypeCheck { expr: inner, .. }
            | hir::ExprKind::Cast { expr: inner, .. } => {
                self.attach_suspend_source_paths_in_expr(inner, top_level_stmt_idx, path);
            }
            hir::ExprKind::Binary { lhs, rhs, .. } => {
                self.attach_suspend_source_paths_in_expr(lhs, top_level_stmt_idx, path);
                self.attach_suspend_source_paths_in_expr(rhs, top_level_stmt_idx, path);
            }
            hir::ExprKind::Block(block) => {
                for (stmt_idx, stmt) in block.stmts.iter().enumerate() {
                    path.push(SuspendSourceFramePath::Block {
                        block_span: block.span,
                        stmt_idx,
                    });
                    self.attach_suspend_source_paths_in_stmt(stmt, top_level_stmt_idx, path);
                    let _ = path.pop();
                }
            }
            hir::ExprKind::If {
                cond,
                then_branch,
                else_branch,
                ..
            } => {
                self.attach_suspend_source_paths_in_expr(cond, top_level_stmt_idx, path);
                if let hir::ExprKind::Block(block) = &then_branch.kind {
                    for (stmt_idx, stmt) in block.stmts.iter().enumerate() {
                        path.push(SuspendSourceFramePath::IfThen {
                            if_span: expr.span,
                            then_span: block.span,
                            stmt_idx,
                        });
                        self.attach_suspend_source_paths_in_stmt(
                            stmt,
                            top_level_stmt_idx,
                            path,
                        );
                        let _ = path.pop();
                    }
                } else {
                    self.attach_suspend_source_paths_in_expr(
                        then_branch,
                        top_level_stmt_idx,
                        path,
                    );
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
                        self.attach_suspend_source_paths_in_stmt(
                            stmt,
                            top_level_stmt_idx,
                            path,
                        );
                        let _ = path.pop();
                    }
                } else if let Some(else_expr) = else_branch.as_deref() {
                    self.attach_suspend_source_paths_in_expr(
                        else_expr,
                        top_level_stmt_idx,
                        path,
                    );
                }
            }
            hir::ExprKind::When { subject, arms } => {
                self.attach_suspend_source_paths_in_expr(subject, top_level_stmt_idx, path);
                for (arm_index, when_arm) in arms.iter().enumerate() {
                    if let Some(guard) = when_arm.guard.as_ref() {
                        self.attach_suspend_source_paths_in_expr(
                            guard,
                            top_level_stmt_idx,
                            path,
                        );
                    }
                    if let hir::ExprKind::Block(block) = &when_arm.body.kind {
                        for (stmt_idx, stmt) in block.stmts.iter().enumerate() {
                            path.push(SuspendSourceFramePath::WhenArm {
                                when_span: expr.span,
                                arm_index,
                                arm_span: block.span,
                                stmt_idx,
                            });
                            self.attach_suspend_source_paths_in_stmt(
                                stmt,
                                top_level_stmt_idx,
                                path,
                            );
                            let _ = path.pop();
                        }
                    } else {
                        self.attach_suspend_source_paths_in_expr(
                            &when_arm.body,
                            top_level_stmt_idx,
                            path,
                        );
                    }
                }
            }
            hir::ExprKind::MemberAccess { receiver, .. } => {
                self.attach_suspend_source_paths_in_expr(receiver, top_level_stmt_idx, path);
            }
            hir::ExprKind::Call { callee, args } => {
                self.record_suspend_source_path(expr, top_level_stmt_idx, path);
                self.attach_suspend_source_paths_in_expr(callee, top_level_stmt_idx, path);
                for arg in args {
                    match arg {
                        hir::CallArg::Positional(arg_expr) => self
                            .attach_suspend_source_paths_in_expr(
                                arg_expr,
                                top_level_stmt_idx,
                                path,
                            ),
                        hir::CallArg::Named { value, .. } => self
                            .attach_suspend_source_paths_in_expr(
                                value,
                                top_level_stmt_idx,
                                path,
                            ),
                    }
                }
            }
            hir::ExprKind::Perform { args, .. } => {
                self.record_suspend_source_path(expr, top_level_stmt_idx, path);
                for arg in args {
                    match arg {
                        hir::CallArg::Positional(arg_expr) => self
                            .attach_suspend_source_paths_in_expr(
                                arg_expr,
                                top_level_stmt_idx,
                                path,
                            ),
                        hir::CallArg::Named { value, .. } => self
                            .attach_suspend_source_paths_in_expr(
                                value,
                                top_level_stmt_idx,
                                path,
                            ),
                    }
                }
            }
        }
    }

    fn record_suspend_source_path(
        &mut self,
        expr: &'hir hir::Expr,
        top_level_stmt_idx: usize,
        path: &[SuspendSourceFramePath],
    ) {
        let Some(site) = self.suspend_sites.iter_mut().find(|site| {
            let kind_matches = matches!(
                (&site.kind, &expr.kind),
                (SuspendSiteKind::Perform { .. }, hir::ExprKind::Perform { .. })
                    | (
                        SuspendSiteKind::CallMaySuspend { .. },
                        hir::ExprKind::Call { .. },
                    )
                    | (
                        SuspendSiteKind::CallStateMachineCallee { .. },
                        hir::ExprKind::Call { .. },
                    )
            );
            kind_matches && site.span == expr.span && site.source_path.is_none()
        }) else {
            return;
        };
        site.source_path = Some(SuspendSourcePath {
            top_level_stmt_idx,
            frames: path.to_vec(),
        });
    }

    fn attach_suspend_resume_paths(&mut self) {
        for stmt in &self.handle.body.stmts {
            self.attach_suspend_resume_paths_in_stmt(stmt);
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
            let kind_matches = matches!(
                (&site.kind, &expr.kind),
                (SuspendSiteKind::Perform { .. }, hir::ExprKind::Perform { .. })
                    | (
                        SuspendSiteKind::CallMaySuspend { .. }
                            | SuspendSiteKind::CallStateMachineCallee { .. }
                            | SuspendSiteKind::ClassCtorInit { .. },
                        hir::ExprKind::Call { .. },
                    )
                    | (
                        SuspendSiteKind::NestedHandleBoundary { .. },
                        hir::ExprKind::Handle(_),
                    )
            );
            kind_matches && site.span == expr.span && site.resume_path.is_none()
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

    fn record_resume_source_expr(
        &mut self,
        site_id: SuspendSiteId,
        source_expr: &'hir hir::Expr,
    ) {
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

        for state in &mut self.states {
            let rewrites = state
                .actions
                .iter()
                .enumerate()
                .filter_map(|(op_index, op)| match op {
                    HandleStateOp::ResumeAfterSite {
                        site_id,
                        resume_slot: Some(resume_slot),
                        ..
                    } => resume_paths
                        .get(site_id)
                        .cloned()
                        .map(|resume_path| {
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
                                source_expr,
                                resume_path,
                                resume_slot.clone(),
                            )
                        }),
                    _ => None,
                })
                .collect::<Vec<_>>();

            for (op_index, source_expr, resume_path, resume_slot) in rewrites {
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
    ) -> SuspendSiteId {
        let id = self.next_site_id;
        self.next_site_id = self.next_site_id.saturating_add(1);
        self.suspend_sites.push(SuspendSitePlan {
            id,
            span,
            kind,
            resume_target: 0,
            matching_arms: Vec::new(),
            available_locals,
            capture_locals: Vec::new(),
            source_path: None,
            resume_path: None,
        });
        id
    }

    fn set_suspend_resume_target(&mut self, site_id: SuspendSiteId, resume_target: PlanStateId) {
        let site = self
            .suspend_sites
            .iter_mut()
            .find(|site| site.id == site_id)
            .expect("site should exist");
        site.resume_target = resume_target;
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
        HandleStateOp::BindLocal { decl, .. } | HandleStateOp::DeclareAnonymousVal { decl } => {
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
    build_resume_tail_block_from_stmt_slice(
        body,
        source_path.top_level_stmt_idx,
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
    let mut tail_stmts = block
        .stmts
        .iter()
        .skip(start_idx)
        .cloned()
        .collect::<Vec<_>>();
    tail_stmts[0] = build_resume_tail_stmt(
        first_stmt,
        frames,
        source_expr,
        resume_path,
        resume_slot,
        allocate_synthetic_symbol_id,
    )?;

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
        hir::ExprKind::Binary { lhs, op, op_span, rhs } => {
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
        hir::ExprKind::Perform { effect_ty, op, args } => {
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

    let resume_first_var =
        make_local_var_expr(original_stmt.span, bool_ty, resume_first_id, &resume_first_name);
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
            **callee = rewrite_expr_from_resume_path(
                callee,
                source_expr,
                &expr_frames[1..],
                resume_slot,
            );
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
            if let Some(hir::CallArg::Named { name_span: arg_name_span, value, .. }) =
                args.get_mut(*arg_index)
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
        (
            SuspendResumeExprFrame::BinaryLhs { binary_span },
            hir::ExprKind::Binary { lhs, .. },
        ) if rewritten.span == *binary_span => {
            **lhs = rewrite_expr_from_resume_path(
                lhs,
                source_expr,
                &expr_frames[1..],
                resume_slot,
            );
        }
        (
            SuspendResumeExprFrame::BinaryRhs { binary_span },
            hir::ExprKind::Binary { rhs, .. },
        ) if rewritten.span == *binary_span => {
            **rhs = rewrite_expr_from_resume_path(
                rhs,
                source_expr,
                &expr_frames[1..],
                resume_slot,
            );
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
            **inner = rewrite_expr_from_resume_path(
                inner,
                source_expr,
                &expr_frames[1..],
                resume_slot,
            );
        }
        (
            SuspendResumeExprFrame::CastOperand { expr_span },
            hir::ExprKind::Cast { expr: inner, .. },
        ) if rewritten.span == *expr_span => {
            **inner = rewrite_expr_from_resume_path(
                inner,
                source_expr,
                &expr_frames[1..],
                resume_slot,
            );
        }
        (
            SuspendResumeExprFrame::TypeCheckOperand { expr_span },
            hir::ExprKind::TypeCheck { expr: inner, .. },
        ) if rewritten.span == *expr_span => {
            **inner = rewrite_expr_from_resume_path(
                inner,
                source_expr,
                &expr_frames[1..],
                resume_slot,
            );
        }
        (
            SuspendResumeExprFrame::IfCond { if_span },
            hir::ExprKind::If { cond, .. },
        ) if rewritten.span == *if_span => {
            **cond = rewrite_expr_from_resume_path(
                cond,
                source_expr,
                &expr_frames[1..],
                resume_slot,
            );
        }
        (
            SuspendResumeExprFrame::IfThenExpr { if_span },
            hir::ExprKind::If {
                then_branch, ..
            },
        ) if rewritten.span == *if_span => {
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
            **subject = rewrite_expr_from_resume_path(
                subject,
                source_expr,
                &expr_frames[1..],
                resume_slot,
            );
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
        | (
            SuspendResumeExprFrame::NamedArgValue { call_span, .. },
            hir::ExprKind::Call { .. },
        ) => expr.span == *call_span,
        (SuspendResumeExprFrame::PerformArg { perform_span, .. }, hir::ExprKind::Perform { .. }) => {
            expr.span == *perform_span
        }
        (
            SuspendResumeExprFrame::MemberReceiver { access_span },
            hir::ExprKind::MemberAccess { .. },
        ) => expr.span == *access_span,
        (SuspendResumeExprFrame::BinaryLhs { binary_span }, hir::ExprKind::Binary { .. })
        | (SuspendResumeExprFrame::BinaryRhs { binary_span }, hir::ExprKind::Binary { .. }) => {
            expr.span == *binary_span
        }
        (SuspendResumeExprFrame::StructField { struct_span, .. }, hir::ExprKind::StructLit { .. }) => {
            expr.span == *struct_span
        }
        (SuspendResumeExprFrame::TupleElement { tuple_span, .. }, hir::ExprKind::TupleLit { .. }) => {
            expr.span == *tuple_span
        }
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
            hir::HandleArmKind::ImmediateResume { resume } => {
                ids.insert(resume);
            }
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
            hir::HandleArmKind::ImmediateResume { resume } => {
                declared.insert(resume);
            }
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

fn collect_known_local_metadata_in_handle(
    handle: &hir::HandleExpr,
    out: &mut HashMap<hir::SymbolId, KnownLocalMetadata>,
) {
    collect_known_local_metadata_in_block(&handle.body, out);
    for arm in &handle.arms {
        collect_known_local_metadata_in_expr(&arm.body, out);
    }
    if let Some(finally_block) = handle.finally.as_ref() {
        collect_known_local_metadata_in_block(finally_block, out);
    }
}

#[cfg(test)]
fn collect_known_local_metadata_in_fun(
    fun: &hir::FunDecl,
    out: &mut HashMap<hir::SymbolId, KnownLocalMetadata>,
) {
    for param in &fun.params {
        out.insert(
            param.id,
            KnownLocalMetadata {
                ty: param.ty,
                mutable: false,
            },
        );
    }
    if let Some(body) = &fun.body {
        collect_known_local_metadata_in_block(body, out);
    }
}

fn collect_known_local_metadata_in_block(
    block: &hir::Block,
    out: &mut HashMap<hir::SymbolId, KnownLocalMetadata>,
) {
    for stmt in &block.stmts {
        collect_known_local_metadata_in_stmt(stmt, out);
    }
}

fn collect_known_local_metadata_in_stmt(
    stmt: &hir::Stmt,
    out: &mut HashMap<hir::SymbolId, KnownLocalMetadata>,
) {
    match &stmt.kind {
        hir::StmtKind::Val(decl) => {
            if let Some(id) = decl.id {
                out.insert(
                    id,
                    KnownLocalMetadata {
                        ty: decl.ty,
                        mutable: decl.mutable,
                    },
                );
            }
            if let Some(init) = decl.init.as_ref() {
                collect_known_local_metadata_in_expr(init, out);
            }
        }
        hir::StmtKind::Expr(expr) => collect_known_local_metadata_in_expr(expr, out),
        hir::StmtKind::Assign { lhs, rhs, .. } => {
            collect_known_local_metadata_in_expr(lhs, out);
            collect_known_local_metadata_in_expr(rhs, out);
        }
        hir::StmtKind::While { cond, body } => {
            collect_known_local_metadata_in_expr(cond, out);
            collect_known_local_metadata_in_block(body, out);
        }
        hir::StmtKind::Return { value } => {
            if let Some(expr) = value {
                collect_known_local_metadata_in_expr(expr, out);
            }
        }
        hir::StmtKind::Empty
        | hir::StmtKind::Break { .. }
        | hir::StmtKind::Continue { .. }
        | hir::StmtKind::Todo(_) => {}
    }
}

fn collect_known_local_metadata_in_expr(
    expr: &hir::Expr,
    out: &mut HashMap<hir::SymbolId, KnownLocalMetadata>,
) {
    match &expr.kind {
        hir::ExprKind::Block(block) => collect_known_local_metadata_in_block(block, out),
        hir::ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => {
            collect_known_local_metadata_in_expr(cond, out);
            collect_known_local_metadata_in_expr(then_branch, out);
            if let Some(else_branch) = else_branch.as_deref() {
                collect_known_local_metadata_in_expr(else_branch, out);
            }
        }
        hir::ExprKind::When { subject, arms } => {
            collect_known_local_metadata_in_expr(subject, out);
            for arm in arms {
                if let Some(guard) = arm.guard.as_ref() {
                    collect_known_local_metadata_in_expr(guard, out);
                }
                collect_known_local_metadata_in_expr(&arm.body, out);
            }
        }
        hir::ExprKind::Closure(closure) => {
            for param in &closure.params {
                out.insert(
                    param.id,
                    KnownLocalMetadata {
                        ty: param.ty,
                        mutable: false,
                    },
                );
            }
            collect_known_local_metadata_in_expr(&closure.body, out);
        }
        hir::ExprKind::StructLit { fields, .. } => {
            for field in fields {
                collect_known_local_metadata_in_expr(&field.value, out);
            }
        }
        hir::ExprKind::TupleLit { elements } => {
            for element in elements {
                collect_known_local_metadata_in_expr(element, out);
            }
        }
        hir::ExprKind::InterpolatedString { parts, .. } => {
            for part in parts {
                if let hir::InterpolatedStringPart::Expr { expr } = part {
                    collect_known_local_metadata_in_expr(expr, out);
                }
            }
        }
        hir::ExprKind::Unary { expr: inner, .. }
        | hir::ExprKind::Cast { expr: inner, .. }
        | hir::ExprKind::TypeCheck { expr: inner, .. }
        | hir::ExprKind::MemberAccess {
            receiver: inner, ..
        } => collect_known_local_metadata_in_expr(inner, out),
        hir::ExprKind::Binary { lhs, rhs, .. } => {
            collect_known_local_metadata_in_expr(lhs, out);
            collect_known_local_metadata_in_expr(rhs, out);
        }
        hir::ExprKind::Call { callee, args } => {
            collect_known_local_metadata_in_expr(callee, out);
            for arg in args {
                match arg {
                    hir::CallArg::Positional(expr) => collect_known_local_metadata_in_expr(expr, out),
                    hir::CallArg::Named { value, .. } => {
                        collect_known_local_metadata_in_expr(value, out)
                    }
                }
            }
        }
        hir::ExprKind::Perform { args, .. } => {
            for arg in args {
                match arg {
                    hir::CallArg::Positional(expr) => collect_known_local_metadata_in_expr(expr, out),
                    hir::CallArg::Named { value, .. } => {
                        collect_known_local_metadata_in_expr(value, out)
                    }
                }
            }
        }
        hir::ExprKind::Handle(handle) => {
            collect_known_local_metadata_in_handle(handle, out);
        }
        hir::ExprKind::Missing
        | hir::ExprKind::Literal(_)
        | hir::ExprKind::VarRef(_)
        | hir::ExprKind::UnresolvedIdent { .. }
        | hir::ExprKind::Todo(_) => {}
    }
}

fn function_ty_may_suspend(types: &TypeStore, ty: TypeId) -> bool {
    matches!(
        types.kind(ty),
        TypeKind::Ref(RefTypeKind::Function(fun_ty)) if !fun_ty.effects.is_pure()
    )
}

fn hir_ty_is_function_value(types: &TypeStore, ty: TypeId) -> bool {
    matches!(types.kind(ty), TypeKind::Ref(RefTypeKind::Function(_)))
}

struct SuspendCallAnalysis<'a> {
    types: &'a TypeStore,
    known_fun_effects: &'a HashMap<String, bool>,
    ctor_call_targets: &'a HashMap<Span, Vec<String>>,
    continuation_resume_call_sites: &'a HashSet<Span>,
    object_value_fqns: &'a HashSet<String>,
    object_property_fqns: &'a HashSet<String>,
}

impl<'a> SuspendCallAnalysis<'a> {
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
                    let may_suspend = function_ty_may_suspend(self.types, decl.ty)
                        || decl.init.as_ref().is_some_and(|expr| {
                            self.function_value_may_suspend_when_called(expr, out)
                        });
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
                    let may_suspend = function_ty_may_suspend(self.types, rhs.ty)
                        || self.function_value_may_suspend_when_called(rhs, out);
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
        block.stmts
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
                self.expr_may_suspend(lhs, known_locals)
                    || self.expr_may_suspend(rhs, known_locals)
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
            | hir::ExprKind::Closure(_)
            | hir::ExprKind::Todo(_) => false,
            hir::ExprKind::VarRef(hir::ValueRef::TopLevel { fqn, .. }) => {
                self.object_value_fqns.contains(fqn)
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
            hir::ExprKind::Cast { expr: inner, op, .. } => {
                matches!(op, ast::CastOp::As) || self.expr_may_suspend(inner, known_locals)
            }
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
                            if self.object_value_fqns.contains(fqn)
                                || self.object_property_fqns.contains(fqn)
                    )
            }
            hir::ExprKind::Binary { lhs, rhs, .. } => {
                self.expr_may_suspend(lhs, known_locals)
                    || self.expr_may_suspend(rhs, known_locals)
            }
            hir::ExprKind::Call { callee, args } => {
                self.continuation_resume_call_sites.contains(&expr.span)
                    || self.ctor_call_targets.contains_key(&callee.span)
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
            hir::ExprKind::Handle(handle) => {
                self.block_may_suspend(&handle.body, known_locals)
                    || handle
                        .arms
                        .iter()
                        .any(|arm| self.expr_may_suspend(&arm.body, known_locals))
                    || handle
                        .finally
                        .as_ref()
                        .is_some_and(|finally| self.block_may_suspend(finally, known_locals))
            }
        }
    }

    fn function_value_may_suspend_when_called(
        &self,
        expr: &hir::Expr,
        known_locals: &HashMap<hir::SymbolId, bool>,
    ) -> bool {
        if function_ty_may_suspend(self.types, expr.ty) {
            return true;
        }
        match &expr.kind {
            hir::ExprKind::VarRef(hir::ValueRef::TopLevel { fqn, .. }) => {
                self.known_fun_effects.get(fqn).copied().unwrap_or(false)
            }
            hir::ExprKind::VarRef(hir::ValueRef::Local { id, .. }) => {
                known_locals.get(id).copied().unwrap_or(false)
            }
            hir::ExprKind::MemberAccess { member, .. } => match member.resolved.as_ref() {
                Some(hir::MemberRef::Fun { fqn, .. })
                | Some(hir::MemberRef::ExtensionFun { fqn, .. }) => {
                    self.known_fun_effects.get(fqn).copied().unwrap_or(false)
                }
                _ => false,
            },
            hir::ExprKind::Closure(closure) => {
                let mut seed_locals = known_locals.clone();
                for param in &closure.params {
                    seed_locals.insert(param.id, function_ty_may_suspend(self.types, param.ty));
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
                .is_some_and(|expr| self.function_value_may_suspend_when_called(expr, known_locals)),
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
            hir::ExprKind::When { arms, .. } => arms.iter().any(|arm| {
                self.function_value_may_suspend_when_called(&arm.body, known_locals)
            }),
            _ => false,
        }
    }
}

fn collect_known_fun_call_suspendability(
    types: &TypeStore,
    fun_index: &HashMap<String, &hir::FunDecl>,
    ctor_call_targets: &HashMap<Span, Vec<String>>,
    continuation_resume_call_sites: &HashSet<Span>,
    object_value_fqns: &HashSet<String>,
    object_property_fqns: &HashSet<String>,
) -> HashMap<String, bool> {
    let mut known_fun_effects = fun_index
        .iter()
        .map(|(fqn, fun)| (fqn.clone(), function_ty_may_suspend(types, fun.ty)))
        .collect::<HashMap<_, _>>();

    loop {
        let snapshot = known_fun_effects.clone();
        let analysis = SuspendCallAnalysis {
            types,
            known_fun_effects: &snapshot,
            ctor_call_targets,
            continuation_resume_call_sites,
            object_value_fqns,
            object_property_fqns,
        };
        let mut newly_effectful = Vec::new();
        let mut changed = false;
        for (fqn, fun) in fun_index {
            if known_fun_effects.get(fqn).copied().unwrap_or(false) {
                continue;
            }
            let Some(body) = &fun.body else {
                continue;
            };
            let seed_locals = fun
                .params
                .iter()
                .map(|param| (param.id, function_ty_may_suspend(types, param.ty)))
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
    types: &TypeStore,
    known_fun_effects: &HashMap<String, bool>,
    ctor_call_targets: &HashMap<Span, Vec<String>>,
    continuation_resume_call_sites: &HashSet<Span>,
    object_value_fqns: &HashSet<String>,
    object_property_fqns: &HashSet<String>,
) -> HashMap<hir::SymbolId, bool> {
    let analysis = SuspendCallAnalysis {
        types,
        known_fun_effects,
        ctor_call_targets,
        continuation_resume_call_sites,
        object_value_fqns,
        object_property_fqns,
    };
    let seed_locals = fun
        .params
        .iter()
        .map(|param| (param.id, function_ty_may_suspend(types, param.ty)))
        .collect::<HashMap<_, _>>();
    fun.body
        .as_ref()
        .map(|body| analysis.solve_local_fun_effects_in_block(body, &seed_locals))
        .unwrap_or(seed_locals)
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
        hir::ExprKind::Closure(closure) => collect_declared_local_ids_in_expr(&closure.body, out),
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
                    hir::HandleArmKind::ImmediateResume { resume } => {
                        out.insert(resume);
                    }
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
        | hir::ExprKind::Todo(_) => {}
    }
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

fn collect_local_refs_in_stmt(stmt: &hir::Stmt, out: &mut HashMap<hir::SymbolId, (String, TypeId)>) {
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

fn collect_local_refs_in_expr(expr: &hir::Expr, out: &mut HashMap<hir::SymbolId, (String, TypeId)>) {
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
fn render_symbol_list(
    ids: &[hir::SymbolId],
    slots: &HashMap<hir::SymbolId, FrameSlot>,
) -> String {
    let mut labels = ids
        .iter()
        .map(|id| {
            slots
                .get(id)
                .map_or_else(|| format!("unknown#{}", id.as_u32()), FrameSlot::display_name)
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
        hir::ExprKind::Todo(_) => 20,
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
        hir::HandleArmKind::ImmediateResume { resume } => 2 ^ (resume.as_u32() as usize),
        hir::HandleArmKind::EscapeContinuation { continuation } => {
            3 ^ (continuation.as_u32() as usize)
        }
    }
}

impl HandlePlanContext {
    fn reserve_synthetic_symbol_floor(&self, floor: u32) {
        let current = self.next_synthetic_symbol_raw.get();
        if floor > current {
            self.next_synthetic_symbol_raw.set(floor);
        }
    }

    fn allocate_synthetic_symbol_id(&self) -> hir::SymbolId {
        let raw = self.next_synthetic_symbol_raw.get();
        self.next_synthetic_symbol_raw
            .set(raw.saturating_add(1));
        hir::SymbolId::from_raw(raw)
    }

    fn from_codegen<'a, 'ctx>(cg: &MainCodegen<'a, 'ctx>) -> Self {
        let ctor_call_targets = cg
            .ctor_call_sites
            .iter()
            .map(|(span, targets)| {
                let mut stable_targets = targets.clone();
                stable_targets.sort();
                stable_targets.dedup();
                (*span, stable_targets)
            })
            .collect();
        let object_value_fqns = cg.object_inits.keys().cloned().collect();
        let object_property_fqns = cg
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
        let known_fun_effects = cg.known_fun_call_suspendability_map().clone();

        let mut known_local_fun_effects = HashMap::new();
        let mut known_local_metadata = HashMap::new();
        for scope in &cg.env.scopes {
            for (&id, local) in scope {
                let Some(hir_ty) = local.hir_ty else {
                    continue;
                };
                known_local_metadata.insert(
                    id,
                    KnownLocalMetadata {
                        ty: hir_ty,
                        mutable: local.mutable,
                    },
                );
                if hir_ty_is_function_value(cg.types, hir_ty) {
                    known_local_fun_effects.insert(id, local.call_may_suspend);
                }
            }
        }
        let next_synthetic_symbol_raw = known_local_metadata
            .keys()
            .copied()
            .map(hir::SymbolId::as_u32)
            .max()
            .unwrap_or(0)
            .saturating_add(1);

        Self {
            known_fun_effects,
            known_local_fun_effects,
            known_local_metadata,
            next_synthetic_symbol_raw: Cell::new(next_synthetic_symbol_raw),
            ctor_call_targets,
            continuation_resume_call_sites: cg.continuation_resume_call_sites.clone(),
            object_value_fqns,
            object_property_fqns,
        }
    }

    fn extend_known_local_metadata_from_handle(&mut self, handle: &hir::HandleExpr) {
        collect_known_local_metadata_in_handle(handle, &mut self.known_local_metadata);
    }
}

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    pub(in super::super) fn build_ordinary_callee_suspend_plan_from_unified_contract(
        &self,
        body: &hir::Block,
        declared_return_ty: TypeId,
    ) -> Option<CalleeSuspendPlan> {
        let synthetic_handle = hir::HandleExpr {
            body: body.clone(),
            arms: Vec::new(),
            finally: None,
        };

        let mut context = HandlePlanContext::from_codegen(self);
        context.extend_known_local_metadata_from_handle(&synthetic_handle);

        let mut builder = HandlePlanBuilder::new(self.types, &synthetic_handle, &context);
        let outer_slots =
            collect_outer_scope_slots(&synthetic_handle, &context.known_local_metadata);
        let mut env = ScopeEnv::with_outer(outer_slots.clone());
        for slot in &outer_slots {
            builder.frame_slots.insert(slot.id, slot.clone());
        }

        let entry_state = builder.new_state("ordinary.body.entry");
        let _body_end_state = builder.build_block(&synthetic_handle.body, entry_state, &mut env);
        builder.attach_suspend_source_paths();
        builder.attach_suspend_resume_paths();

        if builder.suspend_sites.len() != 1 {
            return None;
        }

        let site = builder.suspend_sites.first()?.clone();
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
        let mut allocate_synthetic_symbol_id = || context.allocate_synthetic_symbol_id();
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

        Some(CalleeSuspendPlan {
            saved_locals,
            resume_slot_id: resume_slot.id(),
            resume_slot_name: resume_slot.name().to_string(),
            resume_slot_ty,
            resume_tail,
        })
    }

    fn ensure_known_fun_call_suspend_cache(&self) {
        if self.known_fun_call_suspend_cache.borrow().is_some() {
            return;
        }

        let ctor_call_targets = self
            .ctor_call_sites
            .iter()
            .map(|(span, targets)| {
                let mut stable_targets = targets.clone();
                stable_targets.sort();
                stable_targets.dedup();
                (*span, stable_targets)
            })
            .collect::<HashMap<_, _>>();
        let object_value_fqns = self.object_inits.keys().cloned().collect::<HashSet<_>>();
        let object_property_fqns = self
            .object_inits
            .iter()
            .flat_map(|(owner_fqn, object_init)| {
                object_init
                    .properties
                    .keys()
                    .map(|name| format!("{owner_fqn}.{name}"))
                    .collect::<Vec<_>>()
            })
            .collect::<HashSet<_>>();
        let known_fun_effects = collect_known_fun_call_suspendability(
            self.types,
            self.fun_index,
            &ctor_call_targets,
            self.continuation_resume_call_sites,
            &object_value_fqns,
            &object_property_fqns,
        );
        *self.known_fun_call_suspend_cache.borrow_mut() = Some(known_fun_effects);
    }

    fn known_fun_call_suspendability_map(&self) -> Ref<'_, HashMap<String, bool>> {
        self.ensure_known_fun_call_suspend_cache();
        Ref::map(self.known_fun_call_suspend_cache.borrow(), |cache| {
            cache.as_ref()
                .expect("known fun suspend cache should be initialized")
        })
    }

    pub(in crate::llvm::codegen) fn local_call_may_suspend_from_hir_ty(
        &self,
        hir_ty: Option<TypeId>,
    ) -> bool {
        hir_ty.is_some_and(|ty| function_ty_may_suspend(self.types, ty))
    }

    pub(in crate::llvm::codegen) fn function_value_expr_may_suspend_when_called_for_local(
        &self,
        expr: &hir::Expr,
    ) -> bool {
        let known_fun_effects = self.known_fun_call_suspendability_map();
        let mut known_locals = HashMap::new();
        for scope in &self.env.scopes {
            for (&id, local) in scope {
                let Some(hir_ty) = local.hir_ty else {
                    continue;
                };
                if hir_ty_is_function_value(self.types, hir_ty) {
                    known_locals.insert(id, local.call_may_suspend);
                }
            }
        }

        let ctor_call_targets = self
            .ctor_call_sites
            .iter()
            .map(|(span, targets)| {
                let mut stable_targets = targets.clone();
                stable_targets.sort();
                stable_targets.dedup();
                (*span, stable_targets)
            })
            .collect::<HashMap<_, _>>();
        let object_value_fqns = self.object_inits.keys().cloned().collect::<HashSet<_>>();
        let object_property_fqns = self
            .object_inits
            .iter()
            .flat_map(|(owner_fqn, object_init)| {
                object_init
                    .properties
                    .keys()
                    .map(|name| format!("{owner_fqn}.{name}"))
                    .collect::<Vec<_>>()
            })
            .collect::<HashSet<_>>();

        SuspendCallAnalysis {
            types: self.types,
            known_fun_effects: &known_fun_effects,
            ctor_call_targets: &ctor_call_targets,
            continuation_resume_call_sites: self.continuation_resume_call_sites,
            object_value_fqns: &object_value_fqns,
            object_property_fqns: &object_property_fqns,
        }
        .function_value_may_suspend_when_called(expr, &known_locals)
    }

    /// Build the unified lowering contract for a `handle` expression.
    ///
    /// Pipeline: HandleExpr → plan → segments (+ validation) → unified state machine → contract.
    /// The returned contract is the single structured input consumed by the downstream LLVM emitter.
    pub(super) fn build_unified_lowering_contract(
        &self,
        handle: &hir::HandleExpr,
    ) -> UnifiedHandleLoweringContract {
        let mut context = HandlePlanContext::from_codegen(self);
        context.extend_known_local_metadata_from_handle(handle);
        let source_plan = HandleStateMachinePlan::build_with_context(self.types, handle, &context);

        // Phase 1 → segments: project the plan into segments and validate the builder contract.
        let segment_list = source_plan.build_segment_list();
        #[cfg(debug_assertions)]
        if let Err(message) = segment_list.validate_builder_contract() {
            panic!("invalid handle segment builder contract: {message}");
        }

        // Debug: verify segment round-trip stability.
        #[cfg(debug_assertions)]
        {
            let segment_signature = segment_list.structural_signature();
            let rebuilt_plan = HandleStateMachinePlan::build_from_segments(&segment_list)
                .unwrap_or_else(|message| {
                    panic!("failed to rebuild handle state machine plan: {message}")
                });
            let rebuilt_segment_list = rebuilt_plan.build_segment_list();
            let rebuilt_segment_signature = rebuilt_segment_list.structural_signature();
            if rebuilt_segment_signature != segment_signature {
                panic!(
                    "segment round-trip mismatch: source={segment_signature} rebuilt={rebuilt_segment_signature}"
                );
            }
        }

        // Phase 2 → unified state machine: transform segments into the canonical full machine.
        let machine = segment_list
            .build_unified_state_machine()
            .unwrap_or_else(|message| {
                panic!("failed to build unified state machine: {message}")
            });

        UnifiedHandleLoweringContract { machine }
    }
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
                && source_path.top_level_stmt_idx + 1 == body.stmts.len() =>
        {
            declared_return_ty
        }
        SuspendResumeConsumer::ReturnValue if source_path.frames.is_empty() => declared_return_ty,
        _ => resume_slot.ty(),
    }
}
