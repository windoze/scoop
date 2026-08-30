//! Expression parsing: precedence climbing over the M6 operator set.
//!
//! Precedence, low to high: `?:` (right-associative) < `||` < `&&` <
//! `== != === !==` < `< <= > >= is !is as as?` < `+ -` < `* /` < unary
//! `- !` < postfix `.name` / `.name(args)` / `._n` / `?.name` / `!!` /
//! `[index]` < atoms. All other binary operators are left-associative.

use scoop_ast::{
    BinOp, CallExpr, Diagnostic, Expr, FieldAccess, FieldSelector, Ident, LambdaParam, Param, Span,
    UnOp,
};

use crate::lexer::TokenKind;
use crate::parser::Parser;

/// Precedence tier of the comparison operators — shared by the type
/// operators `is` / `!is` / `as` / `as?` (M6), which are handled outside
/// `binary_op` because their right-hand side is a type, not an
/// expression.
const COMPARISON_PRECEDENCE: u8 = 4;

/// Maps an infix operator token to its AST operator and precedence level
/// (higher binds tighter); `None` for non-operator tokens.
fn binary_op(kind: &TokenKind) -> Option<(BinOp, u8)> {
    let (op, precedence) = match kind {
        TokenKind::PipePipe => (BinOp::Or, 1),
        TokenKind::AmpAmp => (BinOp::And, 2),
        TokenKind::EqualEqual => (BinOp::Eq, 3),
        TokenKind::BangEqual => (BinOp::Ne, 3),
        TokenKind::EqualEqualEqual => (BinOp::RefEq, 3),
        TokenKind::BangEqualEqual => (BinOp::RefNe, 3),
        TokenKind::Less => (BinOp::Lt, COMPARISON_PRECEDENCE),
        TokenKind::LessEqual => (BinOp::Le, COMPARISON_PRECEDENCE),
        TokenKind::Greater => (BinOp::Gt, COMPARISON_PRECEDENCE),
        TokenKind::GreaterEqual => (BinOp::Ge, COMPARISON_PRECEDENCE),
        TokenKind::Plus => (BinOp::Add, 5),
        TokenKind::Minus => (BinOp::Sub, 5),
        TokenKind::Star => (BinOp::Mul, 6),
        TokenKind::Slash => (BinOp::Div, 6),
        _ => return None,
    };
    Some((op, precedence))
}

/// `._<n>` tuple selectors lex as identifiers (`_1`, `_2`, ...). Returns
/// `Some(n)` when `text` is `_` followed by digits only.
fn tuple_index(text: &str) -> Option<u32> {
    let digits = text.strip_prefix('_')?;
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}

impl Parser {
    pub(crate) fn parse_expr(&mut self) -> Result<Expr, Diagnostic> {
        self.parse_binary(0)
    }

    fn parse_binary(&mut self, min_precedence: u8) -> Result<Expr, Diagnostic> {
        let mut lhs = self.parse_unary()?;
        loop {
            // `..` is the range operator in expression position (the
            // pattern rest marker never reaches here); out of the M5
            // subset, with a dedicated diagnostic.
            if matches!(self.peek().kind, TokenKind::DotDot) {
                return Err(Diagnostic::at(
                    self.peek().span,
                    "ranges are not supported yet (milestone M5)",
                ));
            }
            // `?:` sits one level below `||` (level 0) and is
            // right-associative, so it is handled outside `binary_op`.
            if matches!(self.peek().kind, TokenKind::QuestionColon) {
                if min_precedence > 0 {
                    break;
                }
                self.bump();
                let rhs = self.parse_binary(0)?;
                lhs = Expr::Elvis {
                    span: Span::new(lhs.span().start, rhs.span().end),
                    lhs: Box::new(lhs),
                    rhs: Box::new(rhs),
                };
                continue;
            }
            let Some((op, precedence)) = binary_op(&self.peek().kind) else {
                // `is` / `!is` / `as` / `as?` take a type on the right, so
                // they live outside `binary_op` (same tier, left
                // associative).
                if COMPARISON_PRECEDENCE < min_precedence {
                    break;
                }
                if matches!(self.peek().kind, TokenKind::Is) || self.at_bang_is() {
                    let negated = self.at_bang_is();
                    self.bump(); // `is` or `!`
                    if negated {
                        self.bump(); // `is`
                    }
                    let ty = self.parse_type_ref()?;
                    lhs = Expr::Is {
                        span: Span::new(lhs.span().start, ty.span.end),
                        operand: Box::new(lhs),
                        ty,
                        negated,
                    };
                    continue;
                }
                if matches!(self.peek().kind, TokenKind::As) {
                    let as_token = self.bump();
                    // `as?` (safe cast) is `as` directly followed by `?`.
                    let optional = matches!(self.peek().kind, TokenKind::Question)
                        && self.peek().span.start == as_token.span.end;
                    if optional {
                        self.bump(); // `?`
                    }
                    let ty = self.parse_type_ref()?;
                    lhs = Expr::Cast {
                        span: Span::new(lhs.span().start, ty.span.end),
                        operand: Box::new(lhs),
                        ty,
                        optional,
                    };
                    continue;
                }
                break;
            };
            if precedence < min_precedence {
                break;
            }
            self.bump();
            let rhs = self.parse_binary(precedence + 1)?;
            lhs = Expr::Binary {
                op,
                span: Span::new(lhs.span().start, rhs.span().end),
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            };
        }
        Ok(lhs)
    }

