//! 语句与块解析（早期最小子集）。
//!
//! 当前阶段（T0207）目标：
//! - 把块表达式 `{ ... }` 解析为 `ast::Block { stmts }`
//! - 语句仅支持：
//!   - 空语句：`;`
//!   - 表达式语句：基于现有表达式子集解析（`try_parse_expr`，当前支持 postfix + 二元优先级）
//! - 其它尚未实现的语句形态：以 `StmtKind::Missing` 占位，并尽量跳过到语句边界，
//!   以保证 parser cursor 前进与括号平衡（避免把“未实现”误报为顶层语法错误）

use crate::ast;
use crate::span::Span;
use crate::syntax::token::{Keyword, Symbol, TokenKind};

use super::{ParseError, Parser};

impl<'a> Parser<'a> {
    /// 解析块表达式（含函数体 block）：`{ stmt* }`。
    pub(super) fn parse_block(&mut self) -> Result<ast::Block, ParseError> {
        let open = self.expect_symbol(Symbol::LBrace)?;
        let start = open.span.start;

        let mut stmts = Vec::new();
        while !self.peek_kind(TokenKind::Eof) && !self.peek_symbol(Symbol::RBrace) {
            // 允许多余的分号：把它们视为“空语句”。
            if self.peek_symbol(Symbol::Semicolon) {
                let semi = self.bump();
                stmts.push(ast::Stmt {
                    span: semi.span,
                    kind: ast::StmtKind::Empty,
                });
                continue;
            }

            stmts.push(self.parse_stmt()?);
        }

        if self.peek_kind(TokenKind::Eof) {
            return Err(ParseError::UnterminatedGroup {
                close: Symbol::RBrace,
                span: Span::new(start, self.peek().span.end).into(),
            });
        }

        let close = self.expect_symbol(Symbol::RBrace)?;
        Ok(ast::Block {
            span: Span::new(start, close.span.end),
            stmts,
        })
    }

    fn parse_stmt(&mut self) -> Result<ast::Stmt, ParseError> {
        // 局部 `val/var` 绑定语句（T0208）。
        //
        // 注意：此处不能直接复用顶层的 `parse_val_decl`，因为顶层的 initializer 边界规则会假设
        // “下一个 token 必须是顶层 item start/`;`/EOF”，在 block 语境下会误把后续语句吞进 initializer。
        if self.peek_keyword(Keyword::Val) || self.peek_keyword(Keyword::Var) {
            let decl = self.parse_local_val_decl()?;
            let mut span = decl.span;
            // Kotlin 风格也允许 `;` 作为可选分隔符；若存在则把它纳入 stmt span。
            if self.peek_symbol(Symbol::Semicolon) {
                let semi = self.bump();
                span = Span::new(span.start, semi.span.end);
            }

            return Ok(ast::Stmt {
                span,
                kind: ast::StmtKind::Val(decl),
            });
        }

        // 先尝试“表达式语句”：当前阶段的表达式仍是受限子集（postfix + 常见二元优先级），
        // 因此语句边界也就天然落在该表达式结束处。
        if let Some(expr) = self.try_parse_expr()? {
            let mut span = expr.span;
            // Kotlin 风格也允许 `;` 作为可选分隔符；若存在则把它纳入 stmt span。
            if self.peek_symbol(Symbol::Semicolon) {
                let semi = self.bump();
                span = Span::new(span.start, semi.span.end);
            }

            return Ok(ast::Stmt {
                span,
                kind: ast::StmtKind::Expr(expr),
            });
        }

        Ok(self.parse_missing_stmt())
    }

    fn parse_local_val_decl(&mut self) -> Result<ast::ValDecl, ParseError> {
        let kw = if self.peek_keyword(Keyword::Val) {
            self.bump()
        } else if self.peek_keyword(Keyword::Var) {
            self.bump()
        } else {
            let tok = *self.peek();
            return Err(ParseError::Expected {
                expected: "`val` / `var`",
                found: tok.kind,
                span: tok.span.into(),
            });
        };

        let kind = match kw.kind {
            TokenKind::Keyword(Keyword::Val) => ast::ValKind::Val,
            TokenKind::Keyword(Keyword::Var) => ast::ValKind::Var,
            _ => unreachable!("kw 已经被 peek_keyword 过滤"),
        };

        let name_tok = self.expect_kind(TokenKind::Ident, "变量名（标识符）")?;
        let name = ast::Ident {
            span: name_tok.span,
        };

        let ty = if self.eat_symbol(Symbol::Colon) {
            Some(self.parse_type_ref()?)
        } else {
            None
        };

        let mut last_end = ty
            .as_ref()
            .map(|t| t.span().end)
            .unwrap_or(name_tok.span.end);

        let init = if self.eat_symbol(Symbol::Eq) {
            if self.peek_kind(TokenKind::Eof)
                || self.peek_symbol(Symbol::Semicolon)
                || self.peek_symbol(Symbol::RBrace)
            {
                let tok = *self.peek();
                return Err(ParseError::Expected {
                    expected: "表达式（initializer）",
                    found: tok.kind,
                    span: tok.span.into(),
                });
            }

            let init_start = self.peek().span.start;
            match self.try_parse_expr()? {
                Some(expr) => {
                    last_end = expr.span.end;
                    Some(expr)
                }
                None => {
                    // initializer 不是当前表达式子集的起始 token（例如 `-1` / `when (...) { ... }`）：
                    // 当前阶段不报错，消耗一个 token 并降级为 Missing，保证 cursor 前进。
                    let tok = self.bump();
                    last_end = tok.span.end;
                    Some(ast::Expr::missing(Span::new(init_start, last_end)))
                }
            }
        } else {
            None
        };

        Ok(ast::ValDecl {
            span: Span::new(kw.span.start, last_end),
            kind,
            name,
            ty,
            init,
        })
    }

    fn parse_missing_stmt(&mut self) -> ast::Stmt {
        // 保证至少消耗一个 token，避免死循环。
        let start = self.peek().span.start;
        let mut last_end = self.peek().span.end;

        // 粗粒度“语句恢复”：
        // - 在括号深度为 0 时，遇到 `;` / `}` 认为到达语句边界。
        // - 其余 token 全部吞掉，直到边界出现（但不吞 `}`）。
        let mut depth_paren = 0usize;
        let mut depth_brace = 0usize;
        let mut depth_bracket = 0usize;

        while !self.peek_kind(TokenKind::Eof) {
            if depth_paren == 0 && depth_brace == 0 && depth_bracket == 0 {
                if self.peek_symbol(Symbol::Semicolon) || self.peek_symbol(Symbol::RBrace) {
                    break;
                }
            }

            let tok = self.bump();
            last_end = tok.span.end;

            if let TokenKind::Symbol(sym) = tok.kind {
                match sym {
                    Symbol::LParen => depth_paren += 1,
                    Symbol::RParen => depth_paren = depth_paren.saturating_sub(1),
                    Symbol::LBracket => depth_bracket += 1,
                    Symbol::RBracket => depth_bracket = depth_bracket.saturating_sub(1),
                    Symbol::LBrace => depth_brace += 1,
                    Symbol::RBrace => depth_brace = depth_brace.saturating_sub(1),
                    _ => {}
                }
            }
        }

        // 若以 `;` 作为分隔符结束，则把分号也吞掉，避免外层再额外产出一个 Empty stmt。
        if self.peek_symbol(Symbol::Semicolon) {
            let semi = self.bump();
            last_end = semi.span.end;
        }

        ast::Stmt {
            span: Span::new(start, last_end),
            kind: ast::StmtKind::Missing,
        }
    }
}
