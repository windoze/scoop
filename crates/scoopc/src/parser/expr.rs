//! 表达式解析（早期最小子集）。
//!
//! 当前阶段实现：
//! - “原子表达式”（T0206）：
//! - ident
//! - int literal
//! - string literal
//! - 括号分组 `( ... )`（仅在内部也是原子表达式时才保留其 kind，否则降级为 Missing）
//! - lambda 表达式（T0222）：`{ params -> body }` / `{ body }`
//! - struct literal（T0224）：`Point { x: 1, y: 2 }`
//! - postfix 调用表达式（T0209）：`callee(args...)`
//! - postfix 成员访问（T0210）：`receiver.member`
//! - postfix safe-call（T0229）：`receiver?.member` / `receiver?.call(...)`
//! - postfix 非空断言（T0212）：`expr!!`
//! - prefix 一元运算（T0252）：`!expr` / `-expr` / `~expr`
//! - 二元运算符优先级（T0211）：`1 + 2 * 3`
//! - Elvis（T0212）：`a ?: b`
//! - 类型判断/转换（T0213）：`is`/`!is`/`as`/`as?`
//! - `if` 表达式（T0214）：`if (cond) thenExpr else elseExpr?`
//! - `when` 表达式（T0215）：`when (expr) { ... }`（最小分支子集）
//! - 赋值表达式（T0227）：`lhs = rhs`（lhs 先限 ident/member）
//! - unsafe block（T1004）：`@Unsafe { ... }`
//!
//! 说明：
//! - 该模块的目标是支撑顶层 `val/var` initializer 的增量解析；
//! - 更复杂的表达式（调用/成员访问/二元运算/控制流等）会在后续任务中逐步补齐。

use std::cell::OnceCell;

use crate::ast;
use crate::span::Span;
use crate::syntax::lexer::{LexError, lex};
use crate::syntax::token::{Keyword, StringKind, Symbol, Token, TokenKind};

use super::{ParseError, Parser};

impl<'a> Parser<'a> {
    /// 尝试解析一个表达式（当前为：postfix + 常见二元运算符优先级）。
    ///
    /// - 若当前位置不是表达式的起始 token，则返回 `Ok(None)` 且不消费 token。
    /// - 若能解析出表达式，则返回 `Ok(Some(expr))`。
    pub(super) fn try_parse_expr(&mut self) -> Result<Option<ast::Expr>, ParseError> {
        self.parse_assign_expr()
    }

    /// `when` 分支 body 的表达式解析（带 arm 边界规则）。
    ///
    /// 由于 lexer 不保留换行，`when { ... }` 的 arm 列表需要依靠 token 形态推断边界：
    /// - 若看到 `is <TypeRef> ->`，优先将其解释为“下一个 arm 的 pattern 起始”，
    ///   而不是把 `is` 当成当前表达式的中缀类型判断运算符。
    fn try_parse_expr_in_when_arm(&mut self) -> Result<Option<ast::Expr>, ParseError> {
        self.parse_assign_expr_in_when_arm()
    }

    fn parse_assign_expr(&mut self) -> Result<Option<ast::Expr>, ParseError> {
        let Some(lhs) = self.parse_expr_bp(0)? else {
            return Ok(None);
        };

        if !self.peek_symbol(Symbol::Eq) {
            return Ok(Some(lhs));
        }

        if !is_assignable_lhs(&lhs) {
            return Err(ParseError::Expected {
                expected: "可赋值的左值（标识符或成员访问）",
                found: self.peek().kind,
                span: lhs.span.into(),
            });
        }

        let eq = self.expect_symbol(Symbol::Eq)?;
        let tok = *self.peek();
        let rhs = self.parse_assign_expr()?.ok_or(ParseError::Expected {
            expected: "表达式（赋值右侧）",
            found: tok.kind,
            span: tok.span.into(),
        })?;

        Ok(Some(ast::Expr {
            span: Span::new(lhs.span.start, rhs.span.end),
            kind: ast::ExprKind::Assign {
                lhs: Box::new(lhs),
                eq_span: eq.span,
                rhs: Box::new(rhs),
            },
        }))
    }