    fn parse_unary(&mut self) -> Result<Expr, Diagnostic> {
        let token = self.peek().clone();
        let op = match token.kind {
            TokenKind::Minus => UnOp::Neg,
            TokenKind::Bang => UnOp::Not,
            _ => return self.parse_postfix(),
        };
        self.pos += 1;
        let operand = self.parse_unary()?;
        Ok(Expr::Unary {
            op,
            span: Span::new(token.span.start, operand.span().end),
            operand: Box::new(operand),
        })
    }

    /// Postfix operators share the highest precedence tier and chain left
    /// to right: `.name` / `._n`, `?.name`, `!!`, and `[index]`.
    fn parse_postfix(&mut self) -> Result<Expr, Diagnostic> {
        let mut receiver = self.parse_atom()?;
        loop {
            match self.peek().kind {
                TokenKind::Dot => {
                    self.bump();
                    receiver = self.parse_field_access(receiver, false)?;
                }
                TokenKind::QuestionDot => {
                    self.bump();
                    receiver = self.parse_field_access(receiver, true)?;
                }
                TokenKind::DoubleColon => {
                    receiver = self.parse_callable_reference(Some(receiver))?;
                }
                TokenKind::LParen if !self.peek().newline_before => {
                    let start = receiver.span().start;
                    let (args, end) = self.parse_args()?;
                    receiver = Expr::Invoke {
                        callee: Box::new(receiver),
                        args,
                        span: Span::new(start, end),
                    };
                }
                // A `[` immediately after the receiver (their spans touch)
                // is subscript postfix; otherwise it starts a new array
                // literal expression — e.g. a `[...]` statement on the
                // next line (DESIGN.md section 2.1).
                TokenKind::LBracket if self.peek().span.start == receiver.span().end => {
                    receiver = self.parse_index(receiver)?;
                }
                // `a!!` lexes as two adjacent `Bang` tokens — a single
                // `!!` token would break the double negation `!!flag`,
                // which is valid prefix syntax since M1.
                TokenKind::Bang if self.at_null_assert() => {
                    self.bump(); // first `!`
                    let second = self.bump(); // second `!`
                    receiver = Expr::NullAssert {
                        span: Span::new(receiver.span().start, second.span.end),
                        operand: Box::new(receiver),
                    };
                }
                _ => break,
            }
        }
        Ok(receiver)
    }

    /// `receiver[index]` — the `[` is the current token. A `:` after the
    /// index expression is a slice, out of the M5 subset.
    fn parse_index(&mut self, receiver: Expr) -> Result<Expr, Diagnostic> {
        self.bump(); // `[`
        let index = self.parse_expr()?;
        if matches!(self.peek().kind, TokenKind::Colon) {
            return Err(Diagnostic::at(
                self.peek().span,
                "array slices are not supported yet (milestone M5)",
            ));
        }
        let close = self.expect("`]`", |k| matches!(k, TokenKind::RBracket))?;
        Ok(Expr::Index {
            span: Span::new(receiver.span().start, close.span.end),
            receiver: Box::new(receiver),
            index: Box::new(index),
        })
    }

