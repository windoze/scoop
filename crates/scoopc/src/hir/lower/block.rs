//! 块（block）lowering（TODO T0103d）。
//!
//! 说明：
//! - 该模块负责 AST → HIR 的 block 降低；
//! - 规则与 span 选择尽量保持与原先 `lower/mod.rs` 一致，避免 HIR fixtures 输出漂移。

use crate::ast;

use super::super::{Block, StmtKind};
use super::HirLowering;
use super::types::ExpectedExpr;

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
}
