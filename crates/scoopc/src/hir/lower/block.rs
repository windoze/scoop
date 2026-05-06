//! 块（block）lowering（TODO T0103d）。
//!
//! 说明：
//! - 该模块负责 AST → HIR 的 block 降低；
//! - 规则与 span 选择尽量保持与原先 `lower/mod.rs` 一致，避免 HIR fixtures 输出漂移。

use crate::ast;
use crate::span::Span;
use crate::ty::{EffectRow, RefTypeKind, TypeId, TypeKind};

use super::HirLowering;
use super::types::ExpectedExpr;
use super::util::compute_closure_captures;

use super::super::{
    Block, CallArg, ClosureExpr, EffectOpRef, Expr, ExprKind, HandleArm, HandleArmKind,
    HandleBinder, HandleExpr, HandleOp, LiteralKind, Stmt, StmtKind, ValueRef,
};

impl<'a> HirLowering<'a> {
    pub(super) fn lower_block(&mut self, pkg_prefix: &str, b: &ast::Block) -> Block {
        self.lower_block_with_expected(pkg_prefix, b, ExpectedExpr::default())
    }

    pub(super) fn lower_block_with_expected(
        &mut self,
        pkg_prefix: &str,
        b: &ast::Block,
        expected: ExpectedExpr,
    ) -> Block {
        let mut stmts = Vec::with_capacity(b.stmts.len());
        let mut last_is_tail = false;
        for (index, s) in b.stmts.iter().enumerate() {
            let is_last = index + 1 == b.stmts.len();
            // T3102：只有最后一条且无尾分号的 Expr 语句才是 tail expression。
            let is_tail = is_last && !s.has_trailing_semi;
            last_is_tail =
                self.lower_stmt_into_with_tail(pkg_prefix, s, &mut stmts, expected, is_tail);
        }

        // T3102：只有 tail expression（无尾分号的最后 Expr 语句）才贡献 block 类型；
        // 否则 block 值为 Unit。
        let ty = if last_is_tail {
            stmts
                .last()
                .and_then(|s| match &s.kind {
                    StmtKind::Expr(e) => Some(e.ty),
                    _ => None,
                })
                .unwrap_or(self.builtins.unit)
        } else {
            self.builtins.unit
        };

        Block {
            span: b.span,
            ty,
            stmts,
        }
    }

    pub(super) fn lower_async_task_expr_from_block(
        &mut self,
        pkg_prefix: &str,
        span: Span,
        body: &ast::Block,
    ) -> Expr {
        let lowered_body = self.lower_block(pkg_prefix, body);
        let inner_return_ty = self
            .typechecked_expr_ty(span)
            .and_then(|ty| self.task_inner_ty(ty))
            .unwrap_or(lowered_body.ty);
        let body_expr =
            self.lower_async_task_step_result_expr(body.span, lowered_body, inner_return_ty);
        self.wrap_expr_in_task_create_call(span, body_expr, inner_return_ty)
    }

    pub(super) fn lower_async_fun_body_block(
        &mut self,
        pkg_prefix: &str,
        body: &ast::Block,
        inner_return_ty: TypeId,
    ) -> Block {
        let lowered_body = self.lower_block(pkg_prefix, body);
        let body_expr =
            self.lower_async_task_step_result_expr(body.span, lowered_body, inner_return_ty);
        let task_expr = self.wrap_expr_in_task_create_call(body.span, body_expr, inner_return_ty);
        let task_ty = task_expr.ty;
        Block {
            span: body.span,
            ty: task_ty,
            stmts: vec![Stmt {
                span: body.span,
                ty: task_ty,
                kind: StmtKind::Expr(task_expr),
            }],
        }
    }

    fn wrap_expr_in_task_create_call(
        &mut self,
        at: Span,
        body: Expr,
        inner_return_ty: TypeId,
    ) -> Expr {
        let result_ty = self.task_type_of(inner_return_ty);
        let step_result_ty = self.task_step_result_type(inner_return_ty);
        let closure_ty =
            self.types
                .ty_function(None, Vec::new(), step_result_ty, EffectRow::pure(), true);
        let closure = Expr {
            span: body.span,
            ty: closure_ty,
            kind: ExprKind::Closure(ClosureExpr {
                span: body.span,
                id: self.alloc_closure_id(),
                at_safe_span: None,
                captures: compute_closure_captures(&[], &body, &self.local_mutability),
                params: Vec::new(),
                body: Box::new(body),
            }),
        };

        self.call_top_level_fun_with_type_args(
            at,
            Self::TASK_CREATE_FQN,
            &[inner_return_ty],
            vec![closure],
            result_ty,
        )
    }

