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
//! - postfix 非空断言（T0212）：`expr!!`
//! - 二元运算符优先级（T0211）：`1 + 2 * 3`
//! - Elvis（T0212）：`a ?: b`
//! - 类型判断/转换（T0213）：`is`/`!is`/`as`/`as?`
//! - `if` 表达式（T0214）：`if (cond) thenExpr else elseExpr?`
//! - `when` 表达式（T0215）：`when (expr) { ... }`（最小分支子集）
//!
//! 说明：
//! - 该模块的目标是支撑顶层 `val/var` initializer 的增量解析；
//! - 更复杂的表达式（调用/成员访问/二元运算/控制流等）会在后续任务中逐步补齐。

use crate::ast;
use crate::span::Span;
use crate::syntax::token::{Keyword, Symbol, TokenKind};

use super::{ParseError, Parser};

impl Parser {
    /// 尝试解析一个表达式（当前为：postfix + 常见二元运算符优先级）。
    ///
    /// - 若当前位置不是表达式的起始 token，则返回 `Ok(None)` 且不消费 token。
    /// - 若能解析出表达式，则返回 `Ok(Some(expr))`。
    pub(super) fn try_parse_expr(&mut self) -> Result<Option<ast::Expr>, ParseError> {
        self.parse_expr_bp(0)
    }

    /// `when` 分支 body 的表达式解析（带 arm 边界规则）。
    ///
    /// 由于 lexer 不保留换行，`when { ... }` 的 arm 列表需要依靠 token 形态推断边界：
    /// - 若看到 `is <TypeRef> ->`，优先将其解释为“下一个 arm 的 pattern 起始”，
    ///   而不是把 `is` 当成当前表达式的中缀类型判断运算符。
    fn try_parse_expr_in_when_arm(&mut self) -> Result<Option<ast::Expr>, ParseError> {
        self.parse_expr_bp_in_when_arm(0)
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
            if self.peek_keyword(Keyword::With) {
                expr = self.parse_with_update_expr(expr)?;
                continue;
            }
            if self.peek_symbol(Symbol::Dot) {
                expr = self.parse_member_access_expr(expr)?;
                continue;
            }
            if self.peek_symbol(Symbol::LParen) {
                expr = self.parse_call_expr(expr)?;
                continue;
            }
            if self.peek_symbol(Symbol::BangBang) {
                expr = self.parse_not_null_assert_expr(expr)?;
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
            let infix = match self.peek_infix_op() {
                Some(x) => x,
                None => break,
            };

            if infix.l_bp < min_bp {
                break;
            }

            let op_span = self.bump_infix_op_span(infix.consume);

            match infix.kind {
                InfixOpKind::Binary(op) => {
                    let Some(rhs) = self.parse_expr_bp(infix.r_bp)? else {
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
                            op_span,
                            rhs: Box::new(rhs),
                        },
                    };
                }
                InfixOpKind::TypeCheck(op) => {
                    let ty = self.parse_type_ref()?;
                    lhs = ast::Expr {
                        span: Span::new(lhs.span.start, ty.span().end),
                        kind: ast::ExprKind::TypeCheck {
                            expr: Box::new(lhs),
                            op,
                            op_span,
                            ty,
                        },
                    };
                }
                InfixOpKind::Cast(op) => {
                    let ty = self.parse_type_ref()?;
                    lhs = ast::Expr {
                        span: Span::new(lhs.span.start, ty.span().end),
                        kind: ast::ExprKind::Cast {
                            expr: Box::new(lhs),
                            op,
                            op_span,
                            ty,
                        },
                    };
                }
            }
        }

