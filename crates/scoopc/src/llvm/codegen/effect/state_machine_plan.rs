use crate::ast;
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
    ctor_call_targets: HashMap<Span, Vec<String>>,
    object_value_fqns: HashSet<String>,
    object_property_fqns: HashSet<String>,
}

#[derive(Debug, Clone)]
pub(super) struct HandleStateMachinePlan {
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

    #[cfg_attr(not(test), allow(dead_code))]
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
                "{pad}  arm{} op={} mode={} body_entry=s{} body_exit={} detach={}\n",
                arm.id,
                arm.op_fqn,
                arm.resume_mode.label(),
                arm.body_entry_state,
                arm.body_exit.label(),
                arm.detach_policy
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
enum HandleStateOp {
    StmtEmpty,
    BindLocal { id: hir::SymbolId },
    DeclareAnonymousVal,
    Assign,
    Break,
    Continue,
    Return,
    TodoStmt { kind: String },
    WhileCondHeader { span: Span },
    LoopReentry { cond_state: PlanStateId },
    ExprMissing,
    Literal,
    ReadLocal { id: hir::SymbolId },
    CleanupEdgeComplete,
    ReturnToEnclosingExpression,
    ObjectInitAccessBoundary { site_id: SuspendSiteId },
    ResumeAfterSite {
        site_id: SuspendSiteId,
        reason: ResumeAfterSiteReason,
    },
    VarRef,
    StructLit,
    TupleLit,
    InterpolatedString,
    Expr,
    RuntimeRaiseBoundary { site_id: SuspendSiteId },
    BinaryExpr,
    ImplicitElseUnit,
    WhenExpr,
    SuspendCall { site_id: SuspendSiteId },
    Call,
    Perform { op_fqn: String },
    NestedHandleBoundary { site_id: SuspendSiteId },
    NestedHandle { nested_id: usize },
    Closure,
    TodoExpr { kind: String },
    ExecuteArmBody {
        arm_id: ArmPlanId,
        op_fqn: String,
        span: Span,
    },
}

#[derive(Debug, Clone, Copy)]
enum ResumeAfterSiteReason {
    ObjectInitAccess,
    RuntimeRaiseBoundary,
    Call,
    Perform,
    NestedHandleBoundary,
}

#[derive(Debug, Clone, Copy)]
enum HandleBranchCondition {
    WhileCond { span: Span },
    IfCond { span: Span },
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
            HandleStateOp::StmtEmpty => "stmt empty".to_string(),
            HandleStateOp::BindLocal { id } => {
                let Some(slot) = slots.get(id) else {
                    return format!("bind local unknown#{}:<?>", id.as_u32());
                };
                format!(
                    "bind local {}:{}",
                    slot.display_name(),
                    types.display(slot.ty)
                )
            }
            HandleStateOp::DeclareAnonymousVal => "declare anonymous val".to_string(),
            HandleStateOp::Assign => "assign".to_string(),
            HandleStateOp::Break => "break".to_string(),
            HandleStateOp::Continue => "continue".to_string(),
            HandleStateOp::Return => "return".to_string(),
            HandleStateOp::TodoStmt { kind } => format!("todo stmt {kind}"),
            HandleStateOp::WhileCondHeader { span } => {
                format!("while cond span={span:?}")
            }
            HandleStateOp::LoopReentry { cond_state } => {
                format!("loop re-entry -> s{cond_state}")
            }
            HandleStateOp::ExprMissing => "expr missing".to_string(),
            HandleStateOp::Literal => "literal".to_string(),
            HandleStateOp::ReadLocal { id } => {
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
            HandleStateOp::ObjectInitAccessBoundary { site_id } => {
                format!("object init access site{site_id}")
            }
            HandleStateOp::ResumeAfterSite { site_id, reason } => {
                format!("resume target for site{site_id} {}", reason.label())
            }
            HandleStateOp::VarRef => "var-ref".to_string(),
            HandleStateOp::StructLit => "struct-lit".to_string(),
            HandleStateOp::TupleLit => "tuple-lit".to_string(),
            HandleStateOp::InterpolatedString => "interpolated-string".to_string(),
            HandleStateOp::Expr => "expr".to_string(),
            HandleStateOp::RuntimeRaiseBoundary { site_id } => {
                format!("runtime raise site{site_id}")
            }
            HandleStateOp::BinaryExpr => "binary-expr".to_string(),
            HandleStateOp::ImplicitElseUnit => "implicit else unit".to_string(),
            HandleStateOp::WhenExpr => "when-expr".to_string(),
            HandleStateOp::SuspendCall { site_id } => format!("suspend call site{site_id}"),
            HandleStateOp::Call => "call".to_string(),
            HandleStateOp::Perform { op_fqn } => format!("perform {op_fqn}"),
            HandleStateOp::NestedHandleBoundary { site_id } => {
                format!("nested handle boundary site{site_id}")
            }
            HandleStateOp::NestedHandle { nested_id } => {
                format!("nested handle nested#{nested_id}")
            }
            HandleStateOp::Closure => "closure".to_string(),
            HandleStateOp::TodoExpr { kind } => format!("todo expr {kind}"),
            HandleStateOp::ExecuteArmBody { op_fqn, span, .. } => {
                format!("execute arm body op={op_fqn} span={span:?}")
            }
        }
    }