    fn lower_async_task_step_result_expr(
        &mut self,
        body_span: Span,
        lowered_body: Block,
        inner_return_ty: TypeId,
    ) -> Expr {
        let step_result_ty = self.task_step_result_type(inner_return_ty);
        let mut normalized_body = lowered_body;
        if let Some(tail_stmt) = normalized_body.stmts.last_mut()
            && let StmtKind::Return { value } = &mut tail_stmt.kind
        {
            let value = value.take().unwrap_or(Expr {
                span: tail_stmt.span,
                ty: self.builtins.unit,
                kind: ExprKind::Literal(LiteralKind::Unit),
            });
            tail_stmt.ty = inner_return_ty;
            tail_stmt.kind = StmtKind::Expr(value);
        }
        self.normalize_async_returns_in_block(
            &mut normalized_body,
            body_span,
            inner_return_ty,
            step_result_ty,
        );
        if !matches!(
            normalized_body.stmts.last().map(|stmt| &stmt.kind),
            Some(StmtKind::Return { .. })
        ) {
            if let Some(tail_stmt) = normalized_body.stmts.pop() {
                match tail_stmt.kind {
                    StmtKind::Expr(tail_expr) => self.push_async_ready_tail(
                        &mut normalized_body.stmts,
                        tail_stmt.span,
                        tail_expr,
                        inner_return_ty,
                        step_result_ty,
                    ),
                    other => {
                        normalized_body.stmts.push(Stmt {
                            span: tail_stmt.span,
                            ty: tail_stmt.ty,
                            kind: other,
                        });
                        if inner_return_ty == self.builtins.unit {
                            let unit_expr = Expr {
                                span: body_span,
                                ty: self.builtins.unit,
                                kind: ExprKind::Literal(LiteralKind::Unit),
                            };
                            self.push_async_ready_tail(
                                &mut normalized_body.stmts,
                                body_span,
                                unit_expr,
                                inner_return_ty,
                                step_result_ty,
                            );
                        }
                    }
                }
            } else if inner_return_ty == self.builtins.unit {
                let unit_expr = Expr {
                    span: body_span,
                    ty: self.builtins.unit,
                    kind: ExprKind::Literal(LiteralKind::Unit),
                };
                self.push_async_ready_tail(
                    &mut normalized_body.stmts,
                    body_span,
                    unit_expr,
                    inner_return_ty,
                    step_result_ty,
                );
            }
        }
        normalized_body.ty = step_result_ty;
        let handle_body = Block {
            span: body_span,
            ty: step_result_ty,
            stmts: normalized_body.stmts,
        };

        // 这里由一个内部 `Async.await` handler 统一拦截 task body 中的所有 await 站点。
        // 为了避免把 `Int`/`Bool` 这类 word payload 擦成普通 `Any` 后丢失数值，
        // task 私有 resume payload 在 HIR 边界上统一改走 `(Int, Any)` transport carrier；
        // step driver 的 delimiter answer 继续显式保留为私有 `__TaskStepResult<T>`。
        let transport_ty = self.task_transport_type();
        let task_binder_ty = self.task_type_of(transport_ty);
        let continuation_ty = self.continuation_type_of(transport_ty, step_result_ty);
        let (task_span, task_id, task_name) =
            self.fresh_synthetic_local(body_span, "__task_awaited", false);
        let (continuation_span, continuation_id, continuation_name) =
            self.fresh_synthetic_local(body_span, "__task_continuation", false);

        let task_ref = Expr {
            span: task_span,
            ty: task_binder_ty,
            kind: ExprKind::VarRef(ValueRef::Local {
                id: task_id,
                name: task_name.clone(),
                decl_span: task_span,
            }),
        };
        let continuation_ref = Expr {
            span: continuation_span,
            ty: continuation_ty,
            kind: ExprKind::VarRef(ValueRef::Local {
                id: continuation_id,
                name: continuation_name,
                decl_span: continuation_span,
            }),
        };
        let pending_expr = self.call_top_level_fun_with_type_args(
            body_span,
            Self::TASK_STEP_PENDING_FQN,
            &[inner_return_ty],
            vec![task_ref, continuation_ref],
            step_result_ty,
        );

        Expr {
            span: body_span,
            ty: step_result_ty,
            kind: ExprKind::Handle(HandleExpr {
                body: handle_body,
                arms: vec![HandleArm {
                    span: body_span,
                    op: HandleOp {
                        span: body_span,
                        effect_ty: self.async_effect_type(),
                        op: EffectOpRef {
                            span: body_span,
                            fqn: Self::ASYNC_AWAIT_FQN.to_string(),
                        },
                        binders: vec![HandleBinder {
                            span: task_span,
                            id: task_id,
                            name: task_name,
                            ty: task_binder_ty,
                        }],
                    },
                    kind: HandleArmKind::EscapeContinuation {
                        continuation: continuation_id,
                    },
                    body: pending_expr,
                }],
                finally: None,
            }),
        }
    }

