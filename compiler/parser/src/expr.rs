//! Expression parsing: precedence climbing over the M6 operator set.
//!
//! Precedence, low to high: `?:` (right-associative) < `||` < `&&` <
//! `== != === !==` < `< <= > >= is !is as as?` < `+ -` < `* /` < unary
//! `- !` < postfix `.name` / `.name(args)` / `._n` / `?.name` / `!!` /
//! `[index]` < atoms. All other binary operators are left-associative.

use scoop_ast::{BinOp, CallExpr, Diagnostic, Expr, FieldAccess, FieldSelector, Ident, Span, UnOp};

use crate::lexer::TokenKind;
use crate::parser::Parser;

mod callables;
mod primary;

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
                TokenKind::LBrace
                    if !self.peek().newline_before
                        && matches!(
                            &receiver,
                            Expr::Call(_) | Expr::MethodCall { .. } | Expr::Invoke { .. }
                        ) =>
                {
                    let lambda = self.parse_lambda(false, None)?;
                    let end = lambda.span().end;
                    match &mut receiver {
                        Expr::Call(call) => {
                            call.args.push(lambda);
                            call.span.end = end;
                        }
                        Expr::MethodCall { args, span, .. } | Expr::Invoke { args, span, .. } => {
                            args.push(lambda);
                            span.end = end;
                        }
                        _ => unreachable!("the trailing-lambda guard accepts only calls"),
                    }
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
        let type_args = self.parse_explicit_call_type_args();
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
                type_args,
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
}
