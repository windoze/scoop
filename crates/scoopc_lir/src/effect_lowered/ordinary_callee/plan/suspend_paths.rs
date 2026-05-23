//!  Suspend-site planning types: SuspendSitePlan, source/resume paths, SuspendSiteKind.

#![allow(dead_code)]

use super::*;

#[derive(Debug, Clone)]
pub(crate) struct SuspendSitePlan {
    pub(crate) id: SuspendSiteId,
    pub(crate) span: Span,
    pub(crate) kind: SuspendSiteKind,
    pub(crate) owner_state: PlanStateId,
    pub(crate) resume_target: PlanStateId,
    pub(crate) escape_resume_target: Option<PlanStateId>,
    pub(crate) matching_arms: Vec<ArmPlanId>,
    pub(crate) available_locals: Vec<hir::SymbolId>,
    pub(crate) capture_locals: Vec<hir::SymbolId>,
    pub(crate) source_path: Option<SuspendSourcePath>,
    pub(crate) resume_path: Option<SuspendResumePath>,
    pub(crate) continuation_escape: ContinuationEscapeState,
}

/// suspend site 在其所属外层 source root（handle body stmt / arm body / finally stmt）
/// 下的源码路径。
///
/// 该路径只描述源码中的根位置与嵌套控制流层级，供统一 state-machine
/// 构建、重建与验证阶段使用。
#[derive(Debug, Clone)]
pub(crate) enum SuspendSourceRoot {
    HandleBodyStmt { stmt_idx: usize, stmt_span: Span },
    ArmBody { arm_index: usize, body_span: Span },
    FinallyStmt { stmt_idx: usize, stmt_span: Span },
}

impl SuspendSourceRoot {
    #[cfg(test)]
    pub(crate) fn label(&self) -> String {
        match self {
            SuspendSourceRoot::HandleBodyStmt { stmt_idx, .. } => format!("top[{stmt_idx}]"),
            SuspendSourceRoot::ArmBody { arm_index, .. } => format!("arm#{arm_index}"),
            SuspendSourceRoot::FinallyStmt { stmt_idx, .. } => format!("finally[{stmt_idx}]"),
        }
    }

    pub(crate) fn structural_signature(&self) -> usize {
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

    pub(crate) fn span(&self) -> Span {
        match self {
            SuspendSourceRoot::HandleBodyStmt { stmt_span, .. }
            | SuspendSourceRoot::FinallyStmt { stmt_span, .. } => *stmt_span,
            SuspendSourceRoot::ArmBody { body_span, .. } => *body_span,
        }
    }

    pub(crate) fn handle_body_stmt_idx(&self) -> Option<usize> {
        match self {
            SuspendSourceRoot::HandleBodyStmt { stmt_idx, .. } => Some(*stmt_idx),
            SuspendSourceRoot::ArmBody { .. } | SuspendSourceRoot::FinallyStmt { .. } => None,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct SuspendSourcePath {
    pub(crate) root: SuspendSourceRoot,
    pub(crate) frames: Vec<SuspendSourceFramePath>,
}

impl SuspendSourcePath {
    #[cfg(test)]
    pub(crate) fn label(&self) -> String {
        let mut parts = vec![self.root.label()];
        parts.extend(self.frames.iter().map(SuspendSourceFramePath::label));
        parts.join(" -> ")
    }

    pub(crate) fn structural_signature(&self) -> usize {
        let mut acc = self.root.structural_signature();
        for frame in &self.frames {
            acc ^= frame.structural_signature();
        }
        acc
    }

    pub(crate) fn root_span(&self) -> Span {
        self.root.span()
    }

    pub(crate) fn handle_body_stmt_idx(&self) -> Option<usize> {
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
pub(crate) struct SuspendResumePath {
    pub(crate) consumer: SuspendResumeConsumer,
    pub(crate) expr_frames: Vec<SuspendResumeExprFrame>,
}

impl SuspendResumePath {
    #[cfg(test)]
    pub(crate) fn label(&self) -> String {
        let mut parts = vec![self.consumer.label().to_string()];
        parts.extend(self.expr_frames.iter().map(SuspendResumeExprFrame::label));
        parts.join(" -> ")
    }

    pub(crate) fn structural_signature(&self) -> usize {
        let mut acc = self.consumer.structural_signature();
        for frame in &self.expr_frames {
            acc ^= frame.structural_signature();
        }
        acc
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum SuspendResumeConsumer {
    ValInit,
    ExprStmt,
    AssignLhs,
    AssignRhs,
    ReturnValue,
    WhileCond,
}

impl SuspendResumeConsumer {
    #[cfg(test)]
    pub(crate) fn label(self) -> &'static str {
        match self {
            SuspendResumeConsumer::ValInit => "val-init",
            SuspendResumeConsumer::ExprStmt => "expr-stmt",
            SuspendResumeConsumer::AssignLhs => "assign-lhs",
            SuspendResumeConsumer::AssignRhs => "assign-rhs",
            SuspendResumeConsumer::ReturnValue => "return-value",
            SuspendResumeConsumer::WhileCond => "while-cond",
        }
    }

    pub(crate) fn structural_signature(self) -> usize {
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
pub(crate) enum SuspendResumeExprFrame {
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
    pub(crate) fn expr_span(&self) -> Span {
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
    pub(crate) fn label(&self) -> String {
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

    pub(crate) fn structural_signature(&self) -> usize {
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
pub(crate) enum SuspendSourceFramePath {
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
    pub(crate) fn label(&self) -> String {
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

    pub(crate) fn structural_signature(&self) -> usize {
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
    pub(crate) fn label(&self) -> &'static str {
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
    pub(crate) fn detail(&self) -> Option<String> {
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

    pub(crate) fn is_continuation_resume_boundary(&self) -> bool {
        matches!(
            self,
            SuspendSiteKind::CallMaySuspend { callee }
                | SuspendSiteKind::RuntimeRaise { reason: callee }
                if callee == "Continuation.resume"
        )
    }

    pub(crate) fn structural_signature(&self) -> usize {
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

    pub(crate) fn needs_escape_resume_replay(&self) -> bool {
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
