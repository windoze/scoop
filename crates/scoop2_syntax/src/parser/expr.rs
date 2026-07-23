//! 表达式解析（grammar §8）：Pratt 二元优先级 + postfix + 控制流 + handle/try。

use scoop2_base::Span;

use crate::ast::expr::*;
use crate::ast::{self, Ident, TypePath};
use crate::lexer::{self, float_literal, string_literal};
use crate::token::{Keyword, StringKind, Symbol, Token, TokenKind};

use super::{PResult, Parser};

/// 中缀解析模式：`when` 非块 arm body 内需要 `is TypeRef ->` 的 arm 起始抑制。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InfixMode {
    Normal,
    WhenArm,
}

impl<'a> Parser<'a> {
    // --------------------------------------------------------------
    // §8.0 入口
    // --------------------------------------------------------------

    /// `expr`：普通表达式入口；完整表达式后出现 `=` 是硬错误（§8.0）。
    pub(crate) fn expr(&mut self) -> PResult<Expr> {
        let lhs = self.parse_expr_bp(0, InfixMode::Normal)?;
        if self.at_sym(Symbol::Eq) {
            let eq = self.peek();
            self.err_assignment_in_expr(Span::new(lhs.span.start, eq.span.end));
            self.bump(); // `=`
            // 恢复：解析并丢弃右侧（保持 cursor 健康），错误已记录。
            let _ = self.expr()?;
        }
        Ok(lhs)
    }

    /// `whenArmExpr`：非块 `when` arm body（`is` arm 起始抑制，§8.5）。
    pub(crate) fn when_arm_expr(&mut self) -> PResult<Expr> {
        let lhs = self.parse_expr_bp(0, InfixMode::WhenArm)?;
        if self.at_sym(Symbol::Eq) {
            let eq = self.peek();
            self.err_assignment_in_expr(Span::new(lhs.span.start, eq.span.end));
            self.bump();
            let _ = self.expr()?;
        }
        Ok(lhs)
    }

    // --------------------------------------------------------------
    // §8.1 Pratt
    // --------------------------------------------------------------

    pub(crate) fn parse_expr_bp(&mut self, min_bp: u8, mode: InfixMode) -> PResult<Expr> {
        self.enter()?;
        let result = self.parse_expr_bp_inner(min_bp, mode);
        self.leave();
        result
    }

    fn parse_expr_bp_inner(&mut self, min_bp: u8, mode: InfixMode) -> PResult<Expr> {
        let mut lhs = self.parse_prefix(mode)?;

        while let Some(infix) = self.peek_infix_op(mode) {
            if infix.l_bp < min_bp {
                break;
            }

            match infix.kind {
                InfixOpKind::Binary(op) => {
                    self.bump(); // operator token
                    let rhs = self.parse_expr_bp(infix.r_bp, mode)?;
                    lhs = Expr {
                        id: self.nid(),
                        span: Span::new(lhs.span.start, rhs.span.end),
                        kind: ExprKind::Binary {
                            lhs: Box::new(lhs),
                            op,
                            rhs: Box::new(rhs),
                        },
                    };
                }
                InfixOpKind::ContextualInfix => {
                    let name_tok = self.bump();
                    let name = self.ident(name_tok);
                    let rhs = self.parse_expr_bp(infix.r_bp, mode)?;
                    lhs = Expr {
                        id: self.nid(),
                        span: Span::new(lhs.span.start, rhs.span.end),
                        kind: ExprKind::InfixCall {
                            receiver: Box::new(lhs),
                            name,
                            arg: Box::new(rhs),
                        },
                    };
                }
                InfixOpKind::NotIs => {
                    self.bump(); // `!`
                    self.bump(); // `is`
                    let ty = self.parse_type_ref()?;
                    lhs = Expr {
                        id: self.nid(),
                        span: Span::new(lhs.span.start, ty.span.end),
                        kind: ExprKind::TypeCheck {
                            expr: Box::new(lhs),
                            op: TypeCheckOp::NotIs,
                            ty,
                        },
                    };
                }
                InfixOpKind::Is => {
                    self.bump();
                    let ty = self.parse_type_ref()?;
                    lhs = Expr {
                        id: self.nid(),
                        span: Span::new(lhs.span.start, ty.span.end),
                        kind: ExprKind::TypeCheck {
                            expr: Box::new(lhs),
                            op: TypeCheckOp::Is,
                            ty,
                        },
                    };
                }
                InfixOpKind::As => {
                    self.bump();
                    let ty = self.parse_type_ref()?;
                    lhs = Expr {
                        id: self.nid(),
                        span: Span::new(lhs.span.start, ty.span.end),
                        kind: ExprKind::Cast {
                            expr: Box::new(lhs),
                            op: CastOp::As,
                            ty,
                        },
                    };
                }
                InfixOpKind::AsSafe => {
                    self.bump();
                    let ty = self.parse_type_ref()?;
                    lhs = Expr {
                        id: self.nid(),
                        span: Span::new(lhs.span.start, ty.span.end),
                        kind: ExprKind::Cast {
                            expr: Box::new(lhs),
                            op: CastOp::AsSafe,
                            ty,
                        },
                    };
                }
            }
        }

        Ok(lhs)
    }

    fn peek_infix_op(&self, mode: InfixMode) -> Option<InfixOp> {
        // `!is`（两个 token）。
        if self.at_sym(Symbol::Bang) && self.peek_n(1).kind == TokenKind::Keyword(Keyword::Is) {
            return Some(InfixOp {
                l_bp: 8,
                r_bp: 9,
                kind: InfixOpKind::NotIs,
            });
        }

        // 上下文中缀标识符：`until` / `downTo` / `step`（§8.1.1）。
        if self.at_kind(TokenKind::Ident) {
            match self.token_text(self.peek()) {
                "until" | "downTo" | "step" => {
                    return Some(InfixOp {
                        l_bp: 8,
                        r_bp: 9,
                        kind: InfixOpKind::ContextualInfix,
                    });
                }
                _ => {}
            }
        }

        if let TokenKind::Keyword(kw) = self.peek().kind {
            match kw {
                Keyword::Is => {
                    // when 非块 arm body：`is TypeRef ->` 是下一个 arm 的起始（§8.5）。
                    if mode == InfixMode::WhenArm && self.looks_like_when_is_arm_start() {
                        return None;
                    }
                    return Some(InfixOp {
                        l_bp: 8,
                        r_bp: 9,
                        kind: InfixOpKind::Is,
                    });
                }
                Keyword::As => {
                    return Some(InfixOp {
                        l_bp: 8,
                        r_bp: 9,
                        kind: InfixOpKind::As,
                    });
                }
                Keyword::AsQ => {
                    return Some(InfixOp {
                        l_bp: 8,
                        r_bp: 9,
                        kind: InfixOpKind::AsSafe,
                    });
                }
                _ => return None,
            }
        }

        let TokenKind::Symbol(sym) = self.peek().kind else {
            return None;
        };
        let (l_bp, r_bp, op) = binary_binding_power(sym)?;
        Some(InfixOp {
            l_bp,
            r_bp,
            kind: InfixOpKind::Binary(op),
        })
    }

    // --------------------------------------------------------------
    // §8.3 前缀
    // --------------------------------------------------------------