    /// True when the current `!` is immediately followed by another `!`
    /// with no trivia in between (their spans touch).
    fn at_null_assert(&self) -> bool {
        let Some(next) = self.tokens.get(self.pos + 1) else {
            return false;
        };
        matches!(next.kind, TokenKind::Bang) && next.span.start == self.peek().span.end
    }

    /// True when the current `!` is immediately followed by `is` with no
    /// trivia in between — the `!is` operator (two tokens, like `!!`).
    fn at_bang_is(&self) -> bool {
        let Some(next) = self.tokens.get(self.pos + 1) else {
            return false;
        };
        matches!(self.peek().kind, TokenKind::Bang)
            && matches!(next.kind, TokenKind::Is)
            && next.span.start == self.peek().span.end
    }

    /// `.name` / `.name(args)` / `._n` (or `?.name` when `safe`). The dot
    /// token is already consumed. A name directly followed by `(` is a
    /// method call; otherwise the selector is a field. `?.` accepts only
    /// named selectors and, in M6, no method calls (DESIGN.md section 6).
    fn parse_field_access(&mut self, receiver: Expr, safe: bool) -> Result<Expr, Diagnostic> {
        let token = self.peek().clone();
        let TokenKind::Ident(text) = token.kind else {
            return self.unexpected("field name or tuple index");
        };
        if let Some(index) = tuple_index(&text) {
            if safe {
                return self.unexpected("field name");
            }
            self.pos += 1;
            return Ok(Expr::FieldAccess(FieldAccess {
                span: Span::new(receiver.span().start, token.span.end),
                receiver: Box::new(receiver),
                selector: FieldSelector::Index(index, token.span),
                safe,
            }));
        }
        self.pos += 1;
        let name = Ident {
            text,
            span: token.span,
        };
        if matches!(self.peek().kind, TokenKind::LParen) {
            if safe {
                return Err(Diagnostic::at(
                    self.peek().span,
                    "method calls with `?.` are not supported yet (milestone M6)",
                ));
            }
            let (args, end) = self.parse_args()?;
            return Ok(Expr::MethodCall {
                span: Span::new(receiver.span().start, end),
                receiver: Box::new(receiver),
                name,
                args,
            });
        }
        Ok(Expr::FieldAccess(FieldAccess {
            span: Span::new(receiver.span().start, token.span.end),
            receiver: Box::new(receiver),
            selector: FieldSelector::Name(name),
            safe,
        }))
    }

    fn parse_atom(&mut self) -> Result<Expr, Diagnostic> {
        let token = self.peek().clone();
        match token.kind {
            TokenKind::Suspend => Err(Diagnostic::at(
                token.span,
                "suspend lambdas are not supported in M10",
            )),
            TokenKind::Fun => self.parse_anonymous_function(false),
            TokenKind::DoubleColon => self.parse_callable_reference(None),
            TokenKind::LBrace => self.parse_lambda(false),
            TokenKind::Str(value) => {
                self.pos += 1;
                Ok(Expr::StringLiteral {
                    value,
                    span: token.span,
                })
            }
            TokenKind::Int(value) => {
                self.pos += 1;
                Ok(Expr::IntLiteral {
                    value,
                    span: token.span,
                })
            }
            TokenKind::True => {
                self.pos += 1;
                Ok(Expr::BoolLiteral {
                    value: true,
                    span: token.span,
                })
            }
            TokenKind::False => {
                self.pos += 1;
                Ok(Expr::BoolLiteral {
                    value: false,
                    span: token.span,
                })
            }
            TokenKind::If => self.parse_if_expression(),
            TokenKind::When => self.parse_when_expression(),
            TokenKind::This => {
                self.pos += 1;
                Ok(Expr::This { span: token.span })
            }
            TokenKind::Ident(ref text) if text == "try" => self.parse_try_expression(),
            TokenKind::Ident(text) => {
                self.pos += 1;
                // `Unit` is an ordinary identifier; in expression position
                // it denotes the unit value (spec section 4.3).
                if text == "Unit" {
                    return Ok(Expr::UnitLiteral { span: token.span });
                }
                if text == "super" {
                    return Err(Diagnostic::at(
                        token.span,
                        "`super` calls are not supported yet (milestone M6)",
                    ));
                }
                let ident = Ident {
                    text,
                    span: token.span,
                };
                // `Name(args)` — a call; struct/enum construction shares
                // this syntax, hir-lower tells them apart. A dotted path
                // (`E.V(args)`, `p.m(args)`) is NOT collapsed here: the
                // postfix loop turns it into a method call, and hir-lower
                // resolves enum variant construction from that shape.
                if matches!(self.peek().kind, TokenKind::LParen) {
                    return self.parse_call(ident);
                }
                Ok(Expr::Var(ident))
            }
            TokenKind::LParen => self.parse_paren_expr(),
            TokenKind::LBracket => self.parse_array_literal(),
            _ => self.unexpected("expression"),
        }
    }