    fn normalize_async_returns_in_block(
        &mut self,
        block: &mut Block,
        body_span: Span,
        inner_return_ty: TypeId,
        step_result_ty: TypeId,
    ) {
        let mut normalized = Vec::with_capacity(block.stmts.len());
        for mut stmt in std::mem::take(&mut block.stmts) {
            match stmt.kind {
                StmtKind::Return { value } => {
                    let value = value.unwrap_or(Expr {
                        span: stmt.span,
                        ty: self.builtins.unit,
                        kind: ExprKind::Literal(LiteralKind::Unit),
                    });
                    let ready_ref = self.push_async_ready_value(
                        &mut normalized,
                        stmt.span,
                        value,
                        inner_return_ty,
                    );
                    let ready_expr = self.async_ready_call(
                        stmt.span,
                        ready_ref,
                        inner_return_ty,
                        step_result_ty,
                    );
                    normalized.push(Stmt {
                        span: stmt.span,
                        ty: self.builtins.nothing,
                        kind: StmtKind::Return {
                            value: Some(ready_expr),
                        },
                    });
                }
                StmtKind::Expr(mut expr) => {
                    self.normalize_async_returns_in_expr(
                        &mut expr,
                        body_span,
                        inner_return_ty,
                        step_result_ty,
                    );
                    stmt.kind = StmtKind::Expr(expr);
                    normalized.push(stmt);
                }
                StmtKind::Val(mut decl) => {
                    if let Some(init) = decl.init.as_mut() {
                        self.normalize_async_returns_in_expr(
                            init,
                            body_span,
                            inner_return_ty,
                            step_result_ty,
                        );
                    }
                    stmt.kind = StmtKind::Val(decl);
                    normalized.push(stmt);
                }
                StmtKind::Assign {
                    mut lhs,
                    eq_span,
                    mut rhs,
                } => {
                    self.normalize_async_returns_in_expr(
                        &mut lhs,
                        body_span,
                        inner_return_ty,
                        step_result_ty,
                    );
                    self.normalize_async_returns_in_expr(
                        &mut rhs,
                        body_span,
                        inner_return_ty,
                        step_result_ty,
                    );
                    stmt.kind = StmtKind::Assign { lhs, eq_span, rhs };
                    normalized.push(stmt);
                }
                StmtKind::While { mut cond, mut body } => {
                    self.normalize_async_returns_in_expr(
                        &mut cond,
                        body_span,
                        inner_return_ty,
                        step_result_ty,
                    );
                    self.normalize_async_returns_in_block(
                        &mut body,
                        body_span,
                        inner_return_ty,
                        step_result_ty,
                    );
                    stmt.kind = StmtKind::While { cond, body };
                    normalized.push(stmt);
                }
                other => {
                    stmt.kind = other;
                    normalized.push(stmt);
                }
            }
        }
        block.stmts = normalized;
    }