    fn parse_prefix(&mut self, mode: InfixMode) -> PResult<Expr> {
        // 注解前缀表达式：`@Unsafe do {}` / `@Safe ...` / `@Ann expr`（§8.3）。
        if self.at_sym(Symbol::At) && self.peek_n(1).kind == TokenKind::Ident {
            match self.token_text(self.peek_n(1)) {
                "Unsafe" => return self.parse_unsafe_block_expr(),
                "Safe" => return self.parse_safe_annotated_expr(),
                _ => return self.parse_generic_annotated_expr(mode),
            }
        }

        // `perform` 已移除（§10）。
        if self.at_kw(Keyword::Perform) {
            let span = self.peek().span;
            return Err(self.err_perform_removed(span));
        }

        // `*` 在表达式起始：spread 越界（§10）。
        if self.at_sym(Symbol::Star) {
            let span = self.peek().span;
            return Err(self.err_spread_outside_call(span));
        }

        let op = match self.peek().kind {
            TokenKind::Symbol(Symbol::Bang) => Some(UnaryOp::Not),
            TokenKind::Symbol(Symbol::Minus) => Some(UnaryOp::Neg),
            TokenKind::Symbol(Symbol::Tilde) => Some(UnaryOp::BitNot),
            _ => None,
        };
        if let Some(op) = op {
            let op_tok = self.bump();
            let operand = self.parse_prefix(mode)?;
            return Ok(Expr {
                id: self.nid(),
                span: Span::new(op_tok.span.start, operand.span.end),
                kind: ExprKind::Unary {
                    op,
                    expr: Box::new(operand),
                },
            });
        }

        self.parse_postfix(mode)
    }

    fn parse_generic_annotated_expr(&mut self, mode: InfixMode) -> PResult<Expr> {
        let start = self.peek().span.start;
        let mut annotations = Vec::new();
        while self.at_sym(Symbol::At) && self.peek_n(1).kind == TokenKind::Ident {
            annotations.push(self.parse_annotation_use()?);
        }
        let inner = self.parse_prefix(mode)?;
        Ok(Expr {
            id: self.nid(),
            span: Span::new(start, inner.span.end),
            kind: ExprKind::Annotated {
                annotations,
                expr: Box::new(inner),
            },
        })
    }

    fn parse_unsafe_block_expr(&mut self) -> PResult<Expr> {
        let at = self.expect_sym(Symbol::At)?;
        let start = at.span.start;
        self.expect_ident("`Unsafe`（unsafe block 注解名）")?;

        if !self.eat_kw(Keyword::Do) {
            let tok = self.peek();
            if tok.kind == TokenKind::Symbol(Symbol::LBrace) {
                return Err(self.err_unsafe_requires_do(tok.span));
            }
            return Err(self.err_expected("`do`（写作 `@Unsafe do { ... }`）", tok));
        }
        let body = self.parse_block()?;
        Ok(Expr {
            id: self.nid(),
            span: Span::new(start, body.span.end),
            kind: ExprKind::UnsafeBlock(body),
        })
    }

    fn parse_safe_annotated_expr(&mut self) -> PResult<Expr> {
        let at = self.expect_sym(Symbol::At)?;
        let start = at.span.start;
        self.expect_ident("`Safe`（safe block 注解名）")?;

        if self.eat_kw(Keyword::Do) {
            let body = self.parse_block()?;
            return Ok(Expr {
                id: self.nid(),
                span: Span::new(start, body.span.end),
                kind: ExprKind::SafeBlock(body),
            });
        }

        if self.at_sym(Symbol::LBrace) {
            let (mut lambda, lambda_span) = self.parse_lambda_expr()?;
            lambda.is_safe = true;
            return Ok(Expr {
                id: self.nid(),
                span: Span::new(start, lambda_span.end),
                kind: ExprKind::Lambda(lambda),
            });
        }

        let tok = self.peek();
        Err(self.err_expected("`do { ... }` 或 closure `{ ... }`", tok))
    }

    // --------------------------------------------------------------
    // §8.4 postfix
    // --------------------------------------------------------------

    fn parse_postfix(&mut self, _mode: InfixMode) -> PResult<Expr> {
        let mut expr = self.parse_atom()?;

        loop {
            // 值类型 with 更新：`expr with { ... }`。
            if self.at_kw(Keyword::With) {
                expr = self.parse_with_update_expr(expr)?;
                continue;
            }
            // 显式类型应用 `expr<T>`。
            if self.at_sym(Symbol::Lt) && self.looks_like_type_apply_expr() {
                let (args, end) = self.parse_type_args()?;
                expr = Expr {
                    id: self.nid(),
                    span: Span::new(expr.span.start, end),
                    kind: ExprKind::TypeApply {
                        callee: Box::new(expr),
                        args,
                    },
                };
                continue;
            }
            // class 字面量 `T::class`。
            if self.at_sym(Symbol::Colon)
                && self.at_sym_n(1, Symbol::Colon)
                && self.peek_n(2).kind == TokenKind::Keyword(Keyword::Class)
            {
                expr = self.parse_class_lit_expr(expr)?;
                continue;
            }
            if self.at_sym(Symbol::Dot) {
                expr = self.parse_member_access_expr(expr)?;
                continue;
            }
            if self.at_sym(Symbol::QuestionDot) {
                expr = self.parse_safe_member_access_expr(expr)?;
                continue;
            }
            // 下标读取 `a[i, j]`（§8.4 indexPostfix）。
            if self.at_sym(Symbol::LBracket) {
                expr = self.parse_index_expr(expr)?;
                continue;
            }
            if self.at_sym(Symbol::LParen) {
                let (args_span, args) = self.parse_call_arg_list()?;
                expr = Expr {
                    id: self.nid(),
                    span: Span::new(expr.span.start, args_span.end),
                    kind: ExprKind::Call {
                        callee: Box::new(expr),
                        args,
                    },
                };
                continue;
            }
            if self.at_sym(Symbol::LBrace) {
                // Kotlin 风格 trailing lambda：折叠进 Call args（§8.4）。
                let (lambda_expr, lambda_span) = self.parse_lambda_expr_token()?;
                let end = lambda_span.end;
                let arg = CallArg {
                    id: self.nid(),
                    span: lambda_span,
                    name: None,
                    is_spread: false,
                    value: lambda_expr,
                };
                let start = expr.span.start;
                expr = match expr.kind {
                    ExprKind::Call { callee, mut args } => {
                        args.push(arg);
                        Expr {
                            id: self.nid(),
                            span: Span::new(start, end),
                            kind: ExprKind::Call { callee, args },
                        }
                    }
                    _ => Expr {
                        id: self.nid(),
                        span: Span::new(start, end),
                        kind: ExprKind::Call {
                            callee: Box::new(expr),
                            args: vec![arg],
                        },
                    },
                };
                continue;
            }
            if self.at_sym(Symbol::BangBang) {
                let op = self.bump();
                expr = Expr {
                    id: self.nid(),
                    span: Span::new(expr.span.start, op.span.end),
                    kind: ExprKind::NotNullAssert {
                        expr: Box::new(expr),
                    },
                };
                continue;
            }
            break;
        }

        Ok(expr)
    }

    fn parse_member_access_expr(&mut self, receiver: Expr) -> PResult<Expr> {
        self.expect_sym(Symbol::Dot)?;

        // splice 字段访问 `receiver.[field]`（§8.4，spec §6.4）。
        if self.at_sym(Symbol::LBracket) {
            let open = self.bump();
            let field = self.expr()?;
            if self.at_eof() {
                return Err(self.err_unclosed(open.span.start, "`]`"));
            }
            let close = self.expect_sym(Symbol::RBracket)?;
            return Ok(Expr {
                id: self.nid(),
                span: Span::new(receiver.span.start, close.span.end),
                kind: ExprKind::SpliceField {
                    receiver: Box::new(receiver),
                    field: Box::new(field),
                },
            });
        }

        let member = self.parse_member_segment("成员名（标识符或 tuple 索引）")?;
        let end = member_name_end(&member);
        Ok(Expr {
            id: self.nid(),
            span: Span::new(receiver.span.start, end),
            kind: ExprKind::MemberAccess {
                receiver: Box::new(receiver),
                member,
            },
        })
    }