    fn parse_assign_expr_in_when_arm(&mut self) -> Result<Option<ast::Expr>, ParseError> {
        let Some(lhs) = self.parse_expr_bp_in_when_arm(0)? else {
            return Ok(None);
        };

        if !self.peek_symbol(Symbol::Eq) {
            return Ok(Some(lhs));
        }

        if !is_assignable_lhs(&lhs) {
            return Err(ParseError::Expected {
                expected: "可赋值的左值（标识符或成员访问）",
                found: self.peek().kind,
                span: lhs.span.into(),
            });
        }

        let eq = self.expect_symbol(Symbol::Eq)?;
        let tok = *self.peek();
        let rhs = self
            .parse_assign_expr_in_when_arm()?
            .ok_or(ParseError::Expected {
                expected: "表达式（赋值右侧）",
                found: tok.kind,
                span: tok.span.into(),
            })?;

        Ok(Some(ast::Expr {
            span: Span::new(lhs.span.start, rhs.span.end),
            kind: ast::ExprKind::Assign {
                lhs: Box::new(lhs),
                eq_span: eq.span,
                rhs: Box::new(rhs),
            },
        }))
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
            // Kotlin-like 显式类型实参（T1204）：`callee<T>()`
            //
            // 说明：
            // - `<` 同时也是二元运算符，因此这里必须做 lookahead 消歧；
            // - 当前阶段只在 `>` 后紧跟 `(` 时才把它解释为 “type args + call” 的前半段，
            //   以避免把普通比较表达式误判为类型实参应用。
            if self.looks_like_type_apply_then_call() {
                expr = self.parse_type_apply_expr(expr)?;
                continue;
            }
            // Kotlin-like class literal：`TypeName::class`（T1019）。
            //
            // 说明：
            // - 这里不引入新的 token（仍用两个 `:`）；解析时仅在 `::` 后紧跟 `class` 时成立；
            // - 左侧要求是“类型名路径”（`Ident(.Ident)*`），由 parser 在此处做形态约束。
            if self.peek_symbol(Symbol::Colon)
                && self.peek_n(1).kind == TokenKind::Symbol(Symbol::Colon)
                && self.peek_n(2).kind == TokenKind::Keyword(Keyword::Class)
            {
                expr = self.parse_class_lit_expr(expr)?;
                continue;
            }
            if self.peek_symbol(Symbol::Dot) {
                expr = self.parse_member_access_expr(expr)?;
                continue;
            }
            if self.peek_symbol(Symbol::QuestionDot) {
                expr = self.parse_safe_member_access_expr(expr)?;
                continue;
            }
            if self.peek_symbol(Symbol::LParen) {
                expr = self.parse_call_expr(expr)?;
                continue;
            }
            if self.peek_symbol(Symbol::LBrace) {
                // Kotlin 风格 trailing lambda：`callee { ... }`（spec §12 / Appendix B.5.4）。
                expr = if matches!(expr.kind, ast::ExprKind::Call { .. }) {
                    self.parse_trailing_lambda_append_call_expr(expr)?
                } else {
                    self.parse_trailing_lambda_call_expr(expr)?
                };
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

    fn looks_like_type_apply_then_call(&self) -> bool {
        if !self.peek_symbol(Symbol::Lt) {
            return false;
        }
        let Some(end) = scan_type_args_end(&self.tokens, self.i) else {
            return false;
        };
        matches!(
            kind_at(&self.tokens, end),
            TokenKind::Symbol(Symbol::LParen)
        )
    }

    fn parse_type_apply_expr(&mut self, callee: ast::Expr) -> Result<ast::Expr, ParseError> {
        let start = callee.span.start;
        let (args, end) = self.parse_type_args()?;
        Ok(ast::Expr {
            span: Span::new(start, end),
            kind: ast::ExprKind::TypeApply {
                callee: Box::new(callee),
                args,
            },
        })
    }

    fn try_parse_expr_prefix(&mut self) -> Result<Option<ast::Expr>, ParseError> {
        // spec §15.9.2：`@Unsafe { ... }`（unsafe block）。
        // spec §15.9.5：`@Safe { ... }`（safe block：在 unsafe context 内收窄为 safe）。
        //
        // 说明：
        // - 该语法位于表达式/语句层：作为一个“局部 unsafe context”区域；
        // - 当前阶段仅支持内建 `Unsafe/Safe`，不支持任意注解作为 block 前缀；
        // - 必须紧跟一个 block：`@Unsafe { ... }` / `@Safe { ... }`。
        if self.peek_symbol(Symbol::At) && self.peek_n(1).kind == TokenKind::Ident {
            match self
                .source_text
                .get(self.peek_n(1).span.start..self.peek_n(1).span.end)
            {
                Some("Unsafe") => return Ok(Some(self.parse_unsafe_block_expr()?)),
                Some("Safe") => return Ok(Some(self.parse_safe_block_expr()?)),
                _ => {}
            }
        }

        // spec §5.7：`spawn { ... }`（结构化并发语法糖，T0620）。
        //
        // 说明：
        // - 为避免与 Kotlin 风格 trailing lambda 的 `spawn { ... }`（call + lambda）形态冲突，
        //   这里把 `spawn` 作为“上下文关键字”在前缀位置优先解析为独立语法节点；
        // - 当前阶段只支持紧跟一个 block：`spawn { ... }`。
        if self.peek_ident_text("spawn") {
            let spawn_kw = self.bump(); // `spawn`（ident）
            let start = spawn_kw.span.start;

            let body = self.parse_block()?;
            return Ok(Some(ast::Expr {
                span: Span::new(start, body.span.end),
                kind: ast::ExprKind::Spawn { body },
            }));
        }

        // T0620：`join expr`（结构化并发最小语法糖）。
        //
        // 说明：
        // - lexer 当前把 `join` 作为 ident（上下文关键字），因此这里通过字面文本判别；
        // - `join` 作为前缀操作符，优先级与 `await`/`!`/`-` 等前缀一元运算对齐。
        if self.peek_ident_text("join") {
            let join_kw = self.bump(); // `join`（ident）
            let tok = *self.peek();
            let expr = self.try_parse_expr_prefix()?.ok_or(ParseError::Expected {
                expected: "表达式（join 的操作数）",
                found: tok.kind,
                span: tok.span.into(),
            })?;

            return Ok(Some(ast::Expr {
                span: Span::new(join_kw.span.start, expr.span.end),
                kind: ast::ExprKind::Join {
                    join_span: join_kw.span,
                    expr: Box::new(expr),
                },
            }));
        }

        // spec §5.7：`await expr`（作为 Async effect 的语法糖）。
        //
        // 说明：
        // - lexer 当前把 `await` 作为 ident（上下文关键字），因此这里通过字面文本判别；
        // - `await` 作为前缀操作符，优先级与 `!`/`-` 等前缀一元运算对齐；
        // - 具体 lowering（例如 desugar 成 `Async.await(...)` 的 perform 点）由后续阶段完成。
        if self.peek_ident_text("await") {
            let await_kw = self.bump(); // `await`（ident）
            let tok = *self.peek();
            let expr = self.try_parse_expr_prefix()?.ok_or(ParseError::Expected {
                expected: "表达式（await 的操作数）",
                found: tok.kind,
                span: tok.span.into(),
            })?;

            return Ok(Some(ast::Expr {
                span: Span::new(await_kw.span.start, expr.span.end),
                kind: ast::ExprKind::Await {
                    await_span: await_kw.span,
                    expr: Box::new(expr),
                },
            }));
        }

        let TokenKind::Symbol(sym) = self.peek().kind else {
            return self.try_parse_expr_postfix();
        };

        let op = match sym {
            Symbol::Bang => ast::UnaryOp::Not,
            Symbol::Minus => ast::UnaryOp::Neg,
            Symbol::Tilde => ast::UnaryOp::BitNot,
            _ => return self.try_parse_expr_postfix(),
        };

        let op_tok = self.bump();
        let tok = *self.peek();
        let expr = self.try_parse_expr_prefix()?.ok_or(ParseError::Expected {
            expected: "表达式（前缀一元运算的操作数）",
            found: tok.kind,
            span: tok.span.into(),
        })?;

        Ok(Some(ast::Expr {
            span: Span::new(op_tok.span.start, expr.span.end),
            kind: ast::ExprKind::Unary {
                op,
                op_span: op_tok.span,
                expr: Box::new(expr),
            },
        }))
    }

    fn parse_unsafe_block_expr(&mut self) -> Result<ast::Expr, ParseError> {
        let at = self.expect_symbol(Symbol::At)?;
        let start = at.span.start;

        // 当前阶段仅支持 `@Unsafe`。
        let unsafe_kw = self.expect_kind(TokenKind::Ident, "`Unsafe`（unsafe block 注解名）")?;
        debug_assert_eq!(
            self.source_text
                .get(unsafe_kw.span.start..unsafe_kw.span.end),
            Some("Unsafe")
        );
        let at_unsafe_span = Span::new(start, unsafe_kw.span.end);

        let body = self.parse_block()?;
        Ok(ast::Expr {
            span: Span::new(start, body.span.end),
            kind: ast::ExprKind::UnsafeBlock {
                at_unsafe_span,
                body,
            },
        })
    }

    fn parse_safe_block_expr(&mut self) -> Result<ast::Expr, ParseError> {
        let at = self.expect_symbol(Symbol::At)?;
        let start = at.span.start;

        let safe_kw = self.expect_kind(TokenKind::Ident, "`Safe`（safe block 注解名）")?;
        debug_assert_eq!(
            self.source_text.get(safe_kw.span.start..safe_kw.span.end),
            Some("Safe")
        );
        let at_safe_span = Span::new(start, safe_kw.span.end);

        let body = self.parse_block()?;
        Ok(ast::Expr {
            span: Span::new(start, body.span.end),
            kind: ast::ExprKind::SafeBlock { at_safe_span, body },
        })
    }

    fn parse_expr_bp(&mut self, min_bp: u8) -> Result<Option<ast::Expr>, ParseError> {
        let Some(mut lhs) = self.try_parse_expr_prefix()? else {
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
        let Some(mut lhs) = self.try_parse_expr_prefix()? else {
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
            // struct literal / trailing lambda 的 `{}` 歧义消解（spec §12）：
            // - `TypeName { x: 1 }`：struct literal
            // - `callee { it }`：trailing lambda（Kotlin 风格）
            //
            // 解析策略：
            // - 若 `{ ... }` 形态“更像 struct literal fields”，在这里把它吃成 struct literal；
            // - 否则把当前 `Ident` 先解析为普通表达式，留给 postfix 的 trailing lambda 逻辑消费 `{ ... }`。
            if self.peek_n(1).kind == TokenKind::Symbol(Symbol::LBrace)
                && self.disambiguate_ident_lbrace_group() == BraceGroupKind::StructLit
            {
                return Ok(Some(self.parse_struct_lit_expr()?));
            }

            let tok = self.bump();
            return Ok(Some(ast::Expr {
                span: tok.span,
                kind: ast::ExprKind::Ident(ast::ValueIdent::new(tok.span)),
            }));
        }

        if self.peek_kind(TokenKind::IntLiteral) {
            let tok = self.bump();
            return Ok(Some(ast::Expr {
                span: tok.span,
                kind: ast::ExprKind::IntLit,
            }));
        }

        if let TokenKind::StringLiteral(string_kind) = self.peek().kind {
            let tok = self.bump();
            return Ok(Some(match string_kind {
                StringKind::Normal { interpolated: true } => {
                    self.parse_interpolated_string_expr(tok, false)?
                }
                StringKind::Raw { interpolated: true } => {
                    self.parse_interpolated_string_expr(tok, true)?
                }
                _ => ast::Expr {
                    span: tok.span,
                    kind: ast::ExprKind::StringLit,
                },
            }));
        }

        if self.peek_keyword(Keyword::If) {
            return Ok(Some(self.parse_if_expr()?));
        }

        if self.peek_keyword(Keyword::When) {
            return Ok(Some(self.parse_when_expr()?));
        }

        if self.peek_keyword(Keyword::Handle) {
            return Ok(Some(self.parse_handle_expr()?));
        }

        if self.peek_keyword(Keyword::Try) {
            return Ok(Some(self.parse_try_expr()?));
        }

        if self.peek_keyword(Keyword::Async) {
            return Ok(Some(self.parse_async_expr()?));
        }

        if self.peek_symbol(Symbol::LBrace) {
            return Ok(Some(self.parse_lambda_expr()?));
        }

        if self.peek_symbol(Symbol::LParen) {
            return self.try_parse_paren_group_expr();
        }

        if self.peek_symbol(Symbol::LBracket) {
            return Ok(Some(self.parse_array_lit_expr()?));
        }

        Ok(None)
    }

    /// 解析 `async { ... }`（spec §5.7）。
    ///
    /// 当前阶段：
    /// - 仅支持 block 形式（与 `handle`/`try` 的早期约束对齐）；
    /// - async 的具体语义将由 typecheck/lowering 落地（TODO T0619）。
    fn parse_async_expr(&mut self) -> Result<ast::Expr, ParseError> {
        let async_kw = self.expect_keyword(Keyword::Async)?;
        let start = async_kw.span.start;

        let body = self.parse_block()?;
        Ok(ast::Expr {
            span: Span::new(start, body.span.end),
            kind: ast::ExprKind::Async { body },
        })
    }

    /// 解析 `handle { ... } with { ... }`（spec §5.4）。
    ///
    /// 当前阶段：
    /// - 支持 non-resuming arm：`Effect.op(binders...) -> body`；
    /// - 支持 immediate-resume arm：`Effect.op(binders...) -> resume { ... }`（T0616）；
    /// - 语法错误在 arm 级别做恢复：尽量跳到下一个 arm 起始继续解析；
    /// - 支持 escape continuation arm：`Effect.op(binders...), k -> body`（T0617）。
    fn parse_handle_expr(&mut self) -> Result<ast::Expr, ParseError> {
        let handle_kw = self.expect_keyword(Keyword::Handle)?;
        let start = handle_kw.span.start;

        // spec 示例固定使用 block：`handle { ... } ...`
        let body = self.parse_block()?;

        self.expect_keyword(Keyword::With)?;

        let open = self.expect_symbol(Symbol::LBrace)?;
        let mut arms = Vec::new();

        while !self.peek_kind(TokenKind::Eof) && !self.peek_symbol(Symbol::RBrace) {
            // 允许多余的 `;`（类似 block 内的空语句）
            while self.eat_symbol(Symbol::Semicolon) {}
            if self.peek_symbol(Symbol::RBrace) {
                break;
            }

            match self.parse_handle_arm() {
                Ok(arm) => arms.push(arm),
                Err(e) => {
                    self.record_error(e);
                    self.recover_to_handle_arm_sync();
                }
            }

            while self.eat_symbol(Symbol::Semicolon) {}
        }

        if self.peek_kind(TokenKind::Eof) {
            return Err(ParseError::UnterminatedGroup {
                close: Symbol::RBrace,
                span: Span::new(open.span.start, self.peek().span.end).into(),
            });
        }
        let close = self.expect_symbol(Symbol::RBrace)?;

        // spec §5.7：`handle { ... } with { ... } finally { ... }`
        let finally = if self.peek_keyword(Keyword::Finally) {
            self.bump(); // `finally`
            Some(self.parse_block()?)
        } else {
            None
        };

        let end = finally
            .as_ref()
            .map(|b| b.span.end)
            .unwrap_or(close.span.end);

        Ok(ast::Expr {
            span: Span::new(start, end),
            kind: ast::ExprKind::Handle {
                body,
                arms,
                finally,
            },
        })
    }

    /// 解析 `try { ... } catch (e: T) { ... } ... finally { ... }`（spec §5.7），并在 parser
    /// 层 desugar 为 `handle` 表达式（T0607 / T0631）。
    ///
    /// 当前阶段约束：
    /// - 支持多个 `catch` arm（按书写顺序存入 `Handle { arms: ... }`）；
    /// - finally 可选；
    /// - try body 与每个 catch body 暂要求都是 block（不支持 catch-only 表达式体）。
    fn parse_try_expr(&mut self) -> Result<ast::Expr, ParseError> {
        let try_kw = self.expect_keyword(Keyword::Try)?;
        let start = try_kw.span.start;

        let body = self.parse_block()?;

        // spec §5.7：至少一个 catch。
        let mut arms: Vec<ast::HandleArm> = Vec::new();
        let mut last_catch_end = body.span.end;

        let mut first = true;
        while first || self.peek_keyword(Keyword::Catch) {
            first = false;
            let catch_kw = self.expect_keyword(Keyword::Catch)?;
            let catch_kw_span = catch_kw.span;

            let open_paren = self.expect_symbol(Symbol::LParen)?;
            let binder_tok = self.expect_kind(TokenKind::Ident, "catch 变量名（标识符）")?;
            let binder_name = ast::Ident::new(binder_tok.span);

            let colon = self.expect_symbol(Symbol::Colon)?;
            let binder_ty = self.parse_type_ref()?;

            if self.peek_kind(TokenKind::Eof) {
                return Err(ParseError::UnterminatedGroup {
                    close: Symbol::RParen,
                    span: Span::new(open_paren.span.start, self.peek().span.end).into(),
                });
            }
            let close_paren = self.expect_symbol(Symbol::RParen)?;

            let catch_block = self.parse_block()?;
            let catch_body = ast::Expr {
                span: catch_block.span,
                kind: ast::ExprKind::Block(catch_block),
            };
            last_catch_end = catch_body.span.end;

            // lowering：`try/catch` 等价于捕获 `scoop.core.Raise.raise` 的 handle。
            //
            // 注意：try/catch 语法本身不显式出现 `Raise.raise`，因此这里使用合成 Ident，
            // 让后续 typecheck 能直接解析到 sysroot 中的 effect op。
            let synth_span = catch_kw_span;
            let dot_span = Span::new(synth_span.start, synth_span.start);

            let effect = ast::TypePath {
                span: synth_span,
                segments: vec![
                    ast::Ident::synthetic(synth_span, "scoop"),
                    ast::Ident::synthetic(synth_span, "core"),
                    ast::Ident::synthetic(synth_span, "Raise"),
                ],
                args: Vec::new(),
            };

            let op = ast::HandleOp {
                span: Span::new(catch_kw_span.start, close_paren.span.end),
                effect,
                dot_span,
                op: ast::Ident::synthetic(synth_span, "raise"),
                binders: vec![ast::HandleBinder {
                    span: Span::new(binder_tok.span.start, binder_ty.span().end),
                    name: binder_name,
                    colon_span: Some(colon.span),
                    ty: Some(binder_ty),
                }],
            };

            let arm = ast::HandleArm {
                span: Span::new(op.span.start, catch_body.span.end),
                op,
                // 语法糖并没有显式 `->`；这里用 `catch` 关键字 span 作为占位，
                // 以便诊断（若有）能落在 try/catch 区域内。
                arrow_span: catch_kw_span,
                kind: ast::HandleArmKind::NonResuming,
                body: catch_body,
            };

            arms.push(arm);
        }

        // spec §5.7：finally 可选。
        let finally = if self.peek_keyword(Keyword::Finally) {
            self.bump(); // `finally`
            Some(self.parse_block()?)
        } else {
            None
        };

        let end = finally
            .as_ref()
            .map(|b| b.span.end)
            .unwrap_or(last_catch_end);

        Ok(ast::Expr {
            span: Span::new(start, end),
            kind: ast::ExprKind::Handle {
                body,
                arms,
                finally,
            },
        })
    }

    fn parse_handle_arm(&mut self) -> Result<ast::HandleArm, ParseError> {
        let op = self.parse_handle_op()?;

        // spec §5.4：`Effect.op(...), k -> { ... }`（escape continuation）。
        if self.eat_symbol(Symbol::Comma) {
            let k_tok = self.expect_kind(TokenKind::Ident, "continuation binder（标识符）")?;
            let arrow = self.expect_symbol(Symbol::Arrow)?;
            let body = self.parse_control_body_expr("表达式（handler arm body）")?;

            return Ok(ast::HandleArm {
                span: Span::new(op.span.start, body.span.end),
                op,
                arrow_span: arrow.span,
                kind: ast::HandleArmKind::EscapeContinuation { k_span: k_tok.span },
                body,
            });
        }

        let arrow = self.expect_symbol(Symbol::Arrow)?;

        // spec §5.4：`-> resume { ... }`（immediate-resume）。
        //
        // 注意：`resume` 在 lexer 层仍是 ident；这里以“`resume` + `{`”的形态做语法判别，
        // 避免把它误解析为普通 call 表达式（Kotlin 风格的 trailing lambda）。
        if self.peek_ident_text("resume")
            && self.peek_n(1).kind == TokenKind::Symbol(Symbol::LBrace)
        {
            let resume_tok = self.bump(); // `resume`（ident）
            let resume_span = resume_tok.span;
            let block = self.parse_block()?;
            let body = ast::Expr {
                span: block.span,
                kind: ast::ExprKind::Block(block),
            };

            return Ok(ast::HandleArm {
                span: Span::new(op.span.start, body.span.end),
                op,
                arrow_span: arrow.span,
                kind: ast::HandleArmKind::ImmediateResume { resume_span },
                body,
            });
        }

        let body = self.parse_control_body_expr("表达式（handler arm body）")?;

        Ok(ast::HandleArm {
            span: Span::new(op.span.start, body.span.end),
            op,
            arrow_span: arrow.span,
            kind: ast::HandleArmKind::NonResuming,
            body,
        })
    }

    fn parse_handle_op(&mut self) -> Result<ast::HandleOp, ParseError> {
        // effect operation name：`Raise.raise` / `foo.bar.Async.await`（op 为最后一个 segment）
        let first = self.expect_kind(TokenKind::Ident, "effect operation 名（标识符）")?;
        let start = first.span.start;

        let mut segments = vec![ast::Ident::new(first.span)];
        let mut dot_spans = Vec::new();

        while self.peek_symbol(Symbol::Dot) && self.peek_n(1).kind == TokenKind::Ident {
            let dot = self.bump();
            let seg = self.bump();
            dot_spans.push(dot.span);
            segments.push(ast::Ident::new(seg.span));
        }

        // 至少要有一个 `.`：`Effect.op(...)`
        if segments.len() < 2 {
            let tok = *self.peek();
            return Err(ParseError::Expected {
                expected: "effect operation（例如 `Raise.raise(...)`）",
                found: tok.kind,
                span: tok.span.into(),
            });
        }

        let Some(dot_span) = dot_spans.last().copied() else {
            unreachable!("segments.len()>=2 时应至少消费过一个 dot");
        };

        let op = segments
            .pop()
            .expect("segments.len()>=2 已保证存在 op segment");

        let effect_start = segments.first().map(|x| x.span.start).unwrap_or(start);
        let effect_end = segments.last().map(|x| x.span.end).unwrap_or(op.span.end);

        let effect = ast::TypePath {
            span: Span::new(effect_start, effect_end),
            segments,
            args: Vec::new(),
        };

        let open = self.expect_symbol(Symbol::LParen)?;
        let mut binders = Vec::new();

        if !self.peek_symbol(Symbol::RParen) {
            loop {
                let name_tok = self.expect_kind(TokenKind::Ident, "参数名（标识符）")?;
                let name = ast::Ident::new(name_tok.span);

                let (colon_span, ty, end) = if self.peek_symbol(Symbol::Colon) {
                    let colon = self.bump();
                    let ty = self.parse_type_ref()?;
                    let end = ty.span().end;
                    (Some(colon.span), Some(ty), end)
                } else {
                    (None, None, name_tok.span.end)
                };

                binders.push(ast::HandleBinder {
                    span: Span::new(name_tok.span.start, end),
                    name,
                    colon_span,
                    ty,
                });

                if self.eat_symbol(Symbol::Comma) {
                    // allow trailing comma
                    if self.peek_symbol(Symbol::RParen) {
                        break;
                    }
                    continue;
                }

                break;
            }
        }

        if self.peek_kind(TokenKind::Eof) {
            return Err(ParseError::UnterminatedGroup {
                close: Symbol::RParen,
                span: Span::new(open.span.start, self.peek().span.end).into(),
            });
        }
        let close = self.expect_symbol(Symbol::RParen)?;

        Ok(ast::HandleOp {
            span: Span::new(start, close.span.end),
            effect,
            dot_span,
            op,
            binders,
        })
    }

    fn recover_to_handle_arm_sync(&mut self) {
        // arm 同步点：
        // - handler list 的 `}`（不要吞掉，留给外层闭合）
        // - 下一个 `Effect.op(...) ->` 的起始（不要吞掉，留给外层继续解析）
        // - `;`（可选分隔符；外层会跳过）
        //
        // 说明：handler arms 允许省略分号，因此这里不能只同步到 `;`。
        if self.peek_kind(TokenKind::Eof) || self.peek_symbol(Symbol::RBrace) {
            return;
        }
        if self.peek_symbol(Symbol::Semicolon) {
            return;
        }
        if self.looks_like_handle_arm_start_at(self.i) {
            return;
        }

        let mut depth_paren = 0usize;
        let mut depth_brace = 0usize;
        let mut depth_bracket = 0usize;

        while !self.peek_kind(TokenKind::Eof) {
            if depth_paren == 0 && depth_brace == 0 && depth_bracket == 0 {
                if self.peek_symbol(Symbol::RBrace)
                    || self.peek_symbol(Symbol::Semicolon)
                    || self.looks_like_handle_arm_start_at(self.i)
                {
                    break;
                }
            }

            let tok = self.bump();
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
    }

    fn looks_like_handle_arm_start_at(&self, idx: usize) -> bool {
        let Some(first) = self.tokens.get(idx) else {
            return false;
        };
        if first.kind != TokenKind::Ident {
            return false;
        }

        // arm head 至少应满足 `Ident . Ident` 的前缀形态；这也避免把 `, k ->` 中的 `k` 误判为 arm 起始。
        let Some(dot) = self.tokens.get(idx + 1) else {
            return false;
        };
        let Some(second) = self.tokens.get(idx + 2) else {
            return false;
        };
        if dot.kind != TokenKind::Symbol(Symbol::Dot) || second.kind != TokenKind::Ident {
            return false;
        }

        let mut depth_paren = 0usize;
        let mut depth_brace = 0usize;
        let mut depth_bracket = 0usize;
        let mut saw_lparen = false;

        let mut j = idx;
        while let Some(tok) = self.tokens.get(j) {
            match tok.kind {
                TokenKind::Eof => return false,
                TokenKind::Symbol(Symbol::Arrow)
                    if depth_paren == 0 && depth_brace == 0 && depth_bracket == 0 =>
                {
                    return saw_lparen;
                }
                TokenKind::Symbol(Symbol::Semicolon)
                    if depth_paren == 0 && depth_brace == 0 && depth_bracket == 0 =>
                {
                    return false;
                }
                TokenKind::Symbol(Symbol::RBrace)
                    if depth_paren == 0 && depth_brace == 0 && depth_bracket == 0 =>
                {
                    // handler list 结束前都没遇到 `->`：不是 arm 起始。
                    return false;
                }
                TokenKind::Symbol(sym) => match sym {
                    Symbol::LParen => {
                        saw_lparen = true;
                        depth_paren += 1;
                    }
                    Symbol::RParen => depth_paren = depth_paren.saturating_sub(1),
                    Symbol::LBracket => depth_bracket += 1,
                    Symbol::RBracket => depth_bracket = depth_bracket.saturating_sub(1),
                    Symbol::LBrace => depth_brace += 1,
                    Symbol::RBrace => depth_brace = depth_brace.saturating_sub(1),
                    _ => {}
                },
                _ => {}
            }

            j = j.saturating_add(1);
        }

        false
    }

    fn parse_trailing_lambda_call_expr(
        &mut self,
        callee: ast::Expr,
    ) -> Result<ast::Expr, ParseError> {
        let start = callee.span.start;
        let lambda = self.parse_lambda_expr()?;
        Ok(ast::Expr {
            span: Span::new(start, lambda.span.end),
            kind: ast::ExprKind::Call {
                callee: Box::new(callee),
                args: vec![lambda],
            },
        })
    }

    fn parse_trailing_lambda_append_call_expr(
        &mut self,
        call_expr: ast::Expr,
    ) -> Result<ast::Expr, ParseError> {
        let start = call_expr.span.start;
        let lambda = self.parse_lambda_expr()?;

        let ast::ExprKind::Call { callee, mut args } = call_expr.kind else {
            unreachable!("parse_trailing_lambda_append_call_expr 仅用于 Call 表达式");
        };

        let end = lambda.span.end;
        args.push(lambda);
        Ok(ast::Expr {
            span: Span::new(start, end),
            kind: ast::ExprKind::Call { callee, args },
        })
    }

    /// 解析 struct literal：`TypeName { field: expr, ... }`（spec §12）。
    ///
    /// 当前阶段约束（与 TODO T0224 保持一致）：
    /// - 仅支持单段 `TypeName`（不解析 `a.b.Type`），避免与 “member access + trailing lambda” 的 `{}` 形态冲突；
    /// - 字段初始化只支持 `name: expr`（不支持省略写法）。
    fn parse_struct_lit_expr(&mut self) -> Result<ast::Expr, ParseError> {
        let ty_tok = self.expect_kind(TokenKind::Ident, "类型名（标识符）")?;
        let start = ty_tok.span.start;

        let ty_ident = ast::Ident::new(ty_tok.span);
        let ty = ast::TypePath {
            span: ty_tok.span,
            segments: vec![ty_ident],
            args: Vec::new(),
        };

        let _open = self.expect_symbol(Symbol::LBrace)?;

        let mut fields = Vec::new();

        if !self.peek_symbol(Symbol::RBrace) {
            loop {
                let name_tok = self.expect_kind(TokenKind::Ident, "字段名（标识符）")?;
                let name = ast::Ident::new(name_tok.span);

                let colon = self.expect_symbol(Symbol::Colon)?;

                let tok = *self.peek();
                let value = self.try_parse_expr()?.ok_or(ParseError::Expected {
                    expected: "表达式（字段初始化值）",
                    found: tok.kind,
                    span: tok.span.into(),
                })?;

                fields.push(ast::StructLitField {
                    span: Span::new(name_tok.span.start, value.span.end),
                    name,
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
        }

        let close = self.expect_symbol(Symbol::RBrace)?;

        Ok(ast::Expr {
            span: Span::new(start, close.span.end),
            kind: ast::ExprKind::StructLit { ty, fields },
        })
    }

    fn parse_interpolated_string_expr(
        &mut self,
        tok: Token,
        raw: bool,
    ) -> Result<ast::Expr, ParseError> {
        // lexer 会把整个 `f"..."` / `f"""..."""` 当作一个 token；
        // 这里在 parser 层把其拆分为：Text/Expr 片段列表（spec §8.2）。
        let (content_start, content_end) = if raw {
            // f""" ... """
            (tok.span.start + 4, tok.span.end.saturating_sub(3))
        } else {
            // f" ... "
            (tok.span.start + 2, tok.span.end.saturating_sub(1))
        };

        // 内部健壮性：span 理论上来自 lexer，不应越界；这里避免 panic 影响 fuzz/fixtures。
        if content_start > content_end || content_end > self.source_text.len() {
            return Ok(ast::Expr::missing(tok.span));
        }

        let parts = self.split_interpolated_string_parts(content_start, content_end, raw)?;
        Ok(ast::Expr {
            span: tok.span,
            kind: ast::ExprKind::InterpolatedString { raw, parts },
        })
    }

    fn split_interpolated_string_parts(
        &mut self,
        content_start: usize,
        content_end: usize,
        raw: bool,
    ) -> Result<Vec<ast::InterpolatedStringPart>, ParseError> {
        let bytes = self.source_text.as_bytes();
        let mut parts = Vec::new();

        let mut i = content_start;
        let mut text_start = content_start;

        while i < content_end {
            let b = bytes[i];

            // 普通字符串里支持 `\` 转义；转义序列中的 `{`/`}` 不应触发插值分片。
            if !raw && b == b'\\' {
                i += 1;
                if i < content_end {
                    let ch = self.source_text[i..].chars().next().unwrap();
                    i += ch.len_utf8();
                }
                continue;
            }

            // `{{` / `}}`：字面量大括号（spec §8.2）。
            if b == b'{' {
                if i + 1 < content_end && bytes[i + 1] == b'{' {
                    i += 2;
                    continue;
                }

                // 单个 `{`：插值表达式起始。
                if text_start < i {
                    parts.push(ast::InterpolatedStringPart::Text {
                        span: Span::new(text_start, i),
                    });
                }

                let open_brace = i;
                let expr_start = i + 1;
                let Some(expr_close) =
                    self.find_interpolation_close_in_f_string(expr_start, content_end)
                else {
                    return Err(ParseError::UnterminatedGroup {
                        close: Symbol::RBrace,
                        span: Span::new(open_brace, content_end).into(),
                    });
                };

                let expr = self.parse_expr_snippet(expr_start, expr_close)?;
                parts.push(ast::InterpolatedStringPart::Expr { expr });

                // 跳过 `}`，继续扫描后续文本。
                i = expr_close + 1;
                text_start = i;
                continue;
            }

            if b == b'}' {
                if i + 1 < content_end && bytes[i + 1] == b'}' {
                    i += 2;
                    continue;
                }

                // 单个 `}` 出现在插值字符串文本中时是语法错误（应写成 `}}`）。
                return Err(ParseError::FStringUnescapedRBrace {
                    span: Span::new(i, i + 1).into(),
                });
            }

            // 其它字符：按 UTF-8 前进，保持 index 在 char boundary 上，避免后续 slice panic。
            if b < 0x80 {
                i += 1;
            } else {
                let ch = self.source_text[i..].chars().next().unwrap();
                i += ch.len_utf8();
            }
        }

        if text_start < content_end {
            parts.push(ast::InterpolatedStringPart::Text {
                span: Span::new(text_start, content_end),
            });
        }

        Ok(parts)
    }

    /// 在 f-string 的内容区间内，从 `expr_start` 扫描并找到与插值起始 `{` 匹配的 `}`。
    ///
    /// 需要忽略表达式内部的字符串/注释，并对表达式内部的 `{}` 进行括号平衡；
    /// 这样才能正确处理例如：`f"{ if (a) { b } else { c } }"` 这种嵌套 `{}` 的情况。
    fn find_interpolation_close_in_f_string(
        &self,
        expr_start: usize,
        limit: usize,
    ) -> Option<usize> {
        let bytes = self.source_text.as_bytes();
        let mut i = expr_start;
        let mut brace_depth = 0usize;

        while i < limit {
            // line comment
            if i + 1 < limit && bytes[i] == b'/' && bytes[i + 1] == b'/' {
                i += 2;
                while i < limit && bytes[i] != b'\n' {
                    i += 1;
                }
                continue;
            }

            // block comment (non-nested)
            if i + 1 < limit && bytes[i] == b'/' && bytes[i + 1] == b'*' {
                i += 2;
                while i + 1 < limit {
                    if bytes[i] == b'*' && bytes[i + 1] == b'/' {
                        i += 2;
                        break;
                    }
                    i += 1;
                }
                continue;
            }

            // string literal: "..." / """...""", optionally prefixed with `f`
            if bytes[i] == b'f' && i + 1 < limit && bytes[i + 1] == b'"' {
                i = self.skip_string_literal(i + 1, limit);
                continue;
            }
            if bytes[i] == b'"' {
                i = self.skip_string_literal(i, limit);
                continue;
            }

            match bytes[i] {
                b'{' => {
                    brace_depth += 1;
                    i += 1;
                }
                b'}' => {
                    if brace_depth == 0 {
                        return Some(i);
                    }
                    brace_depth = brace_depth.saturating_sub(1);
                    i += 1;
                }
                _ => {
                    i += 1;
                }
            }
        }

        None
    }

    /// 从 `"` 开始跳过一个字符串字面量，返回“紧随其后”的索引。
    ///
    /// 该函数是“面向括号平衡扫描”的最小实现：不解析转义语义，只负责正确跳过字符串范围。
    fn skip_string_literal(&self, quote_start: usize, limit: usize) -> usize {
        let bytes = self.source_text.as_bytes();

        // raw string: """ ... """
        if quote_start + 2 < limit
            && bytes[quote_start] == b'"'
            && bytes[quote_start + 1] == b'"'
            && bytes[quote_start + 2] == b'"'
        {
            let mut i = quote_start + 3;
            while i + 2 < limit {
                if bytes[i] == b'"' && bytes[i + 1] == b'"' && bytes[i + 2] == b'"' {
                    return i + 3;
                }
                i += 1;
            }
            return limit;
        }

        // normal string: " ... " (with backslash escapes)
        let mut i = quote_start + 1;
        while i < limit {
            match bytes[i] {
                b'\\' => {
                    // consume '\' + next byte if present
                    i = (i + 2).min(limit);
                }
                b'"' => {
                    return i + 1;
                }
                b'\n' => {
                    return limit;
                }
                _ => {
                    i += 1;
                }
            }
        }
        limit
    }

    fn parse_expr_snippet(&self, start: usize, end: usize) -> Result<ast::Expr, ParseError> {
        let snippet = &self.source_text[start..end];

        let mut tokens = lex(snippet).map_err(|e| ParseError::Lex(shift_lex_error(e, start)))?;
        for t in &mut tokens {
            t.span = Span::new(t.span.start + start, t.span.end + start);
        }

        let mut p = Parser::new(self.source_text, tokens);
        let tok = *p.peek();
        let expr = p.try_parse_expr()?.ok_or(ParseError::Expected {
            expected: "表达式（f-string 插值）",
            found: tok.kind,
            span: tok.span.into(),
        })?;

        if !p.peek_kind(TokenKind::Eof) {
            let tok = *p.peek();
            return Err(ParseError::Expected {
                expected: "插值表达式结束（`}`）",
                found: tok.kind,
                span: tok.span.into(),
            });
        }

        // 该 snippet parser 可能会复用 `parse_block()`（例如 `if (...) { ... }` / `when` arm 的 block body），
        // 而 block 内的错误恢复会把诊断记录到 `p.errors` 中。
        // snippet 解析是“独立入口”，因此需要在此处把这些诊断重新提升为返回值。
        if !p.errors.is_empty() {
            let count = p.errors.len();
            let span = p
                .errors
                .iter()
                .find_map(ParseError::primary_span)
                .unwrap_or_else(|| (0usize, 0usize).into());
            let mut errors = std::mem::take(&mut p.errors);
            return Err(if count == 1 {
                errors.pop().expect("count==1 已保证 errors 非空")
            } else {
                ParseError::Many {
                    count,
                    span,
                    errors,
                }
            });
        }

        Ok(expr)
    }

    /// 解析 lambda 表达式：`{ params -> body }` / `{ body }`（spec §12 / Appendix B.5）。
    ///
    /// 说明：
    /// - 在“通用表达式起始”位置遇到 `{` 时优先解析为 lambda（Kotlin 风格）。
    /// - `if` / `when` 等控制结构的 `{ ... }` block body 由各自解析函数优先处理，
    ///   以避免把 `if (cond) { ... }` 解析成 lambda。
    fn parse_lambda_expr(&mut self) -> Result<ast::Expr, ParseError> {
        let open = self.expect_symbol(Symbol::LBrace)?;
        let start = open.span.start;

        let mut params = Vec::new();
        let mut arrow_span = None;

        if self.peek_symbol(Symbol::Arrow) {
            // 0 参数显式箭头：`{ -> body }`
            let arrow = self.bump();
            arrow_span = Some(arrow.span);
        } else if let Some((ps, arrow)) = self.try_parse_lambda_params_and_arrow()? {
            params = ps;
            arrow_span = Some(arrow);
        }

        let block = self.parse_block_with_open(open)?;
        let end = block.span.end;

        // 当前阶段（T0222）只区分两类 body：
        // - 单表达式 body：直接用该表达式
        // - block body：用 `ExprKind::Block`（包含语句列表）
        let body = match block.stmts.as_slice() {
            [
                ast::Stmt {
                    kind: ast::StmtKind::Expr(expr),
                    ..
                },
            ] => expr.clone(),
            _ => ast::Expr {
                span: block.span,
                kind: ast::ExprKind::Block(block),
            },
        };

        Ok(ast::Expr {
            span: Span::new(start, end),
            kind: ast::ExprKind::Lambda(ast::LambdaExpr {
                params,
                arrow_span,
                body: Box::new(body),
            }),
        })
    }

    /// 尝试解析 lambda 的参数列表并消费 `->`。
    ///
    /// 成功时返回 `(params, arrow_span)`；
    /// 若当前位置不像 lambda 参数起始（或缺少 `->`），返回 `Ok(None)` 且不消费任何 token。
    fn try_parse_lambda_params_and_arrow(
        &mut self,
    ) -> Result<Option<(Vec<ast::Param>, Span)>, ParseError> {
        if !self.peek_kind(TokenKind::Ident) {
            return Ok(None);
        }

        let checkpoint = self.i;
        let mut params = Vec::new();

        loop {
            if !self.peek_kind(TokenKind::Ident) {
                self.i = checkpoint;
                return Ok(None);
            }

            let name_tok = self.bump();
            let name = ast::Ident::new(name_tok.span);

            let ty = if self.eat_symbol(Symbol::Colon) {
                Some(self.parse_type_ref()?)
            } else {
                None
            };
            params.push(ast::Param {
                annotations: Vec::new(),
                kind: None,
                is_vararg: false,
                name,
                ty,
                default_value: None,
            });

            if self.peek_symbol(Symbol::Arrow) {
                let arrow = self.bump();
                return Ok(Some((params, arrow.span)));
            }

            if self.eat_symbol(Symbol::Comma) {
                // 与其它列表一致：允许一个宽容的 trailing comma（例如 `{ a, b, -> a }`）。
                if self.peek_symbol(Symbol::Arrow) {
                    let arrow = self.bump();
                    return Ok(Some((params, arrow.span)));
                }
                continue;
            }

            // 缺少 `->`：不是 lambda params 形态，回退到 `{ body }`。
            self.i = checkpoint;
            return Ok(None);
        }
    }

    fn parse_control_body_expr(&mut self, expected: &'static str) -> Result<ast::Expr, ParseError> {
        if self.peek_symbol(Symbol::LBrace) {
            let block = self.parse_block()?;
            return Ok(ast::Expr {
                span: block.span,
                kind: ast::ExprKind::Block(block),
            });
        }

        let tok = *self.peek();
        self.try_parse_expr()?.ok_or(ParseError::Expected {
            expected,
            found: tok.kind,
            span: tok.span.into(),
        })
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

        let then_branch = self.parse_control_body_expr("表达式（then 分支）")?;

        let (end, else_branch) = if self.peek_keyword(Keyword::Else) {
            self.bump();
            let else_expr = self.parse_control_body_expr("表达式（else 分支）")?;
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

            let guard = if self.peek_keyword(Keyword::If) {
                self.bump();
                let tok = *self.peek();
                Some(self.try_parse_expr()?.ok_or(ParseError::Expected {
                    expected: "表达式（when 分支 guard）",
                    found: tok.kind,
                    span: tok.span.into(),
                })?)
            } else {
                None
            };

            let arrow = self.expect_symbol(Symbol::Arrow)?;

            let body = if self.peek_symbol(Symbol::LBrace) {
                let block = self.parse_block()?;
                ast::Expr {
                    span: block.span,
                    kind: ast::ExprKind::Block(block),
                }
            } else {
                let tok = *self.peek();
                self.try_parse_expr_in_when_arm()?
                    .ok_or(ParseError::Expected {
                        expected: "表达式（when 分支 body）",
                        found: tok.kind,
                        span: tok.span.into(),
                    })?
            };

            arms.push(ast::WhenArm {
                span: Span::new(pat_span.start, body.span.end),
                pat,
                guard,
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
        self.parse_when_pat_internal(true)
    }

    fn parse_when_pat_internal(&mut self, allow_else: bool) -> Result<ast::WhenPat, ParseError> {
        let first = self.parse_when_pat_atom_internal(allow_else)?;

        // or-pattern：`A | B | C`
        //
        // 说明：
        // - 该语法仅用于 `when` pattern 位置；
        // - 当前阶段不在 parser 处强制限制 `else` 出现在 or-pattern 内（语义约束交给后续阶段）。
        let mut pats = vec![first];
        while self.eat_symbol(Symbol::Or) {
            pats.push(self.parse_when_pat_atom_internal(false)?);
        }

        if pats.len() == 1 {
            return Ok(pats.pop().unwrap());
        }

        let start = pats.first().unwrap().span().start;
        let end = pats.last().unwrap().span().end;
        Ok(ast::WhenPat::Or {
            span: Span::new(start, end),
            pats,
        })
    }

    fn parse_when_pat_atom_internal(
        &mut self,
        allow_else: bool,
    ) -> Result<ast::WhenPat, ParseError> {
        if allow_else && self.peek_keyword(Keyword::Else) {
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

        if self.peek_symbol(Symbol::LParen) {
            return self.parse_when_tuple_pat();
        }

        if self.peek_kind(TokenKind::IntLiteral) {
            let tok = self.bump();
            return Ok(ast::WhenPat::IntLit { span: tok.span });
        }

        if matches!(self.peek().kind, TokenKind::StringLiteral(_)) {
            let tok = self.bump();
            return Ok(ast::WhenPat::StringLit { span: tok.span });
        }

        if self.peek_kind(TokenKind::Ident) {
            let tok = self.bump();
            let ident = ast::Ident::new(tok.span);
            let name = self
                .source_text
                .get(ident.span.start..ident.span.end)
                .unwrap_or("");

            if name == "_" {
                return Ok(ast::WhenPat::Wildcard { span: tok.span });
            }
            if name == "true" || name == "false" {
                return Ok(ast::WhenPat::BoolLit { span: tok.span });
            }

            // `Name(...)`：variant pattern（位置参数）。
            if self.peek_symbol(Symbol::LParen) {
                let open = self.expect_symbol(Symbol::LParen)?;
                let start = tok.span.start;

                let mut args = Vec::new();
                let mut rest_span: Option<Span> = None;
                if self.peek_symbol(Symbol::RParen) {
                    let close = self.bump();
                    return Ok(ast::WhenPat::Variant {
                        span: Span::new(start, close.span.end),
                        name: ident,
                        args,
                    });
                }

                loop {
                    // `..` rest：仅允许出现一次，并且必须是最后一个参数。
                    if rest_span.is_some() {
                        let tok = *self.peek();
                        if self.peek_symbol(Symbol::DotDot)
                            || (self.peek_symbol(Symbol::Dot)
                                && self.peek_n(1).kind == TokenKind::Symbol(Symbol::Dot))
                        {
                            let err = ParseError::Expected {
                                expected: "when variant pattern：`..` 只能出现一次",
                                found: tok.kind,
                                span: tok.span.into(),
                            };
                            let _ = self.consume_balanced_after_open(
                                Symbol::LParen,
                                Symbol::RParen,
                                open.span.start,
                            );
                            return Err(err);
                        }
                        let err = ParseError::Expected {
                            expected: "`)`（`..` 必须是最后一个参数）",
                            found: tok.kind,
                            span: tok.span.into(),
                        };
                        let _ = self.consume_balanced_after_open(
                            Symbol::LParen,
                            Symbol::RParen,
                            open.span.start,
                        );
                        return Err(err);
                    }

                    if self.peek_symbol(Symbol::DotDot)
                        || (self.peek_symbol(Symbol::Dot)
                            && self.peek_n(1).kind == TokenKind::Symbol(Symbol::Dot))
                    {
                        let span = if self.peek_symbol(Symbol::DotDot) {
                            self.bump().span
                        } else {
                            let dot1 = self.bump();
                            let dot2 = self.bump();
                            Span::new(dot1.span.start, dot2.span.end)
                        };
                        rest_span = Some(span);
                        args.push(ast::WhenPat::Rest { span });
                    } else {
                        args.push(self.parse_when_pat_internal(false)?);
                    }
                    if self.eat_symbol(Symbol::Comma) {
                        // allow trailing comma
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
                return Ok(ast::WhenPat::Variant {
                    span: Span::new(start, close.span.end),
                    name: ident,
                    args,
                });
            }

            // `Name`（无括号）：当前阶段用一个启发式消歧：
            // - 首字符为大写：视为 0-arg enum variant；
            // - 否则视为 bind（并在该 arm body 内引入局部绑定）。
            if name
                .chars()
                .next()
                .map(|c| c.is_uppercase())
                .unwrap_or(false)
            {
                return Ok(ast::WhenPat::Variant {
                    span: tok.span,
                    name: ident,
                    args: Vec::new(),
                });
            }

            return Ok(ast::WhenPat::Bind { ident });
        }

        let tok = *self.peek();
        Err(ParseError::Expected {
            expected: "when 分支模式（`else` / `is T` / 字面量 / 绑定 / tuple / variant）",
            found: tok.kind,
            span: tok.span.into(),
        })
    }

    fn parse_when_tuple_pat(&mut self) -> Result<ast::WhenPat, ParseError> {
        let open = self.expect_symbol(Symbol::LParen)?;
        let start = open.span.start;

        let mut elements = Vec::new();
        let mut rest_span: Option<Span> = None;
        if self.peek_symbol(Symbol::RParen) {
            let close = self.bump();
            return Ok(ast::WhenPat::Tuple {
                span: Span::new(start, close.span.end),
                elements,
            });
        }

        loop {
            // `..` rest：仅允许出现一次，并且必须是最后一个元素。
            if rest_span.is_some() {
                let tok = *self.peek();
                if self.peek_symbol(Symbol::DotDot)
                    || (self.peek_symbol(Symbol::Dot)
                        && self.peek_n(1).kind == TokenKind::Symbol(Symbol::Dot))
                {
                    let err = ParseError::Expected {
                        expected: "when tuple pattern：`..` 只能出现一次",
                        found: tok.kind,
                        span: tok.span.into(),
                    };
                    let _ = self.consume_balanced_after_open(
                        Symbol::LParen,
                        Symbol::RParen,
                        open.span.start,
                    );
                    return Err(err);
                }
                let err = ParseError::Expected {
                    expected: "`)`（`..` 必须是最后一个元素）",
                    found: tok.kind,
                    span: tok.span.into(),
                };
                let _ = self.consume_balanced_after_open(
                    Symbol::LParen,
                    Symbol::RParen,
                    open.span.start,
                );
                return Err(err);
            }

            if self.peek_symbol(Symbol::DotDot)
                || (self.peek_symbol(Symbol::Dot)
                    && self.peek_n(1).kind == TokenKind::Symbol(Symbol::Dot))
            {
                let span = if self.peek_symbol(Symbol::DotDot) {
                    self.bump().span
                } else {
                    let dot1 = self.bump();
                    let dot2 = self.bump();
                    Span::new(dot1.span.start, dot2.span.end)
                };
                rest_span = Some(span);
                elements.push(ast::WhenPat::Rest { span });
            } else {
                elements.push(self.parse_when_pat_internal(false)?);
            }
            if self.eat_symbol(Symbol::Comma) {
                // allow trailing comma
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

        Ok(ast::WhenPat::Tuple {
            span: Span::new(start, close.span.end),
            elements,
        })
    }

    /// 解析一次调用参数列表：`(arg1, arg2, ...)`。
    ///
    /// 说明：
    /// - 该函数只负责解析括号内的参数列表并返回其 span 与参数表达式列表；
    /// - 具体把它包装成 `ExprKind::Call`（或用于 ctor delegation/super ctor args）
    ///   由调用者决定。
    pub(super) fn parse_call_arg_list(&mut self) -> Result<(Span, Vec<ast::Expr>), ParseError> {
        let open = self.expect_symbol(Symbol::LParen)?;
        let start = open.span.start;

        let mut args = Vec::new();
        if self.peek_symbol(Symbol::RParen) {
            let close = self.bump();
            return Ok((Span::new(start, close.span.end), args));
        }

        loop {
            // 命名参数实参（Appendix B.5.3）：仅在调用参数列表中把 `name = expr`
            // 解析为 `ExprKind::NamedArg`，避免与赋值表达式混淆。
            let arg = if self.peek_kind(TokenKind::Ident)
                && self.peek_n(1).kind == TokenKind::Symbol(Symbol::Eq)
            {
                let name_tok = self.bump();
                let eq = self.expect_symbol(Symbol::Eq)?;

                // Appendix B.5.5：spread（Kotlin-like）：`name = *arr`
                let value = if self.peek_symbol(Symbol::Star) {
                    let star = self.bump();
                    let tok = *self.peek();
                    let inner = self.try_parse_expr()?.ok_or(ParseError::Expected {
                        expected: "表达式（spread 实参）",
                        found: tok.kind,
                        span: tok.span.into(),
                    })?;
                    ast::Expr {
                        span: Span::new(star.span.start, inner.span.end),
                        kind: ast::ExprKind::SpreadArg {
                            star_span: star.span,
                            expr: Box::new(inner),
                        },
                    }
                } else {
                    let tok = *self.peek();
                    self.try_parse_expr()?.ok_or(ParseError::Expected {
                        expected: "表达式（命名参数值）",
                        found: tok.kind,
                        span: tok.span.into(),
                    })?
                };

                ast::Expr {
                    span: Span::new(name_tok.span.start, value.span.end),
                    kind: ast::ExprKind::NamedArg {
                        name: ast::Ident::new(name_tok.span),
                        eq_span: eq.span,
                        value: Box::new(value),
                    },
                }
            } else {
                // Appendix B.5.5：spread（Kotlin-like）：`*arr`
                if self.peek_symbol(Symbol::Star) {
                    let star = self.bump();
                    let tok = *self.peek();
                    let inner = self.try_parse_expr()?.ok_or(ParseError::Expected {
                        expected: "表达式（spread 实参）",
                        found: tok.kind,
                        span: tok.span.into(),
                    })?;
                    ast::Expr {
                        span: Span::new(star.span.start, inner.span.end),
                        kind: ast::ExprKind::SpreadArg {
                            star_span: star.span,
                            expr: Box::new(inner),
                        },
                    }
                } else {
                    let tok = *self.peek();
                    self.try_parse_expr()?.ok_or(ParseError::Expected {
                        expected: "表达式（参数）",
                        found: tok.kind,
                        span: tok.span.into(),
                    })?
                }
            };
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
        Ok((Span::new(start, close.span.end), args))
    }

    fn parse_call_expr(&mut self, callee: ast::Expr) -> Result<ast::Expr, ParseError> {
        let start = callee.span.start;
        let (args_span, args) = self.parse_call_arg_list()?;
        Ok(ast::Expr {
            span: Span::new(start, args_span.end),
            kind: ast::ExprKind::Call {
                callee: Box::new(callee),
                args,
            },
        })
    }

    fn parse_member_access_expr(&mut self, receiver: ast::Expr) -> Result<ast::Expr, ParseError> {
        let _dot = self.expect_symbol(Symbol::Dot)?;

        // Splice：`receiver.[field]`（spec §6.4）。
        //
        // 说明：该语法只在 `.` 后紧跟 `[` 时成立，与普通成员访问 `receiver.member` 区分。
        if self.peek_symbol(Symbol::LBracket) {
            let open = self.bump();
            debug_assert_eq!(open.kind, TokenKind::Symbol(Symbol::LBracket));

            let tok = *self.peek();
            let field = self.try_parse_expr()?.ok_or(ParseError::Expected {
                expected: "表达式（splice 字段）",
                found: tok.kind,
                span: tok.span.into(),
            })?;

            if self.peek_kind(TokenKind::Eof) {
                return Err(ParseError::UnterminatedGroup {
                    close: Symbol::RBracket,
                    span: Span::new(open.span.start, self.peek().span.end).into(),
                });
            }
            let close = self.expect_symbol(Symbol::RBracket)?;

            return Ok(ast::Expr {
                span: Span::new(receiver.span.start, close.span.end),
                kind: ast::ExprKind::SpliceField {
                    receiver: Box::new(receiver),
                    field: Box::new(field),
                },
            });
        }

        let member_tok = self.expect_kind(TokenKind::Ident, "成员名（标识符）")?;

        Ok(ast::Expr {
            span: Span::new(receiver.span.start, member_tok.span.end),
            kind: ast::ExprKind::MemberAccess {
                receiver: Box::new(receiver),
                member: ast::MemberIdent::new(member_tok.span),
            },
        })
    }

    fn parse_safe_member_access_expr(
        &mut self,
        receiver: ast::Expr,
    ) -> Result<ast::Expr, ParseError> {
        let op = self.expect_symbol(Symbol::QuestionDot)?;
        let member_tok = self.expect_kind(TokenKind::Ident, "成员名（标识符）")?;

        Ok(ast::Expr {
            span: Span::new(receiver.span.start, member_tok.span.end),
            kind: ast::ExprKind::SafeMemberAccess {
                receiver: Box::new(receiver),
                op_span: op.span,
                member: ast::MemberIdent::new(member_tok.span),
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
                resolved_struct_fqns: OnceCell::new(),
            },
        })
    }

    fn parse_field_path(&mut self) -> Result<ast::FieldPath, ParseError> {
        let first = self.expect_kind(TokenKind::Ident, "字段路径（标识符）")?;
        let start = first.span.start;

        let mut segments = vec![ast::Ident::new(first.span)];
        while self.eat_symbol(Symbol::Dot) {
            let seg = self.expect_kind(TokenKind::Ident, "字段路径（标识符）")?;
            segments.push(ast::Ident::new(seg.span));
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

        // 先允许空 `()`：Unit 字面量。
        if self.peek_symbol(Symbol::RParen) {
            let close = self.bump();
            return Ok(Some(ast::Expr {
                span: Span::new(start, close.span.end),
                kind: ast::ExprKind::UnitLit,
            }));
        }

        // 解析第一个元素：`(expr)` / `(expr, ...)`。
        //
        // 若括号内不是表达式起始，则整体降级为 Missing（吞掉平衡括号，保持 cursor 正确）。
        let first = self.try_parse_expr()?;
        let Some(first) = first else {
            let span = self.consume_balanced_after_open(Symbol::LParen, Symbol::RParen, start)?;
            return Ok(Some(ast::Expr::missing(span)));
        };

        // tuple literal：`(a, b, ...)` / `(a,)`
        if self.eat_symbol(Symbol::Comma) {
            let mut elements = vec![first];

            while !self.peek_symbol(Symbol::RParen) && !self.peek_kind(TokenKind::Eof) {
                let tok = *self.peek();
                let expr = self.try_parse_expr()?.ok_or(ParseError::Expected {
                    expected: "表达式（tuple 元素）",
                    found: tok.kind,
                    span: tok.span.into(),
                })?;
                elements.push(expr);

                if self.eat_symbol(Symbol::Comma) {
                    // allow trailing comma
                    if self.peek_symbol(Symbol::RParen) {
                        break;
                    }
                    continue;
                }
                break;
            }

            let close = self.expect_symbol(Symbol::RParen)?;
            return Ok(Some(ast::Expr {
                span: Span::new(start, close.span.end),
                kind: ast::ExprKind::TupleLit { elements },
            }));
        }

        // grouping expr：`(expr)`
        if self.peek_symbol(Symbol::RParen) {
            let close = self.bump();
            let mut inner = first;
            inner.span = Span::new(start, close.span.end);
            return Ok(Some(inner));
        }

        // 括号内存在额外 token（例如 `(1; 2)`）：
        // 当前阶段不支持，吞掉整段并降级为 Missing。
        let span = self.consume_balanced_after_open(Symbol::LParen, Symbol::RParen, start)?;
        Ok(Some(ast::Expr::missing(span)))
    }

    fn parse_array_lit_expr(&mut self) -> Result<ast::Expr, ParseError> {
        let open = self.expect_symbol(Symbol::LBracket)?;
        let start = open.span.start;

        let mut elements = Vec::new();
        if self.peek_symbol(Symbol::RBracket) {
            let close = self.bump();
            return Ok(ast::Expr {
                span: Span::new(start, close.span.end),
                kind: ast::ExprKind::ArrayLit { elements },
            });
        }

        loop {
            let tok = *self.peek();
            let expr = self.try_parse_expr()?.ok_or(ParseError::Expected {
                expected: "表达式（数组元素）",
                found: tok.kind,
                span: tok.span.into(),
            })?;
            elements.push(expr);

            if self.eat_symbol(Symbol::Comma) {
                // allow trailing comma
                if self.peek_symbol(Symbol::RBracket) {
                    break;
                }
                continue;
            }
            break;
        }

        if self.peek_kind(TokenKind::Eof) {
            return Err(ParseError::UnterminatedGroup {
                close: Symbol::RBracket,
                span: Span::new(open.span.start, self.peek().span.end).into(),
            });
        }
        let close = self.expect_symbol(Symbol::RBracket)?;
        Ok(ast::Expr {
            span: Span::new(start, close.span.end),
            kind: ast::ExprKind::ArrayLit { elements },
        })
    }

    fn parse_class_lit_expr(&mut self, receiver: ast::Expr) -> Result<ast::Expr, ParseError> {
        let receiver_span = receiver.span;
        let start = receiver_span.start;

        let Some(path) = type_path_from_expr(receiver) else {
            return Err(ParseError::ClassLiteralReceiverInvalid {
                span: receiver_span.into(),
            });
        };

        self.bump(); // ':'
        self.bump(); // ':'
        let class_kw = self.expect_keyword(Keyword::Class)?;
        let end = class_kw.span.end;

        Ok(ast::Expr {
            span: Span::new(start, end),
            kind: ast::ExprKind::ClassLit {
                ty: ast::TypeRef::Path(path),
            },
        })
    }

    fn disambiguate_ident_lbrace_group(&self) -> BraceGroupKind {
        debug_assert_eq!(self.peek().kind, TokenKind::Ident);
        debug_assert_eq!(self.peek_n(1).kind, TokenKind::Symbol(Symbol::LBrace));

        // spec §12：通过 `{ ... }` 内容形态区分：
        // - Struct literal：包含 `name: expr`，且不含顶层 `->`
        // - Lambda：包含顶层 `->` 或普通表达式 body
        //
        // 另外，为了保留 “struct literal 字段缺少 `:`” 的精准诊断，
        // 在形态不明确时（例如 `Point { x 1 }`），倾向先按 struct literal 解析并在缺少 `:` 时报错。
        let first = self.peek_n(2);
        match first.kind {
            TokenKind::Symbol(Symbol::RBrace) | TokenKind::Symbol(Symbol::Arrow) => {
                return BraceGroupKind::Lambda;
            }
            TokenKind::Ident => {}
            _ => return BraceGroupKind::Lambda,
        }

        let second = self.peek_n(3);
        match second.kind {
            TokenKind::Symbol(Symbol::Arrow) | TokenKind::Symbol(Symbol::Comma) => {
                BraceGroupKind::Lambda
            }
            TokenKind::Symbol(Symbol::RBrace) => BraceGroupKind::Lambda,
            TokenKind::Symbol(Symbol::Colon) => {
                if self.brace_group_has_top_level_arrow(self.i + 1) {
                    BraceGroupKind::Lambda
                } else {
                    BraceGroupKind::StructLit
                }
            }
            // `it * 2` / `it.foo` / `it(...)` 等都属于“普通表达式 body”的 lambda。
            TokenKind::Symbol(_) | TokenKind::Keyword(_) => BraceGroupKind::Lambda,
            // `Point { x 1 }` / `Point { x y }` 等形态在没有分隔符时更像 struct literal 缺少 `:`。
            _ => BraceGroupKind::StructLit,
        }
    }

    fn brace_group_has_top_level_arrow(&self, open_brace_index: usize) -> bool {
        debug_assert_eq!(
            self.tokens
                .get(open_brace_index)
                .unwrap_or_else(|| self.tokens.last().expect("lexer must produce EOF"))
                .kind,
            TokenKind::Symbol(Symbol::LBrace)
        );

        let mut depth_paren = 0usize;
        let mut depth_brace = 0usize;
        let mut depth_bracket = 0usize;

        let mut idx = open_brace_index + 1;
        while idx < self.tokens.len() {
            let tok = self.tokens.get(idx).unwrap_or_else(|| {
                self.tokens
                    .last()
                    .expect("lexer must produce at least EOF token")
            });

            match tok.kind {
                TokenKind::Symbol(Symbol::Arrow)
                    if depth_paren == 0 && depth_brace == 0 && depth_bracket == 0 =>
                {
                    return true;
                }
                TokenKind::Symbol(Symbol::LParen) => depth_paren += 1,
                TokenKind::Symbol(Symbol::RParen) => depth_paren = depth_paren.saturating_sub(1),
                TokenKind::Symbol(Symbol::LBracket) => depth_bracket += 1,
                TokenKind::Symbol(Symbol::RBracket) => {
                    depth_bracket = depth_bracket.saturating_sub(1);
                }
                TokenKind::Symbol(Symbol::LBrace) => depth_brace += 1,
                TokenKind::Symbol(Symbol::RBrace) => {
                    if depth_paren == 0 && depth_brace == 0 && depth_bracket == 0 {
                        return false;
                    }
                    depth_brace = depth_brace.saturating_sub(1);
                }
                TokenKind::Eof => return false,
                _ => {}
            }

            idx += 1;
        }

        false
    }
}

fn is_assignable_lhs(expr: &ast::Expr) -> bool {
    matches!(
        expr.kind,
        ast::ExprKind::Ident(_) | ast::ExprKind::MemberAccess { .. }
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BraceGroupKind {
    StructLit,
    Lambda,
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

fn scan_tuple_or_group_type_end(
    tokens: &[crate::syntax::token::Token],
    start: usize,
) -> Option<usize> {
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
        while !matches!(
            kind_at(tokens, i),
            TokenKind::Symbol(Symbol::RParen) | TokenKind::Eof
        ) {
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

        // range：`a..b`（Appendix B.12）。
        // v0：先把它放在“与比较同级、低于移位/算术”的层级，后续再对齐 Kotlin 的完整优先级表。
        Symbol::DotDot => Some((8, 9, ast::BinaryOp::RangeInclusive)),

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

fn shift_lex_error(err: LexError, base_offset: usize) -> LexError {
    match err {
        LexError::InvalidChar { ch, span } => LexError::InvalidChar {
            ch,
            span: shift_source_span(span, base_offset),
        },
        LexError::UnterminatedBlockComment { span } => LexError::UnterminatedBlockComment {
            span: shift_source_span(span, base_offset),
        },
        LexError::UnterminatedString { span } => LexError::UnterminatedString {
            span: shift_source_span(span, base_offset),
        },
    }
}

fn type_path_from_expr(expr: ast::Expr) -> Option<ast::TypePath> {
    let mut segments: Vec<ast::Ident> = Vec::new();
    if !collect_type_path_segments_from_expr(&expr, &mut segments) {
        return None;
    }
    if segments.is_empty() {
        return None;
    }
    Some(ast::TypePath {
        span: expr.span,
        segments,
        args: Vec::new(),
    })
}

fn collect_type_path_segments_from_expr(expr: &ast::Expr, out: &mut Vec<ast::Ident>) -> bool {
    match &expr.kind {
        ast::ExprKind::Ident(id) => {
            out.push(ast::Ident::new(id.span));
            true
        }
        ast::ExprKind::MemberAccess { receiver, member } => {
            if !collect_type_path_segments_from_expr(receiver.as_ref(), out) {
                return false;
            }
            out.push(ast::Ident::new(member.span));
            true
        }
        _ => false,
    }
}

fn shift_source_span(span: miette::SourceSpan, base_offset: usize) -> miette::SourceSpan {
    let offset = span.offset();
    let len = span.len();
    (offset + base_offset, len).into()
}
