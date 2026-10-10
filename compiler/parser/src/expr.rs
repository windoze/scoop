//! Expression parsing: precedence climbing over the M6 operator set.
//!
//! Precedence, low to high: `?:` (right-associative) < `||` < `&&` <
//! `== != === !==` < `< <= > >= is !is as as?` < `+ -` < `* /` < unary
//! `- !` < postfix `.name` / `.name(args)` / `._n` / `?.name` / `!!` /
//! `[index]` < atoms. All other binary operators are left-associative.

use scoop_ast::{
    BinOp, CallArgument, CallArgumentName, CallExpr, Diagnostic, Expr, FieldAccess, FieldSelector,
    FieldUpdate, Ident, InfixTarget, Navigation, NonEmptyVec, PlaceExpr, Span, SpreadSyntax, UnOp,
    UpdateNotation, UpdateOp,
};

use crate::lexer::TokenKind;
use crate::parser::Parser;

mod callables;
mod context;
mod interpolation;
mod places;
mod postfix;
mod primary;
mod qualifiers;
mod trailing;

/// Precedence tier of the comparison operators — shared by the type
/// operators `is` / `!is` / `as` / `as?` (M6), which are handled outside
/// `binary_op` because their right-hand side is a type, not an
/// expression.
const OR_PRECEDENCE: u8 = 1;
const AND_PRECEDENCE: u8 = 2;
const EQUALITY_PRECEDENCE: u8 = 3;
const COMPARISON_PRECEDENCE: u8 = 4;
const MEMBERSHIP_PRECEDENCE: u8 = 5;
const ELVIS_PRECEDENCE: u8 = 6;
const INFIX_PRECEDENCE: u8 = 7;
const RANGE_PRECEDENCE: u8 = 8;
const ADDITIVE_PRECEDENCE: u8 = 9;
const MULTIPLICATIVE_PRECEDENCE: u8 = 10;
const CAST_PRECEDENCE: u8 = 11;