    fn parse_safe_member_access_expr(&mut self, receiver: Expr) -> PResult<Expr> {
        self.expect_sym(Symbol::QuestionDot)?;
        let member = self.parse_member_segment("成员名（标识符或 tuple 索引）")?;
        let end = member_name_end(&member);
        Ok(Expr {
            id: self.nid(),
            span: Span::new(receiver.span.start, end),
            kind: ExprKind::SafeMemberAccess {
                receiver: Box::new(receiver),
                member,
            },
        })
    }

    fn parse_member_segment(&mut self, what: &str) -> PResult<MemberName> {
        let tok = self.peek();
        match tok.kind {
            TokenKind::Ident => {
                self.bump();
                Ok(MemberName::Named(self.ident(tok)))
            }
            TokenKind::IntLiteral => {
                self.bump();
                let lit = self.decode_int(tok);
                Ok(MemberName::TupleIndex {
                    value: lit.value,
                    span: tok.span,
                })
            }
            _ => Err(self.err_expected(what, tok)),
        }
    }

    /// `a[i, j]`（多下标；operator get 解析是 typecheck 的事）。
    fn parse_index_expr(&mut self, receiver: Expr) -> PResult<Expr> {
        let open = self.expect_sym(Symbol::LBracket)?;
        let mut indices = vec![self.expr()?];
        while self.eat_sym(Symbol::Comma) {
            if self.at_sym(Symbol::RBracket) {
                break;
            }
            indices.push(self.expr()?);
        }
        if self.at_eof() {
            return Err(self.err_unclosed(open.span.start, "`]`"));
        }
        let close = self.expect_sym(Symbol::RBracket)?;
        Ok(Expr {
            id: self.nid(),
            span: Span::new(receiver.span.start, close.span.end),
            kind: ExprKind::Index {
                receiver: Box::new(receiver),
                indices,
            },
        })
    }

    /// 调用实参列表 `(args...)`（也用于 supertype ctor args / 构造委托）。
    pub(crate) fn parse_call_arg_list(&mut self) -> PResult<(Span, Vec<CallArg>)> {
        let open = self.expect_sym(Symbol::LParen)?;
        let start = open.span.start;

        let mut args = Vec::new();
        if self.at_sym(Symbol::RParen) {
            let close = self.bump();
            return Ok((Span::new(start, close.span.end), args));
        }

        loop {
            let arg_start = self.peek().span.start;
            // 命名实参 `name = ...`（仅参数列表内）。
            let name = if self.at_kind(TokenKind::Ident) && self.at_sym_n(1, Symbol::Eq) {
                let name_tok = self.bump();
                self.bump(); // `=`
                Some(self.ident(name_tok))
            } else {
                None
            };
            // spread：`*expr` / 命名 spread `name = *expr`。
            let is_spread = self.eat_sym(Symbol::Star);
            let value = self.expr()?;
            let end = value.span.end;
            args.push(CallArg {
                id: self.nid(),
                span: Span::new(arg_start, end),
                name,
                is_spread,
                value,
            });

            if self.eat_sym(Symbol::Comma) {
                if self.at_sym(Symbol::RParen) {
                    break;
                }
                continue;
            }
            break;
        }

        if self.at_eof() {
            return Err(self.err_unclosed(start, "`)`"));
        }
        let close = self.expect_sym(Symbol::RParen)?;
        Ok((Span::new(start, close.span.end), args))
    }

    fn parse_class_lit_expr(&mut self, receiver: Expr) -> PResult<Expr> {
        let receiver_span = receiver.span;
        let Some(path) = type_path_from_expr(&receiver) else {
            return Err(self.err_class_lit_receiver(receiver_span));
        };
        self.bump(); // `:`
        self.bump(); // `:`
        let class_kw = self.expect_kw(Keyword::Class)?;
        Ok(Expr {
            id: self.nid(),
            span: Span::new(receiver_span.start, class_kw.span.end),
            kind: ExprKind::ClassLit { path },
        })
    }

    // --------------------------------------------------------------
    // §8.2 atom
    // --------------------------------------------------------------

    fn parse_atom(&mut self) -> PResult<Expr> {
        let tok = self.peek();

        match tok.kind {
            TokenKind::Ident => {
                // `Ident {` 的 struct-lit / trailing-lambda 消歧（§8.2 normative）。
                if self.at_sym_n(1, Symbol::LBrace)
                    && self.disambiguate_ident_lbrace_group() == BraceGroupKind::StructLit
                {
                    return self.parse_struct_lit_expr();
                }
                self.bump();
                let ident = self.ident(tok);
                Ok(Expr {
                    id: self.nid(),
                    span: tok.span,
                    kind: ExprKind::Ident(ident),
                })
            }
            TokenKind::IntLiteral => {
                self.bump();
                let lit = self.decode_int(tok);
                Ok(Expr {
                    id: self.nid(),
                    span: tok.span,
                    kind: ExprKind::IntLit(lit),
                })
            }
            TokenKind::FloatLiteral => {
                self.bump();
                let lit = self.decode_float(tok);
                Ok(Expr {
                    id: self.nid(),
                    span: tok.span,
                    kind: ExprKind::FloatLit(lit),
                })
            }
            TokenKind::CharLiteral => {
                self.bump();
                let lit = self.decode_char(tok);
                Ok(Expr {
                    id: self.nid(),
                    span: tok.span,
                    kind: ExprKind::CharLit(lit),
                })
            }
            TokenKind::StringLiteral(kind) => {
                self.bump();
                match kind {
                    StringKind::Normal {
                        interpolated: true,
                    } => self.parse_interpolated_string_expr(tok, false),
                    StringKind::Raw {
                        interpolated: true,
                    } => self.parse_interpolated_string_expr(tok, true),
                    _ => {
                        let lit = self.decode_string(tok);
                        Ok(Expr {
                            id: self.nid(),
                            span: tok.span,
                            kind: ExprKind::StringLit(lit),
                        })
                    }
                }
            }
            TokenKind::Keyword(Keyword::If) => self.parse_if_expr(),
            TokenKind::Keyword(Keyword::When) => self.parse_when_expr(),
            TokenKind::Keyword(Keyword::Handle) => self.parse_handle_expr(),
            TokenKind::Keyword(Keyword::Try) => self.parse_try_expr(),
            TokenKind::Keyword(Keyword::Do) => {
                self.bump();
                let body = self.parse_block()?;
                Ok(Expr {
                    id: self.nid(),
                    span: Span::new(tok.span.start, body.span.end),
                    kind: ExprKind::DoBlock(body),
                })
            }
            TokenKind::Keyword(Keyword::Object) => self.parse_anonymous_object_expr(),
            TokenKind::Symbol(Symbol::LBrace) => {
                let (lambda, span) = self.parse_lambda_expr()?;
                Ok(Expr {
                    id: self.nid(),
                    span,
                    kind: ExprKind::Lambda(lambda),
                })
            }
            TokenKind::Symbol(Symbol::LParen) => self.parse_paren_expr(),
            TokenKind::Symbol(Symbol::LBracket) => self.parse_array_lit_expr(),
            _ => Err(self.err_expected_expr(tok)),
        }
    }

    /// 匿名 object 表达式不在语言内（§10/§11）：记录专用错误后 best-effort 消费。
    fn parse_anonymous_object_expr(&mut self) -> PResult<Expr> {
        let kw = self.bump(); // `object`
        self.err_anonymous_object(kw.span);

        // best-effort 消费可选名字 / 超类型 / body，保持 cursor 健康。
        if self.at_kind(TokenKind::Ident) {
            self.bump();
        }
        if self.eat_sym(Symbol::Colon) {
            let _ = self.parse_type_ref();
        }
        if self.at_sym(Symbol::LBrace) {
            let block = self.parse_block()?;
            return Ok(Expr {
                id: self.nid(),
                span: Span::new(kw.span.start, block.span.end),
                kind: ExprKind::Block(block),
            });
        }
        Ok(Expr {
            id: self.nid(),
            span: kw.span,
            kind: ExprKind::UnitLit,
        })
    }

