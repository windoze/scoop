//! 块（block）lowering（TODO T0103d）。
//!
//! 说明：
//! - 该模块负责 AST → HIR 的 block 降低；
//! - 规则与 span 选择尽量保持与原先 `lower/mod.rs` 一致，避免 HIR fixtures 输出漂移。

use crate::ast;
use crate::span::Span;

use super::HirLowering;
use super::types::ExpectedExpr;

use super::super::{Block, CallArg, Expr, ExprKind, Stmt, StmtKind, ValueRef};

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
            let stmt = if is_tail {
                match &s.kind {
                    ast::StmtKind::Expr(expr)
                        if !matches!(expr.kind, ast::ExprKind::Assign { .. }) =>
                    {
                        last_is_tail = true;
                        Stmt {
                            span: s.span,
                            ty: self.builtins.unit,
                            kind: StmtKind::Expr(
                                self.lower_expr_with_expected(pkg_prefix, expr, expected),
                            ),
                        }
                    }
                    _ => self.lower_stmt(pkg_prefix, s),
                }
            } else {
                self.lower_stmt(pkg_prefix, s)
            };
            stmts.push(stmt);
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

    pub(super) fn rewrite_async_fun_block(
        &mut self,
        mut block: Block,
        wrap_tail_expr: bool,
    ) -> Block {
        // T0623：把 `async fun` 的返回值包装成 task 句柄：
        // - `return expr` → `return __scoop_task_spawn_int(expr)`（early stage 仅支持 Int）；
        // - block tail expr（隐式返回）同样做一次包装。

        for stmt in &mut block.stmts {
            match &mut stmt.kind {
                StmtKind::While { body, .. } => {
                    // 这里用 `replace` 把 body move 出来，避免对 Block 增加 Default 约束。
                    let placeholder = Block {
                        span: body.span,
                        ty: body.ty,
                        stmts: Vec::new(),
                    };
                    let old = std::mem::replace(body, placeholder);
                    *body = self.rewrite_async_fun_block(old, false);
                }
                StmtKind::Return { value } => {
                    if let Some(v) = value.take() {
                        *value = Some(self.wrap_task_spawn_int_call(stmt.span, v));
                    }
                }
                _ => {}
            }
        }

        if wrap_tail_expr {
            // 隐式返回：若 block 末尾是表达式语句，则将其值包装为 task handle。
            if let Some(last) = block.stmts.last_mut()
                && let StmtKind::Expr(expr) = &mut last.kind
            {
                // 用占位符把 expr move 出来，避免对 Expr 增加 Default 约束。
                let expr_span = expr.span;
                let expr_ty = expr.ty;
                let old = std::mem::replace(
                    expr,
                    Expr {
                        span: expr_span,
                        ty: expr_ty,
                        kind: ExprKind::Missing,
                    },
                );
                *expr = self.wrap_task_spawn_int_call(expr_span, old);
            }
        }

        // 重新计算 block 类型：保持与 `lower_block` 的规则一致。
        block.ty = block
            .stmts
            .last()
            .and_then(|s| match &s.kind {
                StmtKind::Expr(e) => Some(e.ty),
                _ => None,
            })
            .unwrap_or(self.builtins.unit);

        block
    }

    fn wrap_task_spawn_int_call(&mut self, at: Span, value: Expr) -> Expr {
        // `__scoop_task_spawn_int(value)` → task handle (`UInt`)。
        let fqn = Self::TASK_SPAWN_INT_FQN.to_string();
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
            ty: self.builtins.uint,
            kind: ExprKind::Call {
                callee: Box::new(callee),
                args: vec![CallArg::Positional(value)],
            },
        }
    }
}