    fn structural_signature(&self) -> usize {
        match self {
            HandleStateOp::StmtEmpty => 1,
            HandleStateOp::BindLocal { id } => 2 ^ (id.as_u32() as usize),
            HandleStateOp::DeclareAnonymousVal => 3,
            HandleStateOp::Assign => 4,
            HandleStateOp::Break => 5,
            HandleStateOp::Continue => 6,
            HandleStateOp::Return => 7,
            HandleStateOp::TodoStmt { kind } => 8 ^ kind.len(),
            HandleStateOp::WhileCondHeader { span } => 9 ^ span.start ^ (span.end << 1),
            HandleStateOp::LoopReentry { cond_state } => 10 ^ (*cond_state as usize),
            HandleStateOp::ExprMissing => 11,
            HandleStateOp::Literal => 12,
            HandleStateOp::ReadLocal { id } => 13 ^ (id.as_u32() as usize),
            HandleStateOp::CleanupEdgeComplete => 14,
            HandleStateOp::ReturnToEnclosingExpression => 15,
            HandleStateOp::ObjectInitAccessBoundary { site_id } => 16 ^ (*site_id as usize),
            HandleStateOp::ResumeAfterSite { site_id, reason } => {
                17 ^ (*site_id as usize) ^ (reason.structural_signature() << 1)
            }
            HandleStateOp::VarRef => 18,
            HandleStateOp::StructLit => 19,
            HandleStateOp::TupleLit => 20,
            HandleStateOp::InterpolatedString => 21,
            HandleStateOp::Expr => 22,
            HandleStateOp::RuntimeRaiseBoundary { site_id } => 23 ^ (*site_id as usize),
            HandleStateOp::BinaryExpr => 24,
            HandleStateOp::ImplicitElseUnit => 25,
            HandleStateOp::WhenExpr => 26,
            HandleStateOp::SuspendCall { site_id } => 27 ^ (*site_id as usize),
            HandleStateOp::Call => 28,
            HandleStateOp::Perform { op_fqn } => 29 ^ op_fqn.len(),
            HandleStateOp::NestedHandleBoundary { site_id } => 30 ^ (*site_id as usize),
            HandleStateOp::NestedHandle { nested_id } => 31 ^ *nested_id,
            HandleStateOp::Closure => 32,
            HandleStateOp::TodoExpr { kind } => 33 ^ kind.len(),
            HandleStateOp::ExecuteArmBody { arm_id, op_fqn, span } => {
                34 ^ (*arm_id as usize) ^ op_fqn.len() ^ span.start ^ (span.end << 1)
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
    fn label(self) -> String {
        match self {
            HandleBranchCondition::WhileCond { span } => format!("while-cond@{span:?}"),
            HandleBranchCondition::IfCond { span } => format!("if-cond@{span:?}"),
        }
    }

    fn structural_signature(self) -> usize {
        match self {
            HandleBranchCondition::WhileCond { span } => 1 ^ span.start ^ (span.end << 1),
            HandleBranchCondition::IfCond { span } => 2 ^ span.start ^ (span.end << 1),
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
}

/// statement-position `val` 绑定 suspend site 在 `handle` body 中的源码路径。
///
/// 当前用于 single-arm immediate-resume / escape-continuation 复用旧 replay helper：
/// - direct perform 会从这里恢复 `perform` 所在 stmt 与嵌套控制流路径；
/// - effectful call site（indirect / state-machine callee）也会从这里恢复
///   对应的 top-level `val x = f(...)` 语句位置。
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
enum SuspendSiteKind {
    DirectPerform { op_fqn: String },
    IndirectCallMaySuspend { callee: String },
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
            SuspendSiteKind::DirectPerform { .. } => "direct-perform",
            SuspendSiteKind::IndirectCallMaySuspend { .. } => "indirect-call-may-suspend",
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
            SuspendSiteKind::DirectPerform { op_fqn }
            | SuspendSiteKind::IndirectCallMaySuspend { callee: op_fqn }
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
            SuspendSiteKind::DirectPerform { op_fqn } => 0x11 ^ op_fqn.len(),
            SuspendSiteKind::IndirectCallMaySuspend { callee } => 0x22 ^ callee.len(),
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
    binder_slots: Vec<FrameSlot>,
    capture_locals: Vec<hir::SymbolId>,
    resume_mode: ArmResumeMode,
    body_entry_state: PlanStateId,
    body_exit: ArmBodyExit,
    detach_policy: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ArmResumeMode {
    NeverResume,
    ImmediateResume,
    EscapeContinuation,
}

impl ArmResumeMode {
    fn label(self) -> &'static str {
        match self {
            ArmResumeMode::NeverResume => "never-resume",
            ArmResumeMode::ImmediateResume => "immediate-resume",
            ArmResumeMode::EscapeContinuation => "escape-continuation",
        }
    }

    fn structural_signature(self) -> usize {
        match self {
            ArmResumeMode::NeverResume => 1,
            ArmResumeMode::ImmediateResume => 2,
            ArmResumeMode::EscapeContinuation => 3,
        }
    }
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
enum CleanupScopeKind {
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
struct FrameSlot {
    id: hir::SymbolId,
    name: String,
    ty: TypeId,
    owner_arm: Option<ArmPlanId>,
}

impl FrameSlot {
    fn display_name(&self) -> String {
        format!("{}#{}", self.name, self.id.as_u32())
    }

    fn structural_signature(&self) -> usize {
        self.id.as_u32() as usize
            ^ self.name.len()
            ^ ((self.ty.as_u32() as usize) << 1)
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
        acc
    }
}

impl ArmPlan {
    fn structural_signature(&self) -> usize {
        let mut acc = self.id as usize
            ^ self.op_fqn.len()
            ^ self.resume_mode.structural_signature()
            ^ self.body_entry_state as usize
            ^ self.body_exit.structural_signature()
            ^ self.detach_policy.len();
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
    next_state_id: PlanStateId,
    next_site_id: SuspendSiteId,
    next_cleanup_id: CleanupScopeId,
    states: Vec<PlanState>,
    suspend_sites: Vec<SuspendSitePlan>,
    arm_plans: Vec<ArmPlan>,
    cleanup_scopes: Vec<CleanupScopePlan>,
    frame_slots: HashMap<hir::SymbolId, FrameSlot>,
    nested_handles: Vec<HandleStateMachinePlan>,
}

impl<'a, 'hir> HandlePlanBuilder<'a, 'hir> {
    fn new(
        types: &'a TypeStore,
        handle: &'hir hir::HandleExpr,
        context: &'a HandlePlanContext,
    ) -> Self {
        Self {
            types,
            handle,
            context,
            next_state_id: 0,
            next_site_id: 0,
            next_cleanup_id: 0,
            states: Vec::new(),
            suspend_sites: Vec::new(),
            arm_plans: Vec::new(),
            cleanup_scopes: Vec::new(),
            frame_slots: HashMap::new(),
            nested_handles: Vec::new(),
        }
    }

    fn build(mut self) -> HandleStateMachinePlan {
        let outer_slots = collect_outer_scope_slots(self.handle.body.stmts.as_slice());
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
                note: "normal/raise edges converge through a single finally scope".to_string(),
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
                self.push_action(current_state, HandleStateOp::StmtEmpty);
                current_state
            }
            hir::StmtKind::Expr(expr) => self.build_expr(expr, current_state, env),
            hir::StmtKind::Val(decl) => {
                let mut state = current_state;
                if let Some(init) = decl.init.as_ref() {
                    state = self.build_expr(init, state, env);
                }
                if let Some(id) = decl.id {
                    let slot = FrameSlot {
                        id,
                        name: decl
                            .name
                            .clone()
                            .unwrap_or_else(|| format!("local{}", id.as_u32())),
                        ty: decl.ty,
                        owner_arm: None,
                    };
                    self.frame_slots.entry(id).or_insert_with(|| slot.clone());
                    env.push(slot.clone());
                    self.push_action(state, HandleStateOp::BindLocal { id });
                } else {
                    self.push_action(state, HandleStateOp::DeclareAnonymousVal);
                }
                state
            }
            hir::StmtKind::Assign { lhs, rhs, .. } => {
                let mut state = self.build_expr(lhs, current_state, env);
                state = self.build_expr(rhs, state, env);
                self.record_stmt_reads(state, stmt);
                self.push_action(state, HandleStateOp::Assign);
                state
            }
            hir::StmtKind::While { cond, body } => self.build_while(stmt.span, cond, body, current_state, env),
            hir::StmtKind::Break { .. } => {
                self.push_action(current_state, HandleStateOp::Break);
                self.set_terminator(current_state, StateTerminator::ReturnFromFunction);
                self.new_state("unreachable.after.break")
            }
            hir::StmtKind::Continue { .. } => {
                self.push_action(current_state, HandleStateOp::Continue);
                self.set_terminator(current_state, StateTerminator::ReturnFromFunction);
                self.new_state("unreachable.after.continue")
            }
            hir::StmtKind::Return { value } => {
                if let Some(expr) = value {
                    let state = self.build_expr(expr, current_state, env);
                    self.push_action(state, HandleStateOp::Return);
                    self.set_terminator(state, StateTerminator::ReturnFromFunction);
                    self.new_state("unreachable.after.return")
                } else {
                    self.push_action(current_state, HandleStateOp::Return);
                    self.set_terminator(current_state, StateTerminator::ReturnFromFunction);
                    self.new_state("unreachable.after.return")
                }
            }
            hir::StmtKind::Todo(kind) => {
                self.push_action(
                    current_state,
                    HandleStateOp::TodoStmt {
                        kind: kind.to_string(),
                    },
                );
                current_state
            }
        }
    }

    fn build_while(
        &mut self,
        span: Span,
        cond: &'hir hir::Expr,
        body: &'hir hir::Block,
        current_state: PlanStateId,
        env: &mut ScopeEnv,
    ) -> PlanStateId {
        let cond_state = self.new_state("while.cond");
        self.push_action(cond_state, HandleStateOp::WhileCondHeader { span });
        self.set_terminator(current_state, StateTerminator::Goto(cond_state));

        let cond_eval_state = self.build_expr(cond, cond_state, env);
        let body_state = self.new_state("while.body");
        let exit_state = self.new_state("while.exit");
        self.set_terminator(
            cond_eval_state,
            StateTerminator::Branch {
                condition: HandleBranchCondition::WhileCond { span: cond.span },
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
                self.push_action(current_state, HandleStateOp::ExprMissing);
                current_state
            }
            hir::ExprKind::Literal(_) => {
                self.push_action(current_state, HandleStateOp::Literal);
                current_state
            }
            hir::ExprKind::VarRef(hir::ValueRef::Local { id, name, .. }) => {
                self.frame_slots.entry(*id).or_insert_with(|| FrameSlot {
                    id: *id,
                    name: name.clone(),
                    ty: expr.ty,
                    owner_arm: None,
                });
                self.push_action(current_state, HandleStateOp::ReadLocal { id: *id });
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
                        HandleStateOp::ObjectInitAccessBoundary { site_id },
                    );
                    self.set_terminator(current_state, StateTerminator::Suspend { site_id });
                    let resume_state = self.new_state(format!("resume.after.site{site_id}"));
                    self.push_action(
                        resume_state,
                        HandleStateOp::ResumeAfterSite {
                            site_id,
                            reason: ResumeAfterSiteReason::ObjectInitAccess,
                        },
                    );
                    self.set_suspend_resume_target(site_id, resume_state);
                    return resume_state;
                }
                self.push_action(current_state, HandleStateOp::VarRef);
                current_state
            }
            hir::ExprKind::UnresolvedIdent { .. } => {
                self.push_action(current_state, HandleStateOp::VarRef);
                current_state
            }
            hir::ExprKind::StructLit { fields, .. } => {
                let mut state = current_state;
                for field in fields {
                    state = self.build_expr(&field.value, state, env);
                }
                self.push_action(state, HandleStateOp::StructLit);
                state
            }
            hir::ExprKind::TupleLit { elements } => {
                let mut state = current_state;
                for element in elements {
                    state = self.build_expr(element, state, env);
                }
                self.push_action(state, HandleStateOp::TupleLit);
                state
            }
            hir::ExprKind::InterpolatedString { parts, .. } => {
                let mut state = current_state;
                for part in parts {
                    if let hir::InterpolatedStringPart::Expr { expr } = part {
                        state = self.build_expr(expr, state, env);
                    }
                }
                self.push_action(state, HandleStateOp::InterpolatedString);
                state
            }
            hir::ExprKind::Unary { expr: inner, .. }
            | hir::ExprKind::TypeCheck { expr: inner, .. } => {
                let state = self.build_expr(inner, current_state, env);
                self.record_expr_reads(state, expr);
                self.push_action(state, HandleStateOp::Expr);
                state
            }
            hir::ExprKind::Cast { expr: inner, op, .. } => {
                let state = self.build_expr(inner, current_state, env);
                if matches!(op, ast::CastOp::As) {
                    self.record_expr_reads(state, expr);
                    let site_id = self.new_suspend_site(
                        expr.span,
                        SuspendSiteKind::RuntimeRaise {
                            reason: "ClassCastFailed".to_string(),
                        },
                        env.available_ids(),
                    );
                    self.push_action(state, HandleStateOp::RuntimeRaiseBoundary { site_id });
                    self.set_terminator(state, StateTerminator::Suspend { site_id });
                    let resume_state = self.new_state(format!("resume.after.site{site_id}"));
                    self.push_action(
                        resume_state,
                        HandleStateOp::ResumeAfterSite {
                            site_id,
                            reason: ResumeAfterSiteReason::RuntimeRaiseBoundary,
                        },
                    );
                    self.set_suspend_resume_target(site_id, resume_state);
                    return resume_state;
                }
                self.record_expr_reads(state, expr);
                self.push_action(state, HandleStateOp::Expr);
                state
            }
            hir::ExprKind::MemberAccess { receiver, member } => {
                let state = self.build_expr(receiver, current_state, env);
                if let Some(kind) = self.classify_hidden_suspend_member_access(member) {
                    self.record_expr_reads(state, expr);
                    let site_id = self.new_suspend_site(expr.span, kind, env.available_ids());
                    self.push_action(state, HandleStateOp::ObjectInitAccessBoundary { site_id });
                    self.set_terminator(state, StateTerminator::Suspend { site_id });
                    let resume_state = self.new_state(format!("resume.after.site{site_id}"));
                    self.push_action(
                        resume_state,
                        HandleStateOp::ResumeAfterSite {
                            site_id,
                            reason: ResumeAfterSiteReason::ObjectInitAccess,
                        },
                    );
                    self.set_suspend_resume_target(site_id, resume_state);
                    return resume_state;
                }
                self.record_expr_reads(state, expr);
                self.push_action(state, HandleStateOp::Expr);
                state
            }
            hir::ExprKind::Binary { lhs, rhs, .. } => {
                let state = self.build_expr(lhs, current_state, env);
                let state = self.build_expr(rhs, state, env);
                self.record_expr_reads(state, expr);
                self.push_action(state, HandleStateOp::BinaryExpr);
                state
            }
            hir::ExprKind::Block(block) => self.build_block(block, current_state, env),
            hir::ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => {
                let cond_state = self.build_expr(cond, current_state, env);
                let then_state = self.new_state("if.then");
                let else_state = self.new_state("if.else");
                let merge_state = self.new_state("if.merge");
                self.set_terminator(
                    cond_state,
                    StateTerminator::Branch {
                        condition: HandleBranchCondition::IfCond { span: cond.span },
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
                    self.push_action(else_state, HandleStateOp::ImplicitElseUnit);
                    self.set_terminator(else_state, StateTerminator::Goto(merge_state));
                }
                merge_state
            }
            hir::ExprKind::When { subject, arms } => {
                let mut state = self.build_expr(subject, current_state, env);
                for arm in arms {
                    if let Some(guard) = arm.guard.as_ref() {
                        state = self.build_expr(guard, state, env);
                    }
                    state = self.build_expr(&arm.body, state, env);
                }
                self.push_action(state, HandleStateOp::WhenExpr);
                state
            }
            hir::ExprKind::Call { callee, args } => {
                let mut state = self.build_expr(callee, current_state, env);
                for arg in args {
                    state = match arg {
                        hir::CallArg::Positional(expr) => self.build_expr(expr, state, env),
                        hir::CallArg::Named { value, .. } => self.build_expr(value, state, env),
                    };
                }
                if let Some(kind) = self.classify_suspend_call(expr, callee) {
                    self.record_expr_reads(state, expr);
                    let site_id = self.new_suspend_site(expr.span, kind, env.available_ids());
                    self.push_action(state, HandleStateOp::SuspendCall { site_id });
                    self.set_terminator(state, StateTerminator::Suspend { site_id });
                    let resume_state = self.new_state(format!("resume.after.site{site_id}"));
                    self.push_action(
                        resume_state,
                        HandleStateOp::ResumeAfterSite {
                            site_id,
                            reason: ResumeAfterSiteReason::Call,
                        },
                    );
                    self.set_suspend_resume_target(site_id, resume_state);
                    return resume_state;
                }
                self.record_expr_reads(state, expr);
                self.push_action(state, HandleStateOp::Call);
                state
            }
            hir::ExprKind::Perform { op, args } => {
                let mut state = current_state;
                for arg in args {
                    state = match arg {
                        hir::CallArg::Positional(expr) => self.build_expr(expr, state, env),
                        hir::CallArg::Named { value, .. } => self.build_expr(value, state, env),
                    };
                }
                self.record_expr_reads(state, expr);
                let site_id = self.new_suspend_site(
                    expr.span,
                    SuspendSiteKind::DirectPerform {
                        op_fqn: op.fqn.clone(),
                    },
                    env.available_ids(),
                );
                self.push_action(
                    state,
                    HandleStateOp::Perform {
                        op_fqn: op.fqn.clone(),
                    },
                );
                self.set_terminator(state, StateTerminator::Suspend { site_id });
                let resume_state = self.new_state(format!("resume.after.site{site_id}"));
                self.push_action(
                    resume_state,
                    HandleStateOp::ResumeAfterSite {
                        site_id,
                        reason: ResumeAfterSiteReason::Perform,
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
                        HandleStateOp::NestedHandleBoundary { site_id },
                    );
                    self.set_terminator(current_state, StateTerminator::Suspend { site_id });
                    let resume_state = self.new_state(format!("resume.after.site{site_id}"));
                    self.push_action(
                        resume_state,
                        HandleStateOp::ResumeAfterSite {
                            site_id,
                            reason: ResumeAfterSiteReason::NestedHandleBoundary,
                        },
                    );
                    self.set_suspend_resume_target(site_id, resume_state);
                    return resume_state;
                }
                self.push_action(current_state, HandleStateOp::NestedHandle { nested_id });
                current_state
            }
            hir::ExprKind::Closure(closure) => {
                self.push_action(current_state, HandleStateOp::Closure);
                self.record_expr_reads(current_state, &closure.body);
                current_state
            }
            hir::ExprKind::Todo(kind) => {
                self.push_action(
                    current_state,
                    HandleStateOp::TodoExpr {
                        kind: kind.to_string(),
                    },
                );
                current_state
            }
        }
    }

    fn classify_suspend_call(
        &self,
        expr: &hir::Expr,
        callee: &hir::Expr,
    ) -> Option<SuspendSiteKind> {
        if let Some(kind) = self.classify_builtin_suspend_call(callee) {
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
            && let Some(effectful) = self.context.known_local_fun_effects.get(id).copied()
        {
            return if effectful {
                Some(SuspendSiteKind::IndirectCallMaySuspend {
                    callee: format!("local#{}", id.as_u32()),
                })
            } else {
                None
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
                return None;
            }
            return try_extract_callee_fqn(callee).map_or_else(
                || {
                    Some(SuspendSiteKind::IndirectCallMaySuspend {
                        callee: format!("expr@{:?}", expr.span),
                    })
                },
                |fqn| Some(SuspendSiteKind::CallStateMachineCallee { callee: fqn }),
            );
        }
        None
    }

    fn classify_builtin_suspend_call(&self, callee: &hir::Expr) -> Option<SuspendSiteKind> {
        let hir::ExprKind::MemberAccess { member, .. } = &callee.kind else {
            return None;
        };

        if member.name != "resume" {
            return None;
        }

        // 保持与现有 codegen 入口一致：当前阶段尚未支持真实的实例方法分派，
        // `member.name == "resume"` 会被当作 builtin `Continuation.resume(...)`。
        //
        // 这里若额外依赖 receiver 的 HIR 类型，会比 codegen 本身更严格，导致
        // `try/catch { k.resume(...) }` 在 plan 侧误判为 no-suspend。
        Some(SuspendSiteKind::RuntimeRaise {
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
            let mut capture_locals = used
                .into_iter()
                .filter_map(|(id, (name, ty))| {
                    if declared.contains(&id) {
                        return None;
                    }
                    self.frame_slots.entry(id).or_insert_with(|| FrameSlot {
                        id,
                        name,
                        ty,
                        owner_arm: None,
                    });
                    Some(id)
                })
                .collect::<Vec<_>>();
            capture_locals.sort_by_key(|id| id.as_u32());

            let body_entry_state = self.new_state(format!("arm{arm_id}.body"));
            self.push_action(
                body_entry_state,
                HandleStateOp::ExecuteArmBody {
                    arm_id,
                    op_fqn: arm.op.op.fqn.clone(),
                    span: arm.body.span,
                },
            );

            let (resume_mode, body_exit, detach_policy) = match arm.kind {
                hir::HandleArmKind::NonResuming => (
                    ArmResumeMode::NeverResume,
                    ArmBodyExit::ReturnHandle,
                    "detach matching frame; arm body runs outside handler scope".to_string(),
                ),
                hir::HandleArmKind::ImmediateResume { .. } => (
                    ArmResumeMode::ImmediateResume,
                    ArmBodyExit::ResumeMatchedSite,
                    "detach sibling frames; resume writes payload back to matched site".to_string(),
                ),
                hir::HandleArmKind::EscapeContinuation { .. } => (
                    ArmResumeMode::EscapeContinuation,
                    ArmBodyExit::MaterializeContinuation,
                    "detach source frames; continuation reinstalls captured handler stack".to_string(),
                ),
            };
            self.set_terminator(body_entry_state, StateTerminator::ArmExit(body_exit));

            self.arm_plans.push(ArmPlan {
                id: arm_id,
                op_fqn: arm.op.op.fqn.clone(),
                binder_slots,
                capture_locals,
                resume_mode,
                body_entry_state,
                body_exit,
                detach_policy,
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
            has_one_shot_flag: self
                .arm_plans
                .iter()
                .any(|arm| matches!(arm.resume_mode, ArmResumeMode::EscapeContinuation)),
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
                SuspendSiteKind::IndirectCallMaySuspend { .. }
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
                if matches!(
                    init.kind,
                    hir::ExprKind::Perform { .. } | hir::ExprKind::Call { .. }
                ) {
                    self.record_suspend_source_path(init, top_level_stmt_idx, path);
                } else {
                    self.attach_suspend_source_paths_in_expr(init, top_level_stmt_idx, path);
                }
            }
            hir::StmtKind::Expr(expr) => {
                self.attach_suspend_source_paths_in_expr(expr, top_level_stmt_idx, path);
            }
            hir::StmtKind::Assign { .. } | hir::StmtKind::Return { .. } => {}
            hir::StmtKind::While { cond, body } => {
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
                then_branch,
                else_branch,
                ..
            } => {
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
                }
            }
            hir::ExprKind::When { arms, .. } => {
                for (arm_index, when_arm) in arms.iter().enumerate() {
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
                    }
                }
            }
            _ => {}
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
                (SuspendSiteKind::DirectPerform { .. }, hir::ExprKind::Perform { .. })
                    | (
                        SuspendSiteKind::IndirectCallMaySuspend { .. },
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
        todo!("legacy scan.rs 已删除；state read tracking 需改走 unified data source");
    }

    fn record_expr_reads(&mut self, _state_id: PlanStateId, _expr: &hir::Expr) {
        todo!("legacy scan.rs 已删除；state read tracking 需改走 unified data source");
    }

    #[allow(dead_code)]
    fn add_reads(&mut self, state_id: PlanStateId, used: HashSet<hir::SymbolId>) {
        let state = self.state_mut(state_id);
        state.reads.extend(used);
        state.reads.sort_by_key(|id| id.as_u32());
        state.reads.dedup_by_key(|id| id.as_u32());
    }
}

fn matching_arms(arms: &[ArmPlan], kind: &SuspendSiteKind) -> Vec<ArmPlanId> {
    match kind {
        SuspendSiteKind::DirectPerform { op_fqn } => arms
            .iter()
            .filter(|arm| arm.op_fqn == *op_fqn)
            .map(|arm| arm.id)
            .collect(),
        SuspendSiteKind::RuntimeRaise { .. } => arms
            .iter()
            .filter(|arm| arm.op_fqn == "scoop.core.Raise.raise")
            .map(|arm| arm.id)
            .collect(),
        SuspendSiteKind::IndirectCallMaySuspend { .. }
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

fn collect_outer_scope_slots(stmts: &[hir::Stmt]) -> Vec<FrameSlot> {
    let mut declared = HashSet::new();
    for stmt in stmts {
        collect_declared_local_ids_in_stmt(stmt, &mut declared);
    }

    let mut used = HashMap::new();
    for stmt in stmts {
        collect_local_refs_in_stmt(stmt, &mut used);
    }

    let mut slots = used
        .into_iter()
        .filter(|(id, _)| !declared.contains(id))
        .map(|(id, (name, ty))| FrameSlot {
            id,
            name,
            ty,
            owner_arm: None,
        })
        .collect::<Vec<_>>();
    slots.sort_by_key(|slot| slot.id.as_u32());
    slots
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

impl HandlePlanContext {
    fn from_codegen<'a, 'ctx>(cg: &MainCodegen<'a, 'ctx>) -> Self {
        let known_fun_effects = cg
            .fun_index
            .iter()
            .map(|(fqn, fun)| {
                (
                    fqn.clone(),
                    match cg.types.kind(fun.ty) {
                        TypeKind::Ref(RefTypeKind::Function(fun_ty)) => !fun_ty.effects.is_pure(),
                        _ => false,
                    },
                )
            })
            .collect();

        let mut known_local_fun_effects = HashMap::new();
        for scope in &cg.env.scopes {
            for (&id, local) in scope {
                let Some(hir_ty) = local.hir_ty else {
                    continue;
                };
                if let TypeKind::Ref(RefTypeKind::Function(fun_ty)) = cg.types.kind(hir_ty) {
                    known_local_fun_effects.insert(id, !fun_ty.effects.is_pure());
                }
            }
        }

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

        Self {
            known_fun_effects,
            known_local_fun_effects,
            ctor_call_targets,
            object_value_fqns,
            object_property_fqns,
        }
    }
}

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    fn build_handle_state_machine_plan(&self, handle: &hir::HandleExpr) -> HandleStateMachinePlan {
        let context = HandlePlanContext::from_codegen(self);
        let source_plan = HandleStateMachinePlan::build_with_context(self.types, handle, &context);
        // Keep the phase-1 segment projection and its builder-facing invariants in sync
        // during the ground-up rewrite, even before the builder fully switches to it as
        // the only input.
        let segment_list = source_plan.build_segment_list();
        #[cfg(debug_assertions)]
        if let Err(message) = segment_list.validate_builder_contract() {
            panic!("invalid handle segment builder contract: {message}");
        }
        let segment_signature = segment_list.structural_signature();
        let rebuilt_plan = HandleStateMachinePlan::build_from_segments(&segment_list)
            .unwrap_or_else(|message| panic!("failed to rebuild handle state machine plan: {message}"));
        #[cfg(debug_assertions)]
        {
            let rebuilt_segment_list = rebuilt_plan.build_segment_list();
            let rebuilt_segment_signature = rebuilt_segment_list.structural_signature();
            if rebuilt_segment_signature != segment_signature {
                panic!(
                    "segment round-trip mismatch: source={segment_signature} rebuilt={rebuilt_segment_signature}"
                );
            }

            let source_simplification_signature = source_plan
                .build_mode_specific_simplification()
                .structural_signature();
            let rebuilt_simplification_signature = rebuilt_plan
                .build_mode_specific_simplification()
                .structural_signature();
            if source_simplification_signature != rebuilt_simplification_signature {
                panic!(
                    "mode-specific simplification mismatch after segment rebuild: source={source_simplification_signature} rebuilt={rebuilt_simplification_signature}"
                );
            }
        }
        rebuilt_plan
    }
}

#[cfg(test)]
include!("state_machine_plan_tests.rs");