    fn parse_paren_expr(&mut self) -> PResult<Expr> {
        let open = self.expect_sym(Symbol::LParen)?;
        let start = open.span.start;

        // `()` Unit 字面量。
        if self.at_sym(Symbol::RParen) {
            let close = self.bump();
            return Ok(Expr {
                id: self.nid(),
                span: Span::new(start, close.span.end),
                kind: ExprKind::UnitLit,
            });
        }

        let first = self.expr()?;

        // tuple literal：`(a, b, ...)` / `(a,)`。
        if self.eat_sym(Symbol::Comma) {
            let mut elements = vec![first];
            while !self.at_sym(Symbol::RParen) && !self.at_eof() {
                elements.push(self.expr()?);
                if self.eat_sym(Symbol::Comma) {
                    if self.at_sym(Symbol::RParen) {
                        break;
                    }
                    continue;
                }
                break;
            }
            let close = self.expect_sym(Symbol::RParen)?;
            return Ok(Expr {
                id: self.nid(),
                span: Span::new(start, close.span.end),
                kind: ExprKind::TupleLit(elements),
            });
        }

        // 透明分组 `(expr)`：非字面量 inner 采用括号 span（AST 可见的约定）。
        if self.at_sym(Symbol::RParen) {
            let close = self.bump();
            let mut inner = first;
            if !matches!(
                inner.kind,
                ExprKind::IntLit(_)
                    | ExprKind::FloatLit(_)
                    | ExprKind::CharLit(_)
                    | ExprKind::StringLit(_)
                    | ExprKind::InterpolatedString { .. }
            ) {
                inner.span = Span::new(start, close.span.end);
            }
            return Ok(inner);
        }

        let tok = self.peek();
        Err(self.err_expected_token("`)`", tok))
    }

    fn parse_array_lit_expr(&mut self) -> PResult<Expr> {
        let open = self.expect_sym(Symbol::LBracket)?;
        let start = open.span.start;

        let mut elements = Vec::new();
        if !self.at_sym(Symbol::RBracket) {
            loop {
                // `name = expr` 在数组字面量中是专用错误（§10）。
                if self.at_kind(TokenKind::Ident) && self.at_sym_n(1, Symbol::Eq) {
                    let name = self.peek();
                    let eq = self.peek_n(1);
                    return Err(
                        self.err_named_arg_outside_call(Span::new(name.span.start, eq.span.end))
                    );
                }
                elements.push(self.expr()?);
                if self.eat_sym(Symbol::Comma) {
                    if self.at_sym(Symbol::RBracket) {
                        break;
                    }
                    continue;
                }
                break;
            }
        }

        if self.at_eof() {
            return Err(self.err_unclosed(start, "`]`"));
        }
        let close = self.expect_sym(Symbol::RBracket)?;
        Ok(Expr {
            id: self.nid(),
            span: Span::new(start, close.span.end),
            kind: ExprKind::ArrayLit(elements),
        })
    }

    fn parse_struct_lit_expr(&mut self) -> PResult<Expr> {
        let name_tok = self.expect_ident("类型名")?;
        let start = name_tok.span.start;
        let name = self.ident(name_tok);

        self.expect_sym(Symbol::LBrace)?;
        let mut fields = Vec::new();
        if !self.at_sym(Symbol::RBrace) {
            loop {
                let field_name_tok = self.expect_ident("字段名")?;
                let field_name = self.ident(field_name_tok);
                self.expect_sym(Symbol::Colon)?;
                let value = self.expr()?;
                fields.push(StructLitField {
                    id: self.nid(),
                    span: Span::new(field_name_tok.span.start, value.span.end),
                    name: field_name,
                    value,
                });
                if self.eat_sym(Symbol::Comma) {
                    if self.at_sym(Symbol::RBrace) {
                        break;
                    }
                    continue;
                }
                break;
            }
        }
        let close = self.expect_sym(Symbol::RBrace)?;
        Ok(Expr {
            id: self.nid(),
            span: Span::new(start, close.span.end),
            kind: ExprKind::StructLit { name, fields },
        })
    }

    // --------------------------------------------------------------
    // `Ident {` 消歧（§8.2 normative）
    // --------------------------------------------------------------

