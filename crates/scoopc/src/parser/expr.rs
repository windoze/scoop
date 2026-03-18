//! 表达式解析（早期最小子集）。
//!
//! 当前阶段实现：
//! - “原子表达式”（T0206）：
//! - ident
//! - int literal
//! - string literal
//! - 括号分组 `( ... )`（仅在内部也是原子表达式时才保留其 kind，否则降级为 Missing）
//! - postfix 调用表达式（T0209）：`callee(args...)`
//! - postfix 成员访问（T0210）：`receiver.member`
//! - 二元运算符优先级（T0211）：`1 + 2 * 3`
//!
//! 说明：
//! - 该模块的目标是支撑顶层 `val/var` initializer 的增量解析；
//! - 更复杂的表达式（调用/成员访问/二元运算/控制流等）会在后续任务中逐步补齐。

use crate::ast;
use crate::span::Span;
use crate::syntax::token::{Symbol, TokenKind};

use super::{ParseError, Parser};

impl Parser {
    /// 尝试解析一个表达式（当前为：postfix + 常见二元运算符优先级）。
    ///
    /// - 若当前位置不是表达式的起始 token，则返回 `Ok(None)` 且不消费 token。
    /// - 若能解析出表达式，则返回 `Ok(Some(expr))`。
    pub(super) fn try_parse_expr(&mut self) -> Result<Option<ast::Expr>, ParseError> {
        self.parse_expr_bp(0)
    }

    /// 尝试解析一个“postfix 表达式”（当前支持成员访问与函数调用）。
    ///
    /// - 起始处必须是原子表达式，否则返回 `Ok(None)` 且不消费 token。
    /// - 解析到原子表达式后，会尽可能多地消耗 postfix 后缀（例如连续调用）。
    pub(super) fn try_parse_expr_postfix(&mut self) -> Result<Option<ast::Expr>, ParseError> {
        let Some(mut expr) = self.try_parse_expr_atom()? else {
            return Ok(None);
        };

        loop {
            if self.peek_symbol(Symbol::Dot) {
                expr = self.parse_member_access_expr(expr)?;
                continue;
            }
            if self.peek_symbol(Symbol::LParen) {
                expr = self.parse_call_expr(expr)?;
                continue;
            }
            break;
        }

        Ok(Some(expr))
    }

    fn parse_expr_bp(&mut self, min_bp: u8) -> Result<Option<ast::Expr>, ParseError> {
        let Some(mut lhs) = self.try_parse_expr_postfix()? else {
            return Ok(None);
        };

        loop {
            let (l_bp, r_bp, op) = match self.peek_binary_op() {
                Some(x) => x,
                None => break,
            };

            if l_bp < min_bp {
                break;
            }

            let op_tok = self.bump();
            let Some(rhs) = self.parse_expr_bp(r_bp)? else {
                let tok = *self.peek();
                return Err(ParseError::Expected {
                    expected: "表达式（右操作数）",
                    found: tok.kind,
                    span: tok.span.into(),
                });
            };

            lhs = ast::Expr {
                span: Span::new(lhs.span.start, rhs.span.end),
                kind: ast::ExprKind::Binary {
                    lhs: Box::new(lhs),
                    op,
                    op_span: op_tok.span,
                    rhs: Box::new(rhs),
                },
            };
        }

        Ok(Some(lhs))
    }

    fn peek_binary_op(&self) -> Option<(u8, u8, ast::BinaryOp)> {
        let TokenKind::Symbol(sym) = self.peek().kind else {
            return None;
        };
        binary_binding_power(sym)
    }

    /// 尝试解析一个“原子表达式”。
    ///
    /// - 若当前位置不是原子表达式的起始 token，则返回 `Ok(None)`，并且不消费任何 token。
    /// - 若成功（或以 `Missing` 降级）解析出一个原子表达式，则返回 `Ok(Some(expr))`。
    ///
    /// 设计选择：
    /// - 为避免在表达式尚未完全实现时“误报语法错误”，未知起始 token 会交由上层做 fallback 跳过。
    pub(super) fn try_parse_expr_atom(&mut self) -> Result<Option<ast::Expr>, ParseError> {
        if self.peek_kind(TokenKind::Ident) {
            let tok = self.bump();
            return Ok(Some(ast::Expr {
                span: tok.span,
                kind: ast::ExprKind::Ident(ast::Ident { span: tok.span }),
            }));
        }

        if self.peek_kind(TokenKind::IntLiteral) {
            let tok = self.bump();
            return Ok(Some(ast::Expr {
                span: tok.span,
                kind: ast::ExprKind::IntLit,
            }));
        }

        if matches!(self.peek().kind, TokenKind::StringLiteral(_)) {
            let tok = self.bump();
            return Ok(Some(ast::Expr {
                span: tok.span,
                kind: ast::ExprKind::StringLit,
            }));
        }

        if self.peek_symbol(Symbol::LBrace) {
            let block = self.parse_block()?;
            return Ok(Some(ast::Expr {
                span: block.span,
                kind: ast::ExprKind::Block(block),
            }));
        }

        if self.peek_symbol(Symbol::LParen) {
            return self.try_parse_paren_group_expr();
        }

        Ok(None)
    }