    fn normalize_async_returns_in_expr(
        &mut self,
        expr: &mut Expr,
        body_span: Span,
        inner_return_ty: TypeId,
        step_result_ty: TypeId,
    ) {
        match &mut expr.kind {
            ExprKind::Block(block) => self.normalize_async_returns_in_block(
                block,
                body_span,
                inner_return_ty,
                step_result_ty,
            ),
            ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => {
                self.normalize_async_returns_in_expr(
                    cond,
                    body_span,
                    inner_return_ty,
                    step_result_ty,
                );
                self.normalize_async_returns_in_expr(
                    then_branch,
                    body_span,
                    inner_return_ty,
                    step_result_ty,
                );
                if let Some(else_branch) = else_branch {
                    self.normalize_async_returns_in_expr(
                        else_branch,
                        body_span,
                        inner_return_ty,
                        step_result_ty,
                    );
                }
            }
            ExprKind::When { subject, arms } => {
                self.normalize_async_returns_in_expr(
                    subject,
                    body_span,
                    inner_return_ty,
                    step_result_ty,
                );
                for arm in arms {
                    self.normalize_async_returns_in_expr(
                        &mut arm.body,
                        body_span,
                        inner_return_ty,
                        step_result_ty,
                    );
                }
            }
            ExprKind::Handle(handle) => {
                self.normalize_async_returns_in_block(
                    &mut handle.body,
                    body_span,
                    inner_return_ty,
                    step_result_ty,
                );
                for arm in &mut handle.arms {
                    self.normalize_async_returns_in_expr(
                        &mut arm.body,
                        body_span,
                        inner_return_ty,
                        step_result_ty,
                    );
                }
                if let Some(finally) = handle.finally.as_mut() {
                    self.normalize_async_returns_in_block(
                        finally,
                        body_span,
                        inner_return_ty,
                        step_result_ty,
                    );
                }
            }
            ExprKind::Unary { expr, .. }
            | ExprKind::TypeCheck { expr, .. }
            | ExprKind::Cast { expr, .. }
            | ExprKind::MemberAccess { receiver: expr, .. } => self
                .normalize_async_returns_in_expr(expr, body_span, inner_return_ty, step_result_ty),
            ExprKind::Binary { lhs, rhs, .. } => {
                self.normalize_async_returns_in_expr(
                    lhs,
                    body_span,
                    inner_return_ty,
                    step_result_ty,
                );
                self.normalize_async_returns_in_expr(
                    rhs,
                    body_span,
                    inner_return_ty,
                    step_result_ty,
                );
            }
            ExprKind::TupleLit { elements } => {
                for element in elements {
                    self.normalize_async_returns_in_expr(
                        element,
                        body_span,
                        inner_return_ty,
                        step_result_ty,
                    );
                }
            }
            ExprKind::StructLit { fields, .. } => {
                for field in fields {
                    self.normalize_async_returns_in_expr(
                        &mut field.value,
                        body_span,
                        inner_return_ty,
                        step_result_ty,
                    );
                }
            }
            ExprKind::InterpolatedString { parts, .. } => {
                for part in parts {
                    if let super::super::InterpolatedStringPart::Expr { expr } = part {
                        self.normalize_async_returns_in_expr(
                            expr,
                            body_span,
                            inner_return_ty,
                            step_result_ty,
                        );
                    }
                }
            }
            ExprKind::Call { callee, args } => {
                self.normalize_async_returns_in_expr(
                    callee,
                    body_span,
                    inner_return_ty,
                    step_result_ty,
                );
                for arg in args {
                    match arg {
                        CallArg::Positional(expr) | CallArg::Named { value: expr, .. } => self
                            .normalize_async_returns_in_expr(
                                expr,
                                body_span,
                                inner_return_ty,
                                step_result_ty,
                            ),
                    }
                }
            }
            ExprKind::Perform { args, .. } => {
                for arg in args {
                    match arg {
                        CallArg::Positional(expr) | CallArg::Named { value: expr, .. } => self
                            .normalize_async_returns_in_expr(
                                expr,
                                body_span,
                                inner_return_ty,
                                step_result_ty,
                            ),
                    }
                }
            }
            ExprKind::Closure(_) => {}
            ExprKind::Missing
            | ExprKind::Literal(_)
            | ExprKind::VarRef(_)
            | ExprKind::UnresolvedIdent { .. }
            | ExprKind::Todo(_) => {}
        }
    }

    fn push_async_ready_tail(
        &mut self,
        stmts: &mut Vec<Stmt>,
        span: Span,
        value: Expr,
        inner_return_ty: TypeId,
        step_result_ty: TypeId,
    ) {
        let ready_ref = self.push_async_ready_value(stmts, span, value, inner_return_ty);
        let ready_expr = self.async_ready_call(span, ready_ref, inner_return_ty, step_result_ty);
        stmts.push(Stmt {
            span,
            ty: step_result_ty,
            kind: StmtKind::Expr(ready_expr),
        });
    }

    fn push_async_ready_value(
        &mut self,
        stmts: &mut Vec<Stmt>,
        span: Span,
        value: Expr,
        inner_return_ty: TypeId,
    ) -> Expr {
        let (ready_value_span, ready_value_id, ready_value_name) =
            self.fresh_synthetic_local(span, "__task_ready_value", false);
        stmts.push(Stmt {
            span,
            ty: inner_return_ty,
            kind: StmtKind::Val(super::super::ValDecl {
                span,
                id: Some(ready_value_id),
                name: Some(ready_value_name.clone()),
                mutable: false,
                ty: inner_return_ty,
                init: Some(value),
            }),
        });
        Expr {
            span: ready_value_span,
            ty: inner_return_ty,
            kind: ExprKind::VarRef(ValueRef::Local {
                id: ready_value_id,
                name: ready_value_name,
                decl_span: ready_value_span,
            }),
        }
    }

    fn async_ready_call(
        &mut self,
        span: Span,
        ready_value_ref: Expr,
        inner_return_ty: TypeId,
        step_result_ty: TypeId,
    ) -> Expr {
        self.call_top_level_fun_with_type_args(
            span,
            Self::TASK_STEP_READY_FQN,
            &[inner_return_ty],
            vec![ready_value_ref],
            step_result_ty,
        )
    }

    pub(super) fn task_inner_ty(&self, ty: TypeId) -> Option<TypeId> {
        let TypeKind::Ref(RefTypeKind::Nominal(nominal)) = self.types.kind(ty) else {
            return None;
        };
        if nominal.fqn == Self::TASK_TYPE_FQN && nominal.args.len() == 1 {
            nominal.args.first().copied()
        } else {
            None
        }
    }
}