    fn disambiguate_ident_lbrace_group(&self) -> BraceGroupKind {
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
            TokenKind::Symbol(Symbol::Arrow | Symbol::Comma | Symbol::RBrace) => {
                BraceGroupKind::Lambda
            }
            TokenKind::Symbol(Symbol::Colon) => {
                if self.brace_group_has_top_level_arrow(self.i + 1) {
                    BraceGroupKind::Lambda
                } else {
                    BraceGroupKind::StructLit
                }
            }
            // `it * 2` / `it.foo` 等：普通表达式 body 的 lambda。
            TokenKind::Symbol(_) | TokenKind::Keyword(_) => BraceGroupKind::Lambda,
            // `Point { x 1 }` 等：更像 struct literal 缺 `:`（产生 targeted 诊断）。
            _ => BraceGroupKind::StructLit,
        }
    }

    fn brace_group_has_top_level_arrow(&self, open_brace_index: usize) -> bool {
        let mut depth_paren = 0usize;
        let mut depth_brace = 0usize;
        let mut depth_bracket = 0usize;

        let mut idx = open_brace_index + 1;
        while let Some(tok) = self.tokens.get(idx) {
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

    // --------------------------------------------------------------
    // lambda（§8.2）
    // --------------------------------------------------------------

    fn parse_lambda_expr_token(&mut self) -> PResult<(Expr, Span)> {
        let (lambda, span) = self.parse_lambda_expr()?;
        Ok((
            Expr {
                id: self.nid(),
                span,
                kind: ExprKind::Lambda(lambda),
            },
            span,
        ))
    }

    /// 解析 lambda；返回 lambda 与其完整 span（`{` 到 `}`）。
    pub(crate) fn parse_lambda_expr(&mut self) -> PResult<(LambdaExpr, Span)> {
        let open = self.expect_sym(Symbol::LBrace)?;
        let start = open.span.start;

        let mut params = Vec::new();
        if self.at_sym(Symbol::Arrow) {
            // `{ -> body }`：0 参数显式箭头。
            self.bump();
        } else if let Some(ps) = self.try_parse_lambda_params_and_arrow()? {
            params = ps;
        }

        let mut semi_flags = Vec::new();
        let block = self.parse_block_with_open(open, Some(&mut semi_flags))?;
        let span = Span::new(start, block.span.end);

        // 解包规则（§8.2）：单条无尾 `;` 的表达式语句 → 该表达式（lambda 的值）。
        let Block {
            id: block_id,
            span: block_span,
            stmts,
        } = block;
        let single_expr = stmts.len() == 1
            && matches!(semi_flags.first(), Some(false))
            && matches!(stmts.first().map(|s| &s.kind), Some(StmtKind::Expr(_)));
        let body = if single_expr {
            let mut stmts = stmts;
            match stmts.pop() {
                Some(Stmt {
                    kind: StmtKind::Expr(expr),
                    ..
                }) => LambdaBody::Expr(Box::new(expr)),
                other => {
                    // invariant: single_expr 已判定形态，此处仅为防御性兜底。
                    let mut restored = Vec::new();
                    restored.extend(other);
                    LambdaBody::Block(Block {
                        id: block_id,
                        span: block_span,
                        stmts: restored,
                    })
                }
            }
        } else {
            LambdaBody::Block(Block {
                id: block_id,
                span: block_span,
                stmts,
            })
        };

        Ok((
            LambdaExpr {
                is_safe: false,
                params,
                body,
            },
            span,
        ))
    }

    /// 投机解析 lambda 参数列表 + `->`；失败则回退（不消费 token）。
    fn try_parse_lambda_params_and_arrow(&mut self) -> PResult<Option<Vec<LambdaParam>>> {
        if !self.at_kind(TokenKind::Ident) {
            return Ok(None);
        }
        let checkpoint = self.i;
        let mut params = Vec::new();

        loop {
            if !self.at_kind(TokenKind::Ident) {
                self.i = checkpoint;
                return Ok(None);
            }
            let name_tok = self.bump();
            let name = self.ident(name_tok);
            let ty = if self.eat_sym(Symbol::Colon) {
                Some(self.parse_type_ref()?)
            } else {
                None
            };
            let end = ty
                .as_ref()
                .map(|t| t.span.end)
                .unwrap_or(name_tok.span.end);
            params.push(LambdaParam {
                id: self.nid(),
                span: Span::new(name_tok.span.start, end),
                name,
                ty,
            });

            if self.at_sym(Symbol::Arrow) {
                self.bump();
                return Ok(Some(params));
            }
            if self.eat_sym(Symbol::Comma) {
                // 允许 `{ a, b, -> ... }` 的尾逗号。
                if self.at_sym(Symbol::Arrow) {
                    self.bump();
                    return Ok(Some(params));
                }
                continue;
            }
            self.i = checkpoint;
            return Ok(None);
        }
    }

    // --------------------------------------------------------------
    // control body / if / when（§8.5）
    // --------------------------------------------------------------

    /// `controlBody ::= block | expr`（block 时内层是 `ExprKind::Block`）。
    pub(crate) fn parse_control_body_expr(&mut self, expected: &str) -> PResult<Expr> {
        if self.at_sym(Symbol::LBrace) {
            let block = self.parse_block()?;
            let span = block.span;
            return Ok(Expr {
                id: self.nid(),
                span,
                kind: ExprKind::Block(block),
            });
        }
        let tok = self.peek();
        if !self.is_expr_start() {
            return Err(self.err_expected(expected, tok));
        }
        self.expr()
    }

    fn is_expr_start(&self) -> bool {
        matches!(
            self.peek().kind,
            TokenKind::Ident
                | TokenKind::IntLiteral
                | TokenKind::FloatLiteral
                | TokenKind::CharLiteral
                | TokenKind::StringLiteral(_)
                | TokenKind::Keyword(
                    Keyword::If
                        | Keyword::When
                        | Keyword::Handle
                        | Keyword::Try
                        | Keyword::Do
                        | Keyword::Object
                        | Keyword::Perform
                )
                | TokenKind::Symbol(
                    Symbol::LBrace
                        | Symbol::LParen
                        | Symbol::LBracket
                        | Symbol::At
                        | Symbol::Bang
                        | Symbol::Minus
                        | Symbol::Tilde
                        | Symbol::Star
                )
        )
    }

    fn parse_if_expr(&mut self) -> PResult<Expr> {
        let if_kw = self.expect_kw(Keyword::If)?;
        let start = if_kw.span.start;

        let open = self.expect_sym(Symbol::LParen)?;
        let cond = self.expr()?;
        if self.at_eof() {
            return Err(self.err_unclosed(open.span.start, "`)`"));
        }
        self.expect_sym(Symbol::RParen)?;

        let then_branch = self.parse_control_body_expr("表达式（then 分支）")?;
        let (end, else_branch) = if self.at_kw(Keyword::Else) {
            self.bump();
            let else_expr = self.parse_control_body_expr("表达式（else 分支）")?;
            (else_expr.span.end, Some(Box::new(else_expr)))
        } else {
            (then_branch.span.end, None)
        };

        Ok(Expr {
            id: self.nid(),
            span: Span::new(start, end),
            kind: ExprKind::If {
                cond: Box::new(cond),
                then_branch: Box::new(then_branch),
                else_branch,
            },
        })
    }

    fn parse_when_expr(&mut self) -> PResult<Expr> {
        let when_kw = self.expect_kw(Keyword::When)?;
        let start = when_kw.span.start;

        let open = self.expect_sym(Symbol::LParen)?;
        let subject = self.expr()?;
        if self.at_eof() {
            return Err(self.err_unclosed(open.span.start, "`)`"));
        }
        self.expect_sym(Symbol::RParen)?;

        let open_brace = self.expect_sym(Symbol::LBrace)?;
        let mut arms = Vec::new();

        while !self.at_eof() && !self.at_sym(Symbol::RBrace) {
            while self.eat_sym(Symbol::Semicolon) {}
            if self.at_sym(Symbol::RBrace) {
                break;
            }

            let pat = self.parse_when_pat()?;
            let pat_span = pat.span;

            let guard = if self.at_kw(Keyword::If) {
                self.bump();
                Some(self.expr()?)
            } else {
                None
            };

            self.expect_sym(Symbol::Arrow)?;

            let body = if self.at_sym(Symbol::LBrace) {
                let block = self.parse_block()?;
                let span = block.span;
                Expr {
                    id: self.nid(),
                    span,
                    kind: ExprKind::Block(block),
                }
            } else {
                self.when_arm_expr()?
            };

            arms.push(WhenArm {
                id: self.nid(),
                span: Span::new(pat_span.start, body.span.end),
                pat,
                guard,
                body,
            });

            while self.eat_sym(Symbol::Semicolon) {}
        }

        if self.at_eof() {
            return Err(self.err_unclosed(open_brace.span.start, "`}`"));
        }
        let close = self.expect_sym(Symbol::RBrace)?;

        Ok(Expr {
            id: self.nid(),
            span: Span::new(start, close.span.end),
            kind: ExprKind::When {
                subject: Box::new(subject),
                arms,
            },
        })
    }

    // --------------------------------------------------------------
    // handle / try（§8.6）
    // --------------------------------------------------------------

    fn parse_handle_expr(&mut self) -> PResult<Expr> {
        let handle_kw = self.expect_kw(Keyword::Handle)?;
        let start = handle_kw.span.start;

        let body = self.parse_block()?;

        self.expect_handle_on_keyword()?;

        let open = self.expect_sym(Symbol::LBrace)?;
        let mut arms = Vec::new();

        while !self.at_eof() && !self.at_sym(Symbol::RBrace) {
            while self.eat_sym(Symbol::Semicolon) {}
            if self.at_sym(Symbol::RBrace) {
                break;
            }

            match self.parse_handle_arm() {
                Ok(arm) => arms.push(arm),
                Err(_abort) => {
                    // 错误已记录；arm 级恢复（§8.6）。
                    self.recover_to_handle_arm_sync();
                }
            }

            while self.eat_sym(Symbol::Semicolon) {}
        }

        if self.at_eof() {
            return Err(self.err_unclosed(open.span.start, "`}`"));
        }
        let close = self.expect_sym(Symbol::RBrace)?;

        let finally = if self.at_kw(Keyword::Finally) {
            self.bump();
            Some(self.parse_block()?)
        } else {
            None
        };

        let end = finally
            .as_ref()
            .map(|b| b.span.end)
            .unwrap_or(close.span.end);

        Ok(Expr {
            id: self.nid(),
            span: Span::new(start, end),
            kind: ExprKind::Handle {
                body,
                arms,
                finally,
            },
        })
    }

    fn expect_handle_on_keyword(&mut self) -> PResult<()> {
        if self.eat_kw(Keyword::On) {
            return Ok(());
        }

        // `handle {..} with {..}`：整个 with 段（+可选 finally）消费后报专用错误（§10）。
        if self.at_kw(Keyword::With) {
            let with_kw = self.bump();
            if self.at_sym(Symbol::LBrace) {
                let open = self.bump();
                let _ =
                    self.consume_balanced_after_open(Symbol::LBrace, Symbol::RBrace, open.span.start);
                if self.at_kw(Keyword::Finally) {
                    self.bump();
                    if self.at_sym(Symbol::LBrace) {
                        let open = self.bump();
                        let _ = self.consume_balanced_after_open(
                            Symbol::LBrace,
                            Symbol::RBrace,
                            open.span.start,
                        );
                    }
                }
            }
            return Err(self.err_handler_with_removed(with_kw.span));
        }

        let tok = self.peek();
        Err(self.err_expected_token("`on`", tok))
    }

    fn parse_handle_arm(&mut self) -> PResult<HandleArm> {
        let op = self.parse_handle_op()?;

        // escape continuation：`Effect.op(...), k -> body`。
        if self.eat_sym(Symbol::Comma) {
            let k_tok = self.expect_ident("continuation binder")?;
            let k = self.ident(k_tok);
            self.expect_sym(Symbol::Arrow)?;
            let body = self.parse_control_body_expr("表达式（handler arm body）")?;
            return Ok(HandleArm {
                id: self.nid(),
                span: Span::new(op.span.start, body.span.end),
                op,
                escape_continuation: Some(k),
                body,
            });
        }

        self.expect_sym(Symbol::Arrow)?;

        // `-> resume { ... }` 已移除：消费 block 后报专用错误（§10）。
        if self.at_ident_text("resume") && self.at_sym_n(1, Symbol::LBrace) {
            let resume_tok = self.bump();
            let _ = self.parse_block();
            return Err(self.err_resume_removed(resume_tok.span));
        }

        let body = self.parse_control_body_expr("表达式（handler arm body）")?;
        Ok(HandleArm {
            id: self.nid(),
            span: Span::new(op.span.start, body.span.end),
            op,
            escape_continuation: None,
            body,
        })
    }

    fn parse_handle_op(&mut self) -> PResult<HandleOp> {
        let first = self.expect_ident("effect operation 名")?;
        let start = first.span.start;
        let mut segments = vec![self.ident(first)];

        while self.at_sym(Symbol::Dot) && self.peek_n(1).kind == TokenKind::Ident {
            self.bump(); // `.`
            let seg = self.bump();
            segments.push(self.ident(seg));
        }

        // `Path<Args>.op(...)` 形态（type args 在 `.` 之前）。
        let (effect_path, effect_args, op) = if self.at_sym(Symbol::Lt)
            && self.type_args_followed_by_dot_ident_at(self.i)
        {
            let (args, gt_end) = self.parse_type_args()?;
            self.expect_sym(Symbol::Dot)?;
            let op_tok = self.expect_ident("effect operation 名")?;
            let op = self.ident(op_tok);
            (
                TypePath {
                    segments,
                    span: Span::new(start, gt_end),
                },
                args,
                op,
            )
        } else {
            if segments.len() < 2 {
                let tok = self.peek();
                return Err(self.err_expected("effect operation（例如 `Raise.raise(...)`）", tok));
            }
            let op = segments.pop().unwrap_or(Ident {
                symbol: self.intern("_"),
                span: Span::synthetic(),
            });
            let effect_end = segments.last().map(|s| s.span.end).unwrap_or(start);
            (
                TypePath {
                    segments,
                    span: Span::new(start, effect_end),
                },
                Vec::new(),
                op,
            )
        };

        // op 自己的类型实参（`Query.ask<Int>(...)`）。
        let op_type_args = if self.at_sym(Symbol::Lt) {
            let (args, _end) = self.parse_type_args()?;
            args
        } else {
            Vec::new()
        };

        let open = self.expect_sym(Symbol::LParen)?;
        let mut binders = Vec::new();
        if !self.at_sym(Symbol::RParen) {
            loop {
                let name_tok = self.expect_ident("参数名")?;
                let name = self.ident(name_tok);
                let (ty, end) = if self.eat_sym(Symbol::Colon) {
                    let ty = self.parse_type_ref()?;
                    let end = ty.span.end;
                    (Some(ty), end)
                } else {
                    (None, name_tok.span.end)
                };
                binders.push(HandleBinder {
                    id: self.nid(),
                    span: Span::new(name_tok.span.start, end),
                    name,
                    ty,
                });
                if self.eat_sym(Symbol::Comma) {
                    if self.at_sym(Symbol::RParen) {
                        break;
                    }
                    continue;
                }
                break;
            }
        }

        if self.at_eof() {
            return Err(self.err_unclosed(open.span.start, "`)`"));
        }
        let close = self.expect_sym(Symbol::RParen)?;

        Ok(HandleOp {
            id: self.nid(),
            span: Span::new(start, close.span.end),
            effect_path,
            effect_args,
            op,
            op_type_args,
            binders,
        })
    }

    fn recover_to_handle_arm_sync(&mut self) {
        if self.at_eof() || self.at_sym(Symbol::RBrace) || self.at_sym(Symbol::Semicolon) {
            return;
        }
        if self.looks_like_handle_arm_start_at(self.i) {
            return;
        }

        let mut depth_paren = 0usize;
        let mut depth_brace = 0usize;
        let mut depth_bracket = 0usize;

        while !self.at_eof() {
            if depth_paren == 0
                && depth_brace == 0
                && depth_bracket == 0
                && (self.at_sym(Symbol::RBrace)
                    || self.at_sym(Symbol::Semicolon)
                    || self.looks_like_handle_arm_start_at(self.i))
            {
                break;
            }
            let tok = self.bump();
            match tok.kind {
                TokenKind::Symbol(Symbol::LParen) => depth_paren += 1,
                TokenKind::Symbol(Symbol::RParen) => depth_paren = depth_paren.saturating_sub(1),
                TokenKind::Symbol(Symbol::LBracket) => depth_bracket += 1,
                TokenKind::Symbol(Symbol::RBracket) => {
                    depth_bracket = depth_bracket.saturating_sub(1)
                }
                TokenKind::Symbol(Symbol::LBrace) => depth_brace += 1,
                TokenKind::Symbol(Symbol::RBrace) => depth_brace = depth_brace.saturating_sub(1),
                _ => {}
            }
        }
    }

    /// `try/catch/finally`：parser 层脱糖为 `handle` over `scoop.core.Raise.raise`（§8.6）。
    fn parse_try_expr(&mut self) -> PResult<Expr> {
        let try_kw = self.expect_kw(Keyword::Try)?;
        let start = try_kw.span.start;

        let body = self.parse_block()?;

        let mut arms: Vec<HandleArm> = Vec::new();
        let mut last_catch_end = body.span.end;

        let mut first = true;
        while first || self.at_kw(Keyword::Catch) {
            first = false;
            let catch_kw = self.expect_kw(Keyword::Catch)?;
            let catch_kw_span = catch_kw.span;

            let open_paren = self.expect_sym(Symbol::LParen)?;
            let binder_tok = self.expect_ident("catch 变量名")?;
            let binder_name = self.ident(binder_tok);

            self.expect_sym(Symbol::Colon)?;
            let binder_ty = self.parse_type_ref()?;

            if self.at_eof() {
                return Err(self.err_unclosed(open_paren.span.start, "`)`"));
            }
            let close_paren = self.expect_sym(Symbol::RParen)?;

            let catch_block = self.parse_block()?;
            let catch_span = catch_block.span;
            let catch_body = Expr {
                id: self.nid(),
                span: catch_span,
                kind: ExprKind::Block(catch_block),
            };
            last_catch_end = catch_body.span.end;

            // 脱糖：合成 `scoop.core.Raise.raise`（标识符取 `catch` 关键字 span）。
            let synth_span = catch_kw_span;
            let effect_path = TypePath {
                segments: vec![
                    self.synthetic_ident("scoop", synth_span),
                    self.synthetic_ident("core", synth_span),
                    self.synthetic_ident("Raise", synth_span),
                ],
                span: synth_span,
            };
            let op = HandleOp {
                id: self.nid(),
                span: Span::new(catch_kw_span.start, close_paren.span.end),
                effect_path,
                effect_args: Vec::new(),
                op: self.synthetic_ident("raise", synth_span),
                op_type_args: Vec::new(),
                binders: vec![HandleBinder {
                    id: self.nid(),
                    span: Span::new(binder_tok.span.start, binder_ty.span.end),
                    name: binder_name,
                    ty: Some(binder_ty),
                }],
            };

            arms.push(HandleArm {
                id: self.nid(),
                span: Span::new(op.span.start, catch_body.span.end),
                op,
                escape_continuation: None,
                body: catch_body,
            });
        }

        let finally = if self.at_kw(Keyword::Finally) {
            self.bump();
            Some(self.parse_block()?)
        } else {
            None
        };

        let end = finally
            .as_ref()
            .map(|b| b.span.end)
            .unwrap_or(last_catch_end);

        Ok(Expr {
            id: self.nid(),
            span: Span::new(start, end),
            kind: ExprKind::Handle {
                body,
                arms,
                finally,
            },
        })
    }

    // --------------------------------------------------------------
    // with 更新（§8.4，spec §2.6）
    // --------------------------------------------------------------

    fn parse_with_update_expr(&mut self, base: Expr) -> PResult<Expr> {
        self.expect_kw(Keyword::With)?;
        let start = base.span.start;

        let open = self.expect_sym(Symbol::LBrace)?;
        let mut updates = Vec::new();

        while !self.at_eof() && !self.at_sym(Symbol::RBrace) {
            let path = self.parse_field_path()?;
            self.expect_sym(Symbol::Colon)?;
            let value = self.expr()?;
            updates.push(WithUpdateField {
                id: self.nid(),
                span: Span::new(path.span.start, value.span.end),
                path,
                value,
            });
            if self.eat_sym(Symbol::Comma) {
                if self.at_sym(Symbol::RBrace) {
                    break;
                }
                continue;
            }
            break;
        }

        if self.at_eof() {
            return Err(self.err_unclosed(open.span.start, "`}`"));
        }
        let close = self.expect_sym(Symbol::RBrace)?;

        Ok(Expr {
            id: self.nid(),
            span: Span::new(start, close.span.end),
            kind: ExprKind::WithUpdate {
                base: Box::new(base),
                updates,
            },
        })
    }

    fn parse_field_path(&mut self) -> PResult<FieldPath> {
        let mut segments = self.expect_field_path_initial_segments()?;
        let start = match segments.first() {
            Some(MemberName::Named(ident)) => ident.span.start,
            Some(MemberName::TupleIndex { span, .. }) => span.start,
            None => self.peek().span.start,
        };

        while self.eat_sym(Symbol::Dot) {
            segments.push(self.parse_member_segment("字段路径（标识符或 tuple 索引）")?);
        }

        let end = segments.last().map(member_name_end).unwrap_or(start);
        Ok(FieldPath {
            span: Span::new(start, end),
            segments,
        })
    }

    fn expect_field_path_initial_segments(&mut self) -> PResult<Vec<MemberName>> {
        let tok = self.peek();
        match tok.kind {
            TokenKind::Ident => {
                self.bump();
                Ok(vec![MemberName::Named(self.ident(tok))])
            }
            TokenKind::IntLiteral => {
                self.bump();
                let lit = self.decode_int(tok);
                Ok(vec![MemberName::TupleIndex {
                    value: lit.value,
                    span: tok.span,
                }])
            }
            TokenKind::FloatLiteral => {
                // `with { 0.1: v }`：float token 拆成两个整数段（§8.4/§12.10）。
                if let Some((left, right)) = self.split_numeric_field_path_float_span(tok.span) {
                    self.bump();
                    let left_lit = self.decode_int(Token {
                        kind: TokenKind::IntLiteral,
                        span: left,
                    });
                    let right_lit = self.decode_int(Token {
                        kind: TokenKind::IntLiteral,
                        span: right,
                    });
                    Ok(vec![
                        MemberName::TupleIndex {
                            value: left_lit.value,
                            span: left,
                        },
                        MemberName::TupleIndex {
                            value: right_lit.value,
                            span: right,
                        },
                    ])
                } else {
                    Err(self.err_expected("字段路径（标识符或 tuple 索引）", tok))
                }
            }
            _ => Err(self.err_expected("字段路径（标识符或 tuple 索引）", tok)),
        }
    }

    fn split_numeric_field_path_float_span(&self, span: Span) -> Option<(Span, Span)> {
        let text = self.source.get(span.start..span.end)?;
        let dot = text.find('.')?;
        let left = &text[..dot];
        let right = &text[dot + 1..];
        if left.is_empty()
            || right.is_empty()
            || !left.chars().all(|ch| ch.is_ascii_digit())
            || !right.chars().all(|ch| ch.is_ascii_digit())
        {
            return None;
        }
        Some((
            Span::new(span.start, span.start + dot),
            Span::new(span.start + dot + 1, span.end),
        ))
    }

    // --------------------------------------------------------------
    // 插值字符串（§8.2：`${ expr }` hole 由子 parser 解析）
    // --------------------------------------------------------------

    fn parse_interpolated_string_expr(&mut self, tok: Token, raw: bool) -> PResult<Expr> {
        let (content_start, content_end) = if raw {
            // f""" ... """
            (
                tok.span.start.saturating_add(4),
                tok.span.end.saturating_sub(3),
            )
        } else {
            // f" ... "
            (
                tok.span.start.saturating_add(2),
                tok.span.end.saturating_sub(1),
            )
        };

        // 未闭合字符串已由 lexer 报告；这里防御性钳制，不重报。
        if content_start > content_end || content_end > self.source.len() {
            return Ok(Expr {
                id: self.nid(),
                span: tok.span,
                kind: ExprKind::InterpolatedString {
                    raw,
                    parts: Vec::new(),
                },
            });
        }

        let parts = self.split_interpolated_string_parts(content_start, content_end, raw)?;
        Ok(Expr {
            id: self.nid(),
            span: tok.span,
            kind: ExprKind::InterpolatedString { raw, parts },
        })
    }

    fn split_interpolated_string_parts(
        &mut self,
        content_start: usize,
        content_end: usize,
        raw: bool,
    ) -> PResult<Vec<StringPart>> {
        let bytes = self.source.as_bytes();
        let mut parts = Vec::new();

        let mut i = content_start;
        let mut text_start = content_start;

        while i < content_end {
            let b = bytes[i];

            // 普通 f-string 的 `\` 转义不触发插值分片。
            if !raw && b == b'\\' {
                i += 1;
                if i < content_end {
                    let Some(ch) = self.source.get(i..).and_then(|s| s.chars().next()) else {
                        break;
                    };
                    i += ch.len_utf8();
                }
                continue;
            }

            if b == b'$' && i + 1 < content_end && bytes[i + 1] == b'{' {
                if text_start < i {
                    parts.push(StringPart::Text(self.decode_f_string_text(
                        raw,
                        Span::new(text_start, i),
                    )));
                }

                let expr_start = i + 2;
                let Some(expr_close) = self.find_interpolation_close(expr_start, content_end)
                else {
                    return Err(self.err_unclosed(i + 1, "`}`"));
                };

                let expr = self.parse_expr_snippet(expr_start, expr_close)?;
                parts.push(StringPart::Expr(expr));

                i = expr_close + 1;
                text_start = i;
                continue;
            }

            // 其它字符：按 UTF-8 前进，保持 char boundary。
            if b < 0x80 {
                i += 1;
            } else {
                let Some(ch) = self.source.get(i..).and_then(|s| s.chars().next()) else {
                    break;
                };
                i += ch.len_utf8();
            }
        }

        if text_start < content_end {
            parts.push(StringPart::Text(
                self.decode_f_string_text(raw, Span::new(text_start, content_end)),
            ));
        }

        Ok(parts)
    }

    fn decode_f_string_text(&mut self, raw: bool, span: Span) -> String {
        let text = self.source.get(span.start..span.end).unwrap_or("");
        string_literal::parse_f_string_text_utf8(raw, text).unwrap_or_else(|_| text.to_string())
    }

    /// 在 f-string 内容区间内找与 `${` 匹配的 `}`（忽略嵌套字符串/注释/字符）。
    fn find_interpolation_close(&self, expr_start: usize, limit: usize) -> Option<usize> {
        let bytes = self.source.as_bytes();
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
            // string literal: "..." / """...""", 可带 `f` 前缀
            if bytes[i] == b'f' && i + 1 < limit && bytes[i + 1] == b'"' {
                i = skip_string_literal(self.source, i + 1, limit);
                continue;
            }
            if bytes[i] == b'"' {
                i = skip_string_literal(self.source, i, limit);
                continue;
            }
            if bytes[i] == b'\'' {
                i = skip_char_literal(self.source, i, limit);
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
                _ => i += 1,
            }
        }

        None
    }

    /// 解析 f-string hole 的源码切片：恰好一个完整表达式（§8.2）。
    fn parse_expr_snippet(&mut self, start: usize, end: usize) -> PResult<Expr> {
        let Some(snippet) = self.source.get(start..end) else {
            let tok = self.peek();
            return Err(self.err_expected_expr(tok));
        };

        let mut lexed = lexer::lex(snippet);
        for t in &mut lexed.tokens {
            t.span = Span::new(t.span.start + start, t.span.end + start);
        }

        let (merged, trailing_ok, sub_diags, sub_depth) = {
            let mut sub = self.sub_parser(lexed.tokens);
            sub.diagnostics = lexed.diagnostics;
            let result = sub.expr();
            let trailing_ok = result.is_ok() && sub.at_eof();
            if result.is_ok() && !trailing_ok {
                let tok = sub.peek();
                sub.err_expected("插值表达式结束（`}`）", tok);
            }
            (
                result.ok(),
                trailing_ok,
                std::mem::take(&mut sub.diagnostics),
                sub.depth,
            )
        };
        self.diagnostics.extend(sub_diags);
        self.depth = sub_depth;

        let Some(expr) = merged.filter(|_| trailing_ok) else {
            return Err(super::Abort);
        };
        Ok(expr)
    }

    fn decode_float(&mut self, tok: Token) -> ast::FloatLit {
        let text = self.token_text(tok);
        match float_literal::parse_float_literal_checked(text) {
            Ok(parsed) => ast::FloatLit {
                value: parsed.value,
                suffix: match parsed.suffix {
                    float_literal::FloatLiteralSuffix::Float64 => None,
                    float_literal::FloatLiteralSuffix::Float32 => Some(ast::FloatSuffix::F32),
                },
                span: tok.span,
            },
            Err(_) => {
                // lexer 已报告非法字面量；best-effort 兜底，不重报。
                let suffix = if text.ends_with("f32") || text.ends_with('f') {
                    Some(ast::FloatSuffix::F32)
                } else {
                    None
                };
                ast::FloatLit {
                    value: 0.0,
                    suffix,
                    span: tok.span,
                }
            }
        }
    }
}

