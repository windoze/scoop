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
    HandleBinder, HandleExpr, HandleOp, Stmt, StmtKind, ValueRef,
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
            if is_tail {
                match &s.kind {
                    ast::StmtKind::Expr(expr)
                        if !matches!(expr.kind, ast::ExprKind::Assign { .. }) =>
                    {
                        last_is_tail = true;
                        stmts.push(Stmt {
                            span: s.span,
                            ty: self.builtins.unit,
                            kind: StmtKind::Expr(
                                self.lower_expr_with_expected(pkg_prefix, expr, expected),
                            ),
                        });
                    }
                    _ => self.lower_stmt_into(pkg_prefix, s, &mut stmts),
                }
            } else {
                self.lower_stmt_into(pkg_prefix, s, &mut stmts);
            }
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

        let fqn = Self::TASK_CREATE_FQN.to_string();
        let callee = Expr {
            span: at,
            ty: self.builtins.any,
            kind: ExprKind::VarRef(ValueRef::TopLevel {
                id: self.symbols.intern_top_level(fqn.clone()),
                fqn,
            }),
        };

        Expr {
            span: at,
            ty: result_ty,
            kind: ExprKind::Call {
                callee: Box::new(callee),
                args: vec![CallArg::Positional(closure)],
            },
        }
    }

    fn lower_async_task_step_result_expr(
        &mut self,
        body_span: Span,
        lowered_body: Block,
        inner_return_ty: TypeId,
    ) -> Expr {
        let step_result_ty = self.task_step_result_type(inner_return_ty);
        let mut normalized_body = lowered_body;
        if let Some(tail_stmt) = normalized_body.stmts.pop() {
            match tail_stmt.kind {
                // 先把 async body 的尾值物化到局部，避免“尾值直接是 await/handle replay”
                // 时在后续 `__task_ready_value` 包装里丢掉 GC 引用结果。
                StmtKind::Expr(tail_expr) => {
                    let (body_value_span, body_value_id, body_value_name) =
                        self.fresh_synthetic_local(body_span, "__task_body_value", false);
                    let body_value_ref = Expr {
                        span: body_value_span,
                        ty: inner_return_ty,
                        kind: ExprKind::VarRef(ValueRef::Local {
                            id: body_value_id,
                            name: body_value_name.clone(),
                            decl_span: body_value_span,
                        }),
                    };
                    normalized_body.stmts.push(Stmt {
                        span: tail_stmt.span,
                        ty: inner_return_ty,
                        kind: StmtKind::Val(super::super::ValDecl {
                            span: tail_stmt.span,
                            id: Some(body_value_id),
                            name: Some(body_value_name),
                            mutable: false,
                            ty: inner_return_ty,
                            init: Some(tail_expr),
                        }),
                    });
                    normalized_body.stmts.push(Stmt {
                        span: tail_stmt.span,
                        ty: inner_return_ty,
                        kind: StmtKind::Expr(body_value_ref),
                    });
                }
                other => {
                    normalized_body.stmts.push(Stmt {
                        span: tail_stmt.span,
                        ty: tail_stmt.ty,
                        kind: other,
                    });
                }
            }
        }
        let body_expr = Expr {
            span: normalized_body.span,
            ty: inner_return_ty,
            kind: ExprKind::Block(normalized_body),
        };
        let (ready_value_span, ready_value_id, ready_value_name) =
            self.fresh_synthetic_local(body_span, "__task_ready_value", false);
        let ready_value_ref = Expr {
            span: ready_value_span,
            ty: inner_return_ty,
            kind: ExprKind::VarRef(ValueRef::Local {
                id: ready_value_id,
                name: ready_value_name.clone(),
                decl_span: ready_value_span,
            }),
        };
        let ready_expr = self.call_top_level_fun(
            body_span,
            Self::TASK_STEP_READY_FQN,
            vec![ready_value_ref],
            step_result_ty,
        );
        let handle_body = Block {
            span: body_span,
            ty: step_result_ty,
            stmts: vec![
                // 先把 async body 的结果落到局部，再调用 `step_ready(...)`。
                // 直接生成 `step_ready(body_expr)` 会把 effectful body 放进普通调用实参位置，
                // 当前 state-machine replay 在这种形状下会错误地回放整段 body。
                Stmt {
                    span: body_span,
                    ty: inner_return_ty,
                    kind: StmtKind::Val(super::super::ValDecl {
                        span: body_span,
                        id: Some(ready_value_id),
                        name: Some(ready_value_name),
                        mutable: false,
                        ty: inner_return_ty,
                        init: Some(body_expr),
                    }),
                },
                Stmt {
                    span: body_span,
                    ty: step_result_ty,
                    kind: StmtKind::Expr(ready_expr),
                },
            ],
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
        let pending_expr = self.call_top_level_fun(
            body_span,
            Self::TASK_STEP_PENDING_FQN,
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
