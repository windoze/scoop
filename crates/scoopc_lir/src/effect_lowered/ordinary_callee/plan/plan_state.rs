//!  Plan state and handle state operations: PlanState, HandleStateOp, StateTerminator, HandleBranchCondition, ResumeAfterSiteReason.

#![allow(dead_code)]

use super::*;

#[derive(Debug, Clone)]
pub(crate) struct PlanState {
    pub(crate) id: PlanStateId,
    pub(crate) label: String,
    pub(crate) actions: Vec<HandleStateOp>,
    pub(crate) terminator: StateTerminator,
    pub(crate) reads: Vec<hir::SymbolId>,
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
pub(crate) enum StateTerminator {
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
    pub(crate) fn label(&self) -> String {
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

    pub(crate) fn structural_signature(&self) -> usize {
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
    pub(crate) fn label(
        &self,
        slots: &HashMap<hir::SymbolId, FrameSlot>,
        types: &TypeStore,
    ) -> String {
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

    pub(crate) fn structural_signature(&self) -> usize {
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
    pub(crate) fn label(self) -> &'static str {
        match self {
            ResumeAfterSiteReason::ObjectInitAccess => "after object init access",
            ResumeAfterSiteReason::RuntimeRaiseBoundary => "after runtime raise boundary",
            ResumeAfterSiteReason::Call => "after call",
            ResumeAfterSiteReason::Perform => "after perform",
            ResumeAfterSiteReason::NestedHandleBoundary => "after nested handle boundary",
        }
    }

    pub(crate) fn structural_signature(self) -> usize {
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
    pub(crate) fn label(&self) -> String {
        match self {
            HandleBranchCondition::WhileCond { condition } => {
                format!("while-cond@{:?}", condition.span)
            }
            HandleBranchCondition::IfCond { condition } => {
                format!("if-cond@{:?}", condition.span)
            }
        }
    }

    pub(crate) fn structural_signature(&self) -> usize {
        match self {
            HandleBranchCondition::WhileCond { condition } => 1 ^ expr_payload_signature(condition),
            HandleBranchCondition::IfCond { condition } => 2 ^ expr_payload_signature(condition),
        }
    }
}