// ------------------------------------------------------------------
// 辅助
// ------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BraceGroupKind {
    StructLit,
    Lambda,
}

#[derive(Debug, Clone, Copy)]
struct InfixOp {
    l_bp: u8,
    r_bp: u8,
    kind: InfixOpKind,
}

#[derive(Debug, Clone, Copy)]
enum InfixOpKind {
    Binary(BinaryOp),
    ContextualInfix,
    Is,
    NotIs,
    As,
    AsSafe,
}

fn binary_binding_power(sym: Symbol) -> Option<(u8, u8, BinaryOp)> {
    // 优先级表见 grammar §8.1；左结合 (p, p+1)，右结合 (p, p)。
    match sym {
        Symbol::Star => Some((11, 12, BinaryOp::Mul)),
        Symbol::Slash => Some((11, 12, BinaryOp::Div)),
        Symbol::Percent => Some((11, 12, BinaryOp::Rem)),

        Symbol::Plus => Some((10, 11, BinaryOp::Add)),
        Symbol::Minus => Some((10, 11, BinaryOp::Sub)),

        Symbol::LtLt => Some((9, 10, BinaryOp::Shl)),
        Symbol::GtGt => Some((9, 10, BinaryOp::Shr)),

        // `..` 与比较运算同级（§8.1 normative，刻意不同于 Kotlin）。
        Symbol::DotDot => Some((8, 9, BinaryOp::Range)),
        Symbol::Lt => Some((8, 9, BinaryOp::Lt)),
        Symbol::LtEq => Some((8, 9, BinaryOp::Le)),
        Symbol::Gt => Some((8, 9, BinaryOp::Gt)),
        Symbol::GtEq => Some((8, 9, BinaryOp::Ge)),

        Symbol::EqEq => Some((7, 8, BinaryOp::Eq)),
        Symbol::BangEq => Some((7, 8, BinaryOp::Ne)),

        Symbol::And => Some((6, 7, BinaryOp::BitAnd)),
        Symbol::Caret => Some((5, 6, BinaryOp::BitXor)),
        Symbol::Or => Some((4, 5, BinaryOp::BitOr)),

        Symbol::AndAnd => Some((3, 4, BinaryOp::LogAnd)),
        Symbol::OrOr => Some((2, 3, BinaryOp::LogOr)),

        // Elvis：唯一右结合二元运算。
        Symbol::Elvis => Some((1, 1, BinaryOp::Elvis)),

        _ => None,
    }
}