/// Maps an infix operator token to its AST operator and precedence level
/// (higher binds tighter); `None` for non-operator tokens.
fn binary_op(kind: &TokenKind) -> Option<(BinOp, u8)> {
    let (op, precedence) = match kind {
        TokenKind::PipePipe => (BinOp::Or, OR_PRECEDENCE),
        TokenKind::AmpAmp => (BinOp::And, AND_PRECEDENCE),
        TokenKind::EqualEqual => (BinOp::Eq, EQUALITY_PRECEDENCE),
        TokenKind::BangEqual => (BinOp::Ne, EQUALITY_PRECEDENCE),
        TokenKind::EqualEqualEqual => (BinOp::RefEq, EQUALITY_PRECEDENCE),
        TokenKind::BangEqualEqual => (BinOp::RefNe, EQUALITY_PRECEDENCE),
        TokenKind::Less => (BinOp::Lt, COMPARISON_PRECEDENCE),
        TokenKind::LessEqual => (BinOp::Le, COMPARISON_PRECEDENCE),
        TokenKind::Greater => (BinOp::Gt, COMPARISON_PRECEDENCE),
        TokenKind::GreaterEqual => (BinOp::Ge, COMPARISON_PRECEDENCE),
        TokenKind::In => (BinOp::Contains, MEMBERSHIP_PRECEDENCE),
        TokenKind::BangIn => (BinOp::NotContains, MEMBERSHIP_PRECEDENCE),
        TokenKind::DotDot => (BinOp::RangeTo, RANGE_PRECEDENCE),
        TokenKind::DotDotLess => (BinOp::RangeUntil, RANGE_PRECEDENCE),
        TokenKind::Plus => (BinOp::Add, ADDITIVE_PRECEDENCE),
        TokenKind::Minus => (BinOp::Sub, ADDITIVE_PRECEDENCE),
        TokenKind::Star => (BinOp::Mul, MULTIPLICATIVE_PRECEDENCE),
        TokenKind::Slash => (BinOp::Div, MULTIPLICATIVE_PRECEDENCE),
        TokenKind::Percent => (BinOp::Rem, MULTIPLICATIVE_PRECEDENCE),
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
            if self.at_arm_body_newline() {
                break;
            }
            if !self.peek().newline_before
                && matches!(self.peek().kind, TokenKind::Break | TokenKind::Continue)
            {
                let token = self.peek();
                let name = if matches!(&token.kind, TokenKind::Break) {
                    "break"
                } else {
                    "continue"
                };
                return Err(loop_jump_expression_diagnostic(name, token.span));
            }
            // Elvis is right-associative; every other binary/infix source
            // form in this table is left-associative.
            if matches!(self.peek().kind, TokenKind::QuestionColon) {
                if ELVIS_PRECEDENCE < min_precedence {
                    break;
                }
                self.bump();
                let rhs = self.parse_binary(ELVIS_PRECEDENCE)?;
                lhs = Expr::Elvis {
                    span: Span::new(lhs.span().start, rhs.span().end),
                    lhs: Box::new(lhs),
                    rhs: Box::new(rhs),
                };
                continue;
            }
            let Some((op, precedence)) = binary_op(&self.peek().kind) else {
                // `is` / `!is` and `as` / `as?` take a type on the right.
                if matches!(self.peek().kind, TokenKind::Is) || self.at_bang_is() {
                    if MEMBERSHIP_PRECEDENCE < min_precedence {
                        break;
                    }
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
                    if CAST_PRECEDENCE < min_precedence {
                        break;
                    }
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
                if let TokenKind::Ident(_) = &self.peek().kind {
                    if INFIX_PRECEDENCE < min_precedence || self.peek().newline_before {
                        break;
                    }
                    let name = self.expect_ident("infix function name")?;
                    let rhs = self.parse_binary(INFIX_PRECEDENCE + 1)?;
                    lhs = Expr::InfixCall {
                        span: Span::new(lhs.span().start, rhs.span().end),
                        lhs: Box::new(lhs),
                        target: InfixTarget::Named(name),
                        rhs: Box::new(rhs),
                    };
                    continue;
                }
                if INFIX_PRECEDENCE >= min_precedence && self.starts_infix_invoke_rhs() {
                    let rhs = self.parse_binary(INFIX_PRECEDENCE + 1)?;
                    lhs = Expr::InfixCall {
                        span: Span::new(lhs.span().start, rhs.span().end),
                        lhs: Box::new(lhs),
                        target: InfixTarget::Invoke,
                        rhs: Box::new(rhs),
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

    /// Property-like infix invoke has no name token between its two values.
    /// Identifier-led right operands remain reserved for named infix syntax;
    /// callers can parenthesize such an operand to make the boundary explicit.
    fn starts_infix_invoke_rhs(&self) -> bool {
        !self.peek().newline_before
            && matches!(
                self.peek().kind,
                TokenKind::Char(_)
                    | TokenKind::Str(_)
                    | TokenKind::FStringStart
                    | TokenKind::Int(_)
                    | TokenKind::True
                    | TokenKind::False
                    | TokenKind::This
                    | TokenKind::LParen
                    | TokenKind::LBracket
                    | TokenKind::LBrace
                    | TokenKind::Fun
                    | TokenKind::Suspend
            )
    }

    fn parse_unary(&mut self) -> Result<Expr, Diagnostic> {
        let token = self.peek().clone();
        let op = match token.kind {
            TokenKind::Plus => UnOp::Plus,
            TokenKind::Minus => UnOp::Neg,
            TokenKind::Bang => UnOp::Not,
            TokenKind::PlusPlus | TokenKind::MinusMinus => {
                self.pos += 1;
                let operand = self.parse_unary()?;
                let end = operand.span().end;
                let place = Self::expr_into_place(operand, "update operand")?;
                return Ok(Expr::Update {
                    place,
                    op: if matches!(token.kind, TokenKind::PlusPlus) {
                        UpdateOp::Increment
                    } else {
                        UpdateOp::Decrement
                    },
                    notation: UpdateNotation::Prefix,
                    span: Span::new(token.span.start, end),
                });
            }
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
}

pub(crate) fn loop_jump_expression_diagnostic(name: &str, span: Span) -> Diagnostic {
    Diagnostic::at(
        span,
        format!("`{name}` jump expressions are not supported in M22"),
    )
}
