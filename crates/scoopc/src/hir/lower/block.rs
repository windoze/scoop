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

use super::super::{Block, CallArg, ClosureExpr, Expr, ExprKind, Stmt, StmtKind, ValueRef};

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
        let lowered = self.with_async_task_body(|this| this.lower_block(pkg_prefix, body));
        let inner_return_ty = self
            .typechecked_expr_ty(span)
            .and_then(|ty| self.task_inner_ty(ty))
            .unwrap_or(lowered.ty);
        let body_expr = Expr {
            span: lowered.span,
            ty: inner_return_ty,
            kind: ExprKind::Block(lowered),
        };
        self.wrap_expr_in_task_create_call(span, body_expr, inner_return_ty)
    }

    pub(super) fn lower_async_fun_body_block(
        &mut self,
        pkg_prefix: &str,
        body: &ast::Block,
        inner_return_ty: TypeId,
    ) -> Block {
        let lowered = self.with_async_task_body(|this| this.lower_block(pkg_prefix, body));
        let body_expr = Expr {
            span: lowered.span,
            ty: inner_return_ty,
            kind: ExprKind::Block(lowered),
        };
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
        let closure_ty =
            self.types
                .ty_function(None, Vec::new(), inner_return_ty, EffectRow::pure(), true);
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

    fn task_inner_ty(&self, ty: TypeId) -> Option<TypeId> {
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