        Ok(Some(lhs))
    }

    fn parse_expr_bp_in_when_arm(&mut self, min_bp: u8) -> Result<Option<ast::Expr>, ParseError> {
        let Some(mut lhs) = self.try_parse_expr_postfix()? else {
            return Ok(None);
        };

        loop {
            let infix = match self.peek_infix_op_in_when_arm() {
                Some(x) => x,
                None => break,
            };

            if infix.l_bp < min_bp {
                break;
            }

            let op_span = self.bump_infix_op_span(infix.consume);

            match infix.kind {
                InfixOpKind::Binary(op) => {
                    let Some(rhs) = self.parse_expr_bp_in_when_arm(infix.r_bp)? else {
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
                            op_span,
                            rhs: Box::new(rhs),
                        },
                    };
                }
                InfixOpKind::TypeCheck(op) => {
                    let ty = self.parse_type_ref()?;
                    lhs = ast::Expr {
                        span: Span::new(lhs.span.start, ty.span().end),
                        kind: ast::ExprKind::TypeCheck {
                            expr: Box::new(lhs),
                            op,
                            op_span,
                            ty,
                        },
                    };
                }
                InfixOpKind::Cast(op) => {
                    let ty = self.parse_type_ref()?;
                    lhs = ast::Expr {
                        span: Span::new(lhs.span.start, ty.span().end),
                        kind: ast::ExprKind::Cast {
                            expr: Box::new(lhs),
                            op,
                            op_span,
                            ty,
                        },
                    };
                }
            }
        }

        Ok(Some(lhs))
    }

    fn bump_infix_op_span(&mut self, consume: u8) -> Span {
        let first = self.bump();
        if consume <= 1 {
            return first.span;
        }

        let mut end = first.span.end;
        for _ in 1..consume {
            end = self.bump().span.end;
        }
        Span::new(first.span.start, end)
    }

    fn peek_infix_op(&self) -> Option<InfixOp> {
        // 1) `!is`（两个 token）：`!` + `is`
        if self.peek_symbol(Symbol::Bang) && self.peek_n(1).kind == TokenKind::Keyword(Keyword::Is)
        {
            return Some(InfixOp {
                l_bp: 8,
                r_bp: 9,
                consume: 2,
                kind: InfixOpKind::TypeCheck(ast::TypeCheckOp::NotIs),
            });
        }

        // 2) 单 token 的 keyword 运算符：`is` / `as` / `as?`
        if let TokenKind::Keyword(kw) = self.peek().kind {
            let kind = match kw {
                Keyword::Is => InfixOpKind::TypeCheck(ast::TypeCheckOp::Is),
                Keyword::As => InfixOpKind::Cast(ast::CastOp::As),
                Keyword::AsQ => InfixOpKind::Cast(ast::CastOp::AsQ),
                _ => return None,
            };
            return Some(InfixOp {
                l_bp: 8,
                r_bp: 9,
                consume: 1,
                kind,
            });
        }

        // 3) 普通 symbol 二元运算符
        let TokenKind::Symbol(sym) = self.peek().kind else {
            return None;
        };
        let (l_bp, r_bp, op) = binary_binding_power(sym)?;
        Some(InfixOp {
            l_bp,
            r_bp,
            consume: 1,
            kind: InfixOpKind::Binary(op),
        })
    }

    fn peek_infix_op_in_when_arm(&self) -> Option<InfixOp> {
        // 1) `!is`（两个 token）：`!` + `is`
        if self.peek_symbol(Symbol::Bang) && self.peek_n(1).kind == TokenKind::Keyword(Keyword::Is)
        {
            return Some(InfixOp {
                l_bp: 8,
                r_bp: 9,
                consume: 2,
                kind: InfixOpKind::TypeCheck(ast::TypeCheckOp::NotIs),
            });
        }

        // 2) 单 token 的 keyword 运算符：`is` / `as` / `as?`
        if let TokenKind::Keyword(kw) = self.peek().kind {
            if kw == Keyword::Is && self.looks_like_when_is_arm_start() {
                return None;
            }

            let kind = match kw {
                Keyword::Is => InfixOpKind::TypeCheck(ast::TypeCheckOp::Is),
                Keyword::As => InfixOpKind::Cast(ast::CastOp::As),
                Keyword::AsQ => InfixOpKind::Cast(ast::CastOp::AsQ),
                _ => return None,
            };
            return Some(InfixOp {
                l_bp: 8,
                r_bp: 9,
                consume: 1,
                kind,
            });
        }

        // 3) 普通 symbol 二元运算符
        let TokenKind::Symbol(sym) = self.peek().kind else {
            return None;
        };
        let (l_bp, r_bp, op) = binary_binding_power(sym)?;
        Some(InfixOp {
            l_bp,
            r_bp,
            consume: 1,
            kind: InfixOpKind::Binary(op),
        })
    }

    fn looks_like_when_is_arm_start(&self) -> bool {
        if self.peek().kind != TokenKind::Keyword(Keyword::Is) {
            return false;
        }

        let Some(end) = scan_type_ref_end(&self.tokens, self.i + 1) else {
            return false;
        };

        self.tokens
            .get(end)
            .map(|t| t.kind == TokenKind::Symbol(Symbol::Arrow))
            .unwrap_or(false)
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

        if self.peek_keyword(Keyword::If) {
            return Ok(Some(self.parse_if_expr()?));
        }

        if self.peek_keyword(Keyword::When) {
            return Ok(Some(self.parse_when_expr()?));
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

    fn parse_if_expr(&mut self) -> Result<ast::Expr, ParseError> {
        let if_kw = self.expect_keyword(Keyword::If)?;
        let start = if_kw.span.start;

        let open = self.expect_symbol(Symbol::LParen)?;

        let tok = *self.peek();
        let cond = self.try_parse_expr()?.ok_or(ParseError::Expected {
            expected: "表达式（if 条件）",
            found: tok.kind,
            span: tok.span.into(),
        })?;

        if self.peek_kind(TokenKind::Eof) {
            return Err(ParseError::UnterminatedGroup {
                close: Symbol::RParen,
                span: Span::new(open.span.start, self.peek().span.end).into(),
            });
        }
        self.expect_symbol(Symbol::RParen)?;

        let tok = *self.peek();
        let then_branch = self.try_parse_expr()?.ok_or(ParseError::Expected {
            expected: "表达式（then 分支）",
            found: tok.kind,
            span: tok.span.into(),
        })?;

        let (end, else_branch) = if self.peek_keyword(Keyword::Else) {
            self.bump();
            let tok = *self.peek();
            let else_expr = self.try_parse_expr()?.ok_or(ParseError::Expected {
                expected: "表达式（else 分支）",
                found: tok.kind,
                span: tok.span.into(),
            })?;
            (else_expr.span.end, Some(Box::new(else_expr)))
        } else {
            (then_branch.span.end, None)
        };

        Ok(ast::Expr {
            span: Span::new(start, end),
            kind: ast::ExprKind::If {
                cond: Box::new(cond),
                then_branch: Box::new(then_branch),
                else_branch,
            },
        })
    }

    fn parse_when_expr(&mut self) -> Result<ast::Expr, ParseError> {
        let when_kw = self.expect_keyword(Keyword::When)?;
        let start = when_kw.span.start;

        let open_paren = self.expect_symbol(Symbol::LParen)?;

        let tok = *self.peek();
        let subject = self.try_parse_expr()?.ok_or(ParseError::Expected {
            expected: "表达式（when subject）",
            found: tok.kind,
            span: tok.span.into(),
        })?;

        if self.peek_kind(TokenKind::Eof) {
            return Err(ParseError::UnterminatedGroup {
                close: Symbol::RParen,
                span: Span::new(open_paren.span.start, self.peek().span.end).into(),
            });
        }
        self.expect_symbol(Symbol::RParen)?;

        let open_brace = self.expect_symbol(Symbol::LBrace)?;
        let mut arms = Vec::new();

        while !self.peek_kind(TokenKind::Eof) && !self.peek_symbol(Symbol::RBrace) {
            // 允许多余的 `;`（类似 block 内的空语句）
            while self.eat_symbol(Symbol::Semicolon) {}
            if self.peek_symbol(Symbol::RBrace) {
                break;
            }

            let pat = self.parse_when_pat()?;
            let pat_span = pat.span();
            let arrow = self.expect_symbol(Symbol::Arrow)?;

            let tok = *self.peek();
            let body = self.try_parse_expr_in_when_arm()?.ok_or(ParseError::Expected {
                expected: "表达式（when 分支 body）",
                found: tok.kind,
                span: tok.span.into(),
            })?;

            arms.push(ast::WhenArm {
                span: Span::new(pat_span.start, body.span.end),
                pat,
                arrow_span: arrow.span,
                body,
            });

            while self.eat_symbol(Symbol::Semicolon) {}
        }

        if self.peek_kind(TokenKind::Eof) {
            return Err(ParseError::UnterminatedGroup {
                close: Symbol::RBrace,
                span: Span::new(open_brace.span.start, self.peek().span.end).into(),
            });
        }
        let close_brace = self.expect_symbol(Symbol::RBrace)?;

        Ok(ast::Expr {
            span: Span::new(start, close_brace.span.end),
            kind: ast::ExprKind::When {
                subject: Box::new(subject),
                arms,
            },
        })
    }

    fn parse_when_pat(&mut self) -> Result<ast::WhenPat, ParseError> {
        if self.peek_keyword(Keyword::Else) {
            let tok = self.bump();
            return Ok(ast::WhenPat::Else { span: tok.span });
        }

        if self.peek_keyword(Keyword::Is) {
            let is_tok = self.bump();
            let ty = self.parse_type_ref()?;
            return Ok(ast::WhenPat::Is {
                is_span: is_tok.span,
                ty,
            });
        }

        if self.peek_kind(TokenKind::IntLiteral) {
            let tok = self.bump();
            return Ok(ast::WhenPat::IntLit { span: tok.span });
        }

        if matches!(self.peek().kind, TokenKind::StringLiteral(_)) {
            let tok = self.bump();
            return Ok(ast::WhenPat::StringLit { span: tok.span });
        }

        let tok = *self.peek();
        Err(ParseError::Expected {
            expected: "when 分支模式（`else` / `is T` / 字面量）",
            found: tok.kind,
            span: tok.span.into(),
        })
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

    fn parse_not_null_assert_expr(&mut self, receiver: ast::Expr) -> Result<ast::Expr, ParseError> {
        let op_tok = self.expect_symbol(Symbol::BangBang)?;
        Ok(ast::Expr {
            span: Span::new(receiver.span.start, op_tok.span.end),
            kind: ast::ExprKind::NotNullAssert {
                expr: Box::new(receiver),
                op_span: op_tok.span,
            },
        })
    }

    fn parse_with_update_expr(&mut self, base: ast::Expr) -> Result<ast::Expr, ParseError> {
        let with_kw = self.expect_keyword(Keyword::With)?;
        let start = base.span.start;

        let open = self.expect_symbol(Symbol::LBrace)?;
        let mut updates = Vec::new();

        while !self.peek_kind(TokenKind::Eof) && !self.peek_symbol(Symbol::RBrace) {
            let path = self.parse_field_path()?;
            let colon = self.expect_symbol(Symbol::Colon)?;

            let tok = *self.peek();
            let value = self.try_parse_expr()?.ok_or(ParseError::Expected {
                expected: "表达式（with 更新值）",
                found: tok.kind,
                span: tok.span.into(),
            })?;

            updates.push(ast::WithUpdateField {
                span: Span::new(path.span.start, value.span.end),
                path,
                colon_span: colon.span,
                value,
            });

            if self.eat_symbol(Symbol::Comma) {
                // allow trailing comma
                if self.peek_symbol(Symbol::RBrace) {
                    break;
                }
                continue;
            }

            break;
        }

        if self.peek_kind(TokenKind::Eof) {
            return Err(ParseError::UnterminatedGroup {
                close: Symbol::RBrace,
                span: Span::new(open.span.start, self.peek().span.end).into(),
            });
        }
        let close = self.expect_symbol(Symbol::RBrace)?;

        Ok(ast::Expr {
            span: Span::new(start, close.span.end),
            kind: ast::ExprKind::WithUpdate {
                base: Box::new(base),
                with_span: with_kw.span,
                updates,
            },
        })
    }

    fn parse_field_path(&mut self) -> Result<ast::FieldPath, ParseError> {
        let first = self.expect_kind(TokenKind::Ident, "字段路径（标识符）")?;
        let start = first.span.start;

        let mut segments = vec![ast::Ident { span: first.span }];
        while self.eat_symbol(Symbol::Dot) {
            let seg = self.expect_kind(TokenKind::Ident, "字段路径（标识符）")?;
            segments.push(ast::Ident { span: seg.span });
        }

        let end = segments
            .last()
            .map(|x| x.span.end)
            .unwrap_or(first.span.end);

        Ok(ast::FieldPath {
            span: Span::new(start, end),
            segments,
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

fn scan_type_ref_end(tokens: &[crate::syntax::token::Token], start: usize) -> Option<usize> {
    scan_type_ref_end_inner(tokens, start).map(|mut i| {
        if matches!(kind_at(tokens, i), TokenKind::Symbol(Symbol::Question)) {
            i += 1;
        }
        i
    })
}

fn scan_type_ref_end_inner(tokens: &[crate::syntax::token::Token], start: usize) -> Option<usize> {
    match kind_at(tokens, start) {
        TokenKind::Symbol(Symbol::LParen) => scan_tuple_or_group_type_end(tokens, start),
        _ => scan_path_type_end(tokens, start),
    }
}

fn scan_tuple_or_group_type_end(tokens: &[crate::syntax::token::Token], start: usize) -> Option<usize> {
    if !matches!(kind_at(tokens, start), TokenKind::Symbol(Symbol::LParen)) {
        return None;
    }

    let mut i = start + 1;
    if matches!(kind_at(tokens, i), TokenKind::Symbol(Symbol::RParen)) {
        return Some(i + 1);
    }

    i = scan_type_ref_end(tokens, i)?;

    if matches!(kind_at(tokens, i), TokenKind::Symbol(Symbol::Comma)) {
        i += 1;
        while !matches!(kind_at(tokens, i), TokenKind::Symbol(Symbol::RParen) | TokenKind::Eof) {
            i = scan_type_ref_end(tokens, i)?;
            if matches!(kind_at(tokens, i), TokenKind::Symbol(Symbol::Comma)) {
                i += 1;
                // allow trailing comma
                if matches!(kind_at(tokens, i), TokenKind::Symbol(Symbol::RParen)) {
                    break;
                }
                continue;
            }
            break;
        }
        if !matches!(kind_at(tokens, i), TokenKind::Symbol(Symbol::RParen)) {
            return None;
        }
        return Some(i + 1);
    }

    if !matches!(kind_at(tokens, i), TokenKind::Symbol(Symbol::RParen)) {
        return None;
    }
    Some(i + 1)
}

fn scan_path_type_end(tokens: &[crate::syntax::token::Token], start: usize) -> Option<usize> {
    if !matches!(kind_at(tokens, start), TokenKind::Ident) {
        return None;
    }

    let mut i = start + 1;
    while matches!(kind_at(tokens, i), TokenKind::Symbol(Symbol::Dot)) {
        i += 1;
        if !matches!(kind_at(tokens, i), TokenKind::Ident) {
            return None;
        }
        i += 1;
    }

    if matches!(kind_at(tokens, i), TokenKind::Symbol(Symbol::Lt)) {
        i = scan_type_args_end(tokens, i)?;
    }

    Some(i)
}

fn scan_type_args_end(tokens: &[crate::syntax::token::Token], start: usize) -> Option<usize> {
    if !matches!(kind_at(tokens, start), TokenKind::Symbol(Symbol::Lt)) {
        return None;
    }

    let mut i = start + 1;

    if matches!(kind_at(tokens, i), TokenKind::Symbol(Symbol::Gt)) {
        return Some(i + 1);
    }

    loop {
        i = scan_type_ref_end(tokens, i)?;
        if matches!(kind_at(tokens, i), TokenKind::Symbol(Symbol::Comma)) {
            i += 1;
            if matches!(kind_at(tokens, i), TokenKind::Symbol(Symbol::Gt)) {
                break;
            }
            continue;
        }
        break;
    }

    if !matches!(kind_at(tokens, i), TokenKind::Symbol(Symbol::Gt)) {
        return None;
    }

    Some(i + 1)
}

fn kind_at(tokens: &[crate::syntax::token::Token], i: usize) -> TokenKind {
    tokens.get(i).map(|t| t.kind).unwrap_or(TokenKind::Eof)
}

#[derive(Debug, Clone, Copy)]
struct InfixOp {
    l_bp: u8,
    r_bp: u8,
    /// 该运算符在 token 流中占用的 token 数量（`!is` 为 2，其余为 1）。
    consume: u8,
    kind: InfixOpKind,
}

#[derive(Debug, Clone, Copy)]
enum InfixOpKind {
    Binary(ast::BinaryOp),
    TypeCheck(ast::TypeCheckOp),
    Cast(ast::CastOp),
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
    // - elvis: ?:
    //
    // 说明：
    // - 大多数二元运算符按“左结合”处理；
    // - Elvis `?:` 按“右结合”处理（与 Kotlin 类似）：`a ?: b ?: c` 解析为 `a ?: (b ?: c)`。
    //
    // Pratt/precedence climbing：
    // - 左结合：使用 (prec, prec + 1)
    // - 右结合：使用 (prec, prec)
    match sym {
        Symbol::Star => Some((11, 12, ast::BinaryOp::Mul)),
        Symbol::Slash => Some((11, 12, ast::BinaryOp::Div)),
        Symbol::Percent => Some((11, 12, ast::BinaryOp::Rem)),

        Symbol::Plus => Some((10, 11, ast::BinaryOp::Add)),
        Symbol::Minus => Some((10, 11, ast::BinaryOp::Sub)),

        Symbol::LtLt => Some((9, 10, ast::BinaryOp::Shl)),
        Symbol::GtGt => Some((9, 10, ast::BinaryOp::Shr)),

        Symbol::Lt => Some((8, 9, ast::BinaryOp::Lt)),
        Symbol::LtEq => Some((8, 9, ast::BinaryOp::Le)),
        Symbol::Gt => Some((8, 9, ast::BinaryOp::Gt)),
        Symbol::GtEq => Some((8, 9, ast::BinaryOp::Ge)),

        Symbol::EqEq => Some((7, 8, ast::BinaryOp::Eq)),
        Symbol::BangEq => Some((7, 8, ast::BinaryOp::Ne)),

        Symbol::And => Some((6, 7, ast::BinaryOp::BitAnd)),
        Symbol::Caret => Some((5, 6, ast::BinaryOp::BitXor)),
        Symbol::Or => Some((4, 5, ast::BinaryOp::BitOr)),

        Symbol::AndAnd => Some((3, 4, ast::BinaryOp::LogAnd)),
        Symbol::OrOr => Some((2, 3, ast::BinaryOp::LogOr)),

        // Elvis：比 `||` 更低一档的二元（但仍高于未来可能出现的赋值/控制流）。
        Symbol::Elvis => Some((1, 1, ast::BinaryOp::Elvis)),

        _ => None,
    }
}