    fn parse_callable_reference(&mut self, receiver: Option<Expr>) -> Result<Expr, Diagnostic> {
        let start = receiver
            .as_ref()
            .map_or_else(|| self.peek().span.start, |expr| expr.span().start);
        self.expect("`::`", |kind| matches!(kind, TokenKind::DoubleColon))?;
        let name = self.expect_ident("callable name after `::`")?;
        let span = Span::new(start, name.span.end);
        Ok(Expr::CallableReference {
            id: self.alloc_callable_reference_id(),
            receiver: receiver.map(Box::new),
            name,
            span,
        })
    }

    fn parse_anonymous_function(&mut self, is_suspend: bool) -> Result<Expr, Diagnostic> {
        let keyword = self.expect("`fun`", |kind| matches!(kind, TokenKind::Fun))?;
        self.expect("`(`", |kind| matches!(kind, TokenKind::LParen))?;
        let mut params = Vec::new();
        if !matches!(self.peek().kind, TokenKind::RParen) {
            loop {
                let name = self.expect_ident("parameter name")?;
                self.expect("`:`", |kind| matches!(kind, TokenKind::Colon))?;
                let ty = self.parse_type_ref()?;
                params.push(Param {
                    span: Span::new(name.span.start, ty.span.end),
                    name,
                    ty,
                });
                if matches!(self.peek().kind, TokenKind::Comma) {
                    self.bump();
                } else {
                    break;
                }
            }
        }
        self.expect("`)`", |kind| matches!(kind, TokenKind::RParen))?;
        let return_ty = if matches!(self.peek().kind, TokenKind::Colon) {
            self.bump();
            Some(self.parse_type_ref()?)
        } else {
            None
        };
        let body = self.parse_block()?;
        Ok(Expr::AnonymousFunction {
            id: self.alloc_anonymous_function_id(),
            is_suspend,
            params,
            return_ty,
            span: Span::new(keyword.span.start, body.span.end),
            body,
        })
    }

    /// A lambda body. Parameter parsing is speculative only until a `->`
    /// is found; without it the same tokens are parsed as the first body
    /// statement, which preserves `{ value }` as a zero/implicit-parameter
    /// lambda rather than treating `value` as a declaration.
    fn parse_lambda(&mut self, is_suspend: bool) -> Result<Expr, Diagnostic> {
        let open = self.expect("`{`", |kind| matches!(kind, TokenKind::LBrace))?;
        let header_start = self.pos;
        let diagnostics_start = self.diagnostics.len();
        let parameters = if matches!(self.peek().kind, TokenKind::Arrow) {
            self.bump();
            Some(Vec::new())
        } else {
            let parsed = self.try_parse_lambda_parameters();
            match parsed {
                Some(parameters) => Some(parameters),
                None => {
                    self.pos = header_start;
                    self.diagnostics.truncate(diagnostics_start);
                    None
                }
            }
        };
        let body = self.parse_block_after_open(open.clone())?;
        let span = Span::new(open.span.start, body.span.end);
        Ok(Expr::Lambda {
            id: self.alloc_lambda_id(),
            is_suspend,
            parameters,
            body,
            span,
        })
    }