    fn parse_call_expr(&mut self, callee: ast::Expr) -> Result<ast::Expr, ParseError> {
        let open = self.expect_symbol(Symbol::LParen)?;
        let start = callee.span.start;

        let mut args = Vec::new();
        if self.peek_symbol(Symbol::RParen) {
            let close = self.bump();
            return Ok(ast::Expr {
                span: Span::new(start, close.span.end),
                kind: ast::ExprKind::Call {
                    callee: Box::new(callee),
                    args,
                },
            });
        }

        loop {
            let tok = *self.peek();
            let arg = self.try_parse_expr()?.ok_or(ParseError::Expected {
                expected: "表达式（参数）",
                found: tok.kind,
                span: tok.span.into(),
            })?;
            args.push(arg);

            if self.eat_symbol(Symbol::Comma) {
                // trailing comma
                if self.peek_symbol(Symbol::RParen) {
                    break;
                }
                continue;
            }

            break;
        }

        if self.peek_kind(TokenKind::Eof) {
            return Err(ParseError::UnterminatedGroup {
                close: Symbol::RParen,
                span: Span::new(open.span.start, self.peek().span.end).into(),
            });
        }

        let close = self.expect_symbol(Symbol::RParen)?;
        Ok(ast::Expr {
            span: Span::new(start, close.span.end),
            kind: ast::ExprKind::Call {
                callee: Box::new(callee),
                args,
            },
        })
    }

    fn parse_member_access_expr(&mut self, receiver: ast::Expr) -> Result<ast::Expr, ParseError> {
        let _dot = self.expect_symbol(Symbol::Dot)?;
        let member_tok = self.expect_kind(TokenKind::Ident, "成员名（标识符）")?;

        Ok(ast::Expr {
            span: Span::new(receiver.span.start, member_tok.span.end),
            kind: ast::ExprKind::MemberAccess {
                receiver: Box::new(receiver),
                member: ast::Ident {
                    span: member_tok.span,
                },
            },
        })
    }

    fn try_parse_paren_group_expr(&mut self) -> Result<Option<ast::Expr>, ParseError> {
        let open = self.expect_symbol(Symbol::LParen)?;
        let start = open.span.start;

        // 先允许空 `()`：当前没有 Unit 字面量节点，先用 Missing 占位。
        if self.peek_symbol(Symbol::RParen) {
            let close = self.bump();
            return Ok(Some(ast::Expr::missing(Span::new(start, close.span.end))));
        }

        // 仅当括号内也是“当前已支持的表达式子集”时才保留其 kind；否则整体降级为 Missing。
        let inner = self.try_parse_expr()?;
        let Some(mut inner) = inner else {
            let span = self.consume_balanced_after_open(Symbol::LParen, Symbol::RParen, start)?;
            return Ok(Some(ast::Expr::missing(span)));
        };

        if self.peek_symbol(Symbol::RParen) {
            let close = self.bump();
            inner.span = Span::new(start, close.span.end);
            return Ok(Some(inner));
        }

        // 括号内存在额外 token（例如 `(1, 2)` / `(1; 2)`）：
        // 当前阶段不支持，吞掉整段并降级为 Missing。
        let span = self.consume_balanced_after_open(Symbol::LParen, Symbol::RParen, start)?;
        Ok(Some(ast::Expr::missing(span)))
    }

    /// 在已经消费了 `open` 的前提下，继续消费直到与之匹配的 `close`（含 close）。
    ///
    /// 该函数用于“表达式不支持但需要保持 token cursor 正确”的场景。
    fn consume_balanced_after_open(
        &mut self,
        open: Symbol,
        close: Symbol,
        start: usize,
    ) -> Result<Span, ParseError> {
        let mut depth = 1usize;
        while !self.peek_kind(TokenKind::Eof) {
            let tok = self.bump();
            if let TokenKind::Symbol(sym) = tok.kind {
                if sym == open {
                    depth += 1;
                } else if sym == close {
                    depth -= 1;
                    if depth == 0 {
                        return Ok(Span::new(start, tok.span.end));
                    }
                }
            }
        }

        Err(ParseError::UnterminatedGroup {
            close,
            span: Span::new(start, self.peek().span.end).into(),
        })
    }
}

fn binary_binding_power(sym: Symbol) -> Option<(u8, u8, ast::BinaryOp)> {
    // 参考 C/Swift 的常见优先级层级（从高到低）：
    // - multiplicative: * / %
    // - additive: + -
    // - shift: << >>
    // - relational: < <= > >=
    // - equality: == !=
    // - bitwise: & ^ |
    // - logical: && ||
    //
    // 说明：当前阶段所有二元运算均按“左结合”处理。
    let (prec, op) = match sym {
        Symbol::Star => (11, ast::BinaryOp::Mul),
        Symbol::Slash => (11, ast::BinaryOp::Div),
        Symbol::Percent => (11, ast::BinaryOp::Rem),

        Symbol::Plus => (10, ast::BinaryOp::Add),
        Symbol::Minus => (10, ast::BinaryOp::Sub),

        Symbol::LtLt => (9, ast::BinaryOp::Shl),
        Symbol::GtGt => (9, ast::BinaryOp::Shr),

        Symbol::Lt => (8, ast::BinaryOp::Lt),
        Symbol::LtEq => (8, ast::BinaryOp::Le),
        Symbol::Gt => (8, ast::BinaryOp::Gt),
        Symbol::GtEq => (8, ast::BinaryOp::Ge),

        Symbol::EqEq => (7, ast::BinaryOp::Eq),
        Symbol::BangEq => (7, ast::BinaryOp::Ne),

        Symbol::And => (6, ast::BinaryOp::BitAnd),
        Symbol::Caret => (5, ast::BinaryOp::BitXor),
        Symbol::Or => (4, ast::BinaryOp::BitOr),

        Symbol::AndAnd => (3, ast::BinaryOp::LogAnd),
        Symbol::OrOr => (2, ast::BinaryOp::LogOr),

        _ => return None,
    };

    // Pratt/precedence climbing：左结合使用 (prec, prec + 1)。
    Some((prec, prec + 1, op))
}