fn member_name_end(member: &MemberName) -> usize {
    match member {
        MemberName::Named(ident) => ident.span.end,
        MemberName::TupleIndex { span, .. } => span.end,
    }
}

/// `T::class` 的 receiver 必须能还原成类型路径 `Ident(.Ident)*`。
fn type_path_from_expr(expr: &Expr) -> Option<TypePath> {
    let mut segments: Vec<Ident> = Vec::new();
    if !collect_type_path_segments(expr, &mut segments) || segments.is_empty() {
        return None;
    }
    Some(TypePath {
        segments,
        span: expr.span,
    })
}

fn collect_type_path_segments(expr: &Expr, out: &mut Vec<Ident>) -> bool {
    match &expr.kind {
        ExprKind::Ident(ident) => {
            out.push(*ident);
            true
        }
        ExprKind::MemberAccess { receiver, member } => {
            if !collect_type_path_segments(receiver, out) {
                return false;
            }
            match member {
                MemberName::Named(ident) => {
                    out.push(*ident);
                    true
                }
                MemberName::TupleIndex { .. } => false,
            }
        }
        _ => false,
    }
}

fn skip_string_literal(source: &str, quote_start: usize, limit: usize) -> usize {
    let bytes = source.as_bytes();

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

    let mut i = quote_start + 1;
    while i < limit {
        match bytes[i] {
            b'\\' => i = (i + 2).min(limit),
            b'"' => return i + 1,
            b'\n' => return limit,
            _ => i += 1,
        }
    }
    limit
}

fn skip_char_literal(source: &str, quote_start: usize, limit: usize) -> usize {
    let bytes = source.as_bytes();
    let mut i = quote_start + 1;
    let mut escaped = false;
    while i < limit {
        match bytes[i] {
            b'\n' => return limit,
            b'\'' if !escaped => return i + 1,
            b'\\' if !escaped => {
                escaped = true;
                i += 1;
            }
            _ => {
                escaped = false;
                i += 1;
            }
        }
    }
    limit
}