    fn try_parse_lambda_parameters(&mut self) -> Option<Vec<LambdaParam>> {
        let start = self.pos;
        let mut parameters = Vec::new();
        loop {
            let target = self.parse_pattern().ok()?;
            let target_span = crate::pattern::pattern_span(&target);
            let ty = if matches!(self.peek().kind, TokenKind::Colon) {
                self.bump();
                Some(self.parse_type_ref().ok()?)
            } else {
                None
            };
            let end = ty.as_ref().map_or(target_span.end, |ty| ty.span.end);
            parameters.push(LambdaParam {
                target,
                ty,
                span: Span::new(target_span.start, end),
            });
            if matches!(self.peek().kind, TokenKind::Comma) {
                self.bump();
                continue;
            }
            if matches!(self.peek().kind, TokenKind::Arrow) {
                self.bump();
                return Some(parameters);
            }
            self.pos = start;
            return None;
        }
    }

    /// `[e1, e2, ...]` — an array literal (spec 10.2). The empty `[]`
    /// parses too; HIR rejects it when there is no expected type.
    fn parse_array_literal(&mut self) -> Result<Expr, Diagnostic> {
        let open = self.bump(); // `[`
        let mut elements = Vec::new();
        if !matches!(self.peek().kind, TokenKind::RBracket) {
            loop {
                elements.push(self.parse_expr()?);
                if matches!(self.peek().kind, TokenKind::Comma) {
                    self.bump();
                } else {
                    break;
                }
            }
        }
        let close = self.expect("`]`", |k| matches!(k, TokenKind::RBracket))?;
        Ok(Expr::ArrayLiteral {
            elements,
            span: Span::new(open.span.start, close.span.end),
        })
    }

    /// `callee(args...)` — `callee` is already consumed. Struct
    /// construction shares this syntax in M2 (`Point(1, 2)`); hir-lower
    /// tells function calls and struct constructions apart.
    fn parse_call(&mut self, callee: Ident) -> Result<Expr, Diagnostic> {
        let (args, end) = self.parse_args()?;
        let span = Span::new(callee.span.start, end);
        Ok(Expr::Call(CallExpr { callee, args, span }))
    }

    /// `(arg, ...)` — the `(` is the current token. Shared by calls,
    /// method calls, and base-class constructor delegation. Returns the
    /// arguments and the closing paren's end offset.
    pub(crate) fn parse_args(&mut self) -> Result<(Vec<Expr>, u32), Diagnostic> {
        self.expect("`(`", |k| matches!(k, TokenKind::LParen))?;
        let mut args = Vec::new();
        if !matches!(self.peek().kind, TokenKind::RParen) {
            loop {
                args.push(self.parse_expr()?);
                if matches!(self.peek().kind, TokenKind::Comma) {
                    self.bump();
                } else {
                    break;
                }
            }
        }
        let close = self.expect("`)`", |k| matches!(k, TokenKind::RParen))?;
        Ok((args, close.span.end))
    }

    /// `( ... )` disambiguation (spec section 4.3): `()` is the unit
    /// literal, `(e)` is just `e` (no node), `(e,)` a 1-tuple, and
    /// `(e1, e2, ...)` a tuple literal.
    fn parse_paren_expr(&mut self) -> Result<Expr, Diagnostic> {
        let open = self.bump(); // `(`
        if matches!(self.peek().kind, TokenKind::RParen) {
            let close = self.bump();
            return Ok(Expr::UnitLiteral {
                span: Span::new(open.span.start, close.span.end),
            });
        }
        let first = self.parse_expr()?;
        if !matches!(self.peek().kind, TokenKind::Comma) {
            self.expect("`)`", |k| matches!(k, TokenKind::RParen))?;
            return Ok(first);
        }
        let mut elements = vec![first];
        while matches!(self.peek().kind, TokenKind::Comma) {
            self.bump();
            if matches!(self.peek().kind, TokenKind::RParen) {
                break; // trailing comma: `(e,)` / `(e1, e2,)`
            }
            elements.push(self.parse_expr()?);
        }
        let close = self.expect("`)`", |k| matches!(k, TokenKind::RParen))?;
        Ok(Expr::TupleLiteral {
            elements,
            span: Span::new(open.span.start, close.span.end),
        })
    }
}
