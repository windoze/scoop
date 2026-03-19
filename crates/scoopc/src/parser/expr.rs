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
//! - 二元运算符优先级（T0211）：`1 + 2 * 3`
//! - Elvis（T0212）：`a ?: b`
//! - 类型判断/转换（T0213）：`is`/`!is`/`as`/`as?`
//! - `if` 表达式（T0214）：`if (cond) thenExpr else elseExpr?`
//! - `when` 表达式（T0215）：`when (expr) { ... }`（最小分支子集）
//! - 赋值表达式（T0227）：`lhs = rhs`（lhs 先限 ident/member）
//!
//! 说明：
//! - 该模块的目标是支撑顶层 `val/var` initializer 的增量解析；
//! - 更复杂的表达式（调用/成员访问/二元运算/控制流等）会在后续任务中逐步补齐。

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
                //
                // 注意：当前仅处理“无括号参数列表”的形态（例如 `list.map { it }`）。
                // `callee(args) { ... }` 需要把 lambda 追加到现有 `Call.args`，放在后续任务（T0232）实现，
                // 避免把 `f(1) { ... }` 错误解析为“调用返回值再调用”。
                if matches!(expr.kind, ast::ExprKind::Call { .. }) {
                    break;
                }
                expr = self.parse_trailing_lambda_call_expr(expr)?;
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

        if self.peek_symbol(Symbol::LBrace) {
            return Ok(Some(self.parse_lambda_expr()?));
        }

        if self.peek_symbol(Symbol::LParen) {
            return self.try_parse_paren_group_expr();
        }

        Ok(None)
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

    /// 解析 struct literal：`TypeName { field: expr, ... }`（spec §12）。
    ///
    /// 当前阶段约束（与 TODO T0224 保持一致）：
    /// - 仅支持单段 `TypeName`（不解析 `a.b.Type`），避免与 “member access + trailing lambda” 的 `{}` 形态冲突；
    /// - 字段初始化只支持 `name: expr`（不支持省略写法）。
    fn parse_struct_lit_expr(&mut self) -> Result<ast::Expr, ParseError> {
        let ty_tok = self.expect_kind(TokenKind::Ident, "类型名（标识符）")?;
        let start = ty_tok.span.start;

        let ty_ident = ast::Ident { span: ty_tok.span };
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
                let name = ast::Ident {
                    span: name_tok.span,
                };

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
            let name = ast::Ident {
                span: name_tok.span,
            };

            let ty = if self.eat_symbol(Symbol::Colon) {
                Some(self.parse_type_ref()?)
            } else {
                None
            };
            params.push(ast::Param {
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
            // 命名参数实参（Appendix B.5.3）：仅在调用参数列表中把 `name = expr`
            // 解析为 `ExprKind::NamedArg`，避免与赋值表达式混淆。
            let arg = if self.peek_kind(TokenKind::Ident)
                && self.peek_n(1).kind == TokenKind::Symbol(Symbol::Eq)
            {
                let name_tok = self.bump();
                let eq = self.expect_symbol(Symbol::Eq)?;

                let tok = *self.peek();
                let value = self.try_parse_expr()?.ok_or(ParseError::Expected {
                    expected: "表达式（命名参数值）",
                    found: tok.kind,
                    span: tok.span.into(),
                })?;

                ast::Expr {
                    span: Span::new(name_tok.span.start, value.span.end),
                    kind: ast::ExprKind::NamedArg {
                        name: ast::Ident { span: name_tok.span },
                        eq_span: eq.span,
                        value: Box::new(value),
                    },
                }
            } else {
                let tok = *self.peek();
                self.try_parse_expr()?.ok_or(ParseError::Expected {
                    expected: "表达式（参数）",
                    found: tok.kind,
                    span: tok.span.into(),
                })?
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

fn shift_source_span(span: miette::SourceSpan, base_offset: usize) -> miette::SourceSpan {
    let offset = span.offset();
    let len = span.len();
    (offset + base_offset, len).into()
}
