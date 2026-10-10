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
mod primary;
mod qualifiers;

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

    /// Postfix operators share the highest precedence tier and chain left
    /// to right: `.name` / `._n`, `?.name`, `!!`, and `[index]`.
    pub(crate) fn parse_postfix(&mut self) -> Result<Expr, Diagnostic> {
        let mut receiver = self.parse_atom()?;
        loop {
            match self.peek().kind {
                TokenKind::Dot => {
                    self.bump();
                    receiver = if matches!(self.peek().kind, TokenKind::LBrace) {
                        self.parse_copy_update(receiver)?
                    } else {
                        self.parse_field_access(receiver, false)?
                    };
                }
                TokenKind::QuestionDot => {
                    let navigation = self.bump();
                    if matches!(self.peek().kind, TokenKind::LBrace) {
                        return Err(Diagnostic::at(
                            navigation.span,
                            "copy update does not support safe navigation; unwrap the Option with `when` first",
                        ));
                    }
                    receiver = self.parse_field_access(receiver, true)?;
                }
                TokenKind::DoubleColon => {
                    receiver = self.parse_callable_reference(Some(receiver))?;
                }
                TokenKind::LParen
                    if !self.peek().newline_before && matches!(receiver, Expr::This { .. }) =>
                {
                    return Err(Diagnostic::at(
                        receiver.span(),
                        "`this(...)` is only valid in a constructor delegation clause",
                    ));
                }
                TokenKind::LParen if !self.peek().newline_before => {
                    let start = receiver.span().start;
                    let (args, end) = self.parse_args()?;
                    receiver = Expr::Invoke {
                        callee: Box::new(receiver),
                        type_args: Vec::new(),
                        args,
                        span: Span::new(start, end),
                    };
                }
                TokenKind::LBrace
                    if !self.peek().newline_before
                        && matches!(
                            &receiver,
                            Expr::Call(_)
                                | Expr::MethodCall { .. }
                                | Expr::SuperMethodCall { .. }
                                | Expr::QualifiedInterfaceSuperMethodCall { .. }
                                | Expr::Invoke { .. }
                        ) =>
                {
                    let lambda = self.parse_lambda(false, None)?;
                    let end = lambda.span().end;
                    match &mut receiver {
                        Expr::Call(call) => {
                            call.args.push(scoop_ast::CallArgument::positional(lambda));
                            call.span.end = end;
                        }
                        Expr::MethodCall { args, span, .. }
                        | Expr::SuperMethodCall { args, span, .. }
                        | Expr::QualifiedInterfaceSuperMethodCall { args, span, .. }
                        | Expr::Invoke { args, span, .. } => {
                            args.push(scoop_ast::CallArgument::positional(lambda));
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
                TokenKind::Less if !self.peek().newline_before => {
                    if let Some(qualifier) = self.parse_applied_qualifier(&receiver)? {
                        receiver = Expr::TypeQualifier(qualifier);
                        continue;
                    }
                    let type_args = self.parse_explicit_call_type_args()?;
                    if type_args.is_empty() {
                        break;
                    }
                    let start = receiver.span().start;
                    let (args, end) = self.parse_args()?;
                    receiver = Expr::Invoke {
                        callee: Box::new(receiver),
                        type_args,
                        args,
                        span: Span::new(start, end),
                    };
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
                TokenKind::PlusPlus | TokenKind::MinusMinus => {
                    let token = self.bump();
                    let start = receiver.span().start;
                    let place = Self::expr_into_place(receiver, "update operand")?;
                    receiver = Expr::Update {
                        place,
                        op: if matches!(token.kind, TokenKind::PlusPlus) {
                            UpdateOp::Increment
                        } else {
                            UpdateOp::Decrement
                        },
                        notation: UpdateNotation::Postfix,
                        span: Span::new(start, token.span.end),
                    };
                }
                _ => break,
            }
        }
        Ok(receiver)
    }

    /// `receiver.{ field: value, ... }`. The direct dot has already been
    /// consumed and `{` is current. This list deliberately has its own
    /// grammar so it cannot be mistaken for a lambda or statement block.
    fn parse_copy_update(&mut self, base: Expr) -> Result<Expr, Diagnostic> {
        self.bump();
        if matches!(self.peek().kind, TokenKind::RBrace) {
            return Err(Diagnostic::at(
                self.peek().span,
                "copy update field list must not be empty",
            ));
        }

        let mut fields = Vec::new();
        loop {
            if matches!(self.peek().kind, TokenKind::DotDot) {
                return Err(Diagnostic::at(
                    self.peek().span,
                    "copy update field list does not allow rest entries",
                ));
            }
            let field = self.expect_ident("copy update field name")?;
            if matches!(self.peek().kind, TokenKind::Dot | TokenKind::QuestionDot) {
                return Err(Diagnostic::at(
                    self.peek().span,
                    "copy update fields must be direct names, not nested paths",
                ));
            }
            self.expect("`:` after copy update field name", |kind| {
                matches!(kind, TokenKind::Colon)
            })?;
            let statement_only = matches!(
                self.peek().kind,
                TokenKind::Val
                    | TokenKind::Var
                    | TokenKind::While
                    | TokenKind::For
                    | TokenKind::Break
                    | TokenKind::Continue
            );
            if statement_only {
                return Err(Diagnostic::at(
                    self.peek().span,
                    "copy update field values must be expressions, not statements",
                ));
            }
            let value = self.parse_expr()?;
            let span = Span::new(field.span.start, value.span().end);
            fields.push(FieldUpdate { field, value, span });

            if matches!(self.peek().kind, TokenKind::RBrace) {
                break;
            }
            self.expect("`,` or `}` after copy update field", |kind| {
                matches!(kind, TokenKind::Comma)
            })?;
            if matches!(self.peek().kind, TokenKind::RBrace) {
                return Err(Diagnostic::at(
                    self.peek().span,
                    "copy update field list does not allow a trailing comma",
                ));
            }
        }
        let close = self.expect("`}`", |kind| matches!(kind, TokenKind::RBrace))?;
        let mut fields = fields.into_iter();
        let first = fields
            .next()
            .expect("the parser rejected an empty copy-update list");
        let fields = NonEmptyVec::new(first, fields.collect());
        Ok(Expr::CopyUpdate {
            span: Span::new(base.span().start, close.span.end),
            base: Box::new(base),
            fields,
        })
    }

    /// `receiver[index]` — the `[` is the current token. A `:` after the
    /// index expression is a slice, out of the M5 subset.
    fn parse_index(&mut self, receiver: Expr) -> Result<Expr, Diagnostic> {
        self.bump(); // `[`
        let first = self.parse_expr()?;
        if matches!(self.peek().kind, TokenKind::Colon) {
            return Err(Diagnostic::at(
                self.peek().span,
                "array slices are not supported yet (milestone M5)",
            ));
        }
        let mut rest = Vec::new();
        while matches!(self.peek().kind, TokenKind::Comma) {
            self.bump();
            if matches!(self.peek().kind, TokenKind::RBracket) {
                return self.unexpected("index expression after `,`");
            }
            rest.push(self.parse_expr()?);
        }
        let close = self.expect("`]`", |k| matches!(k, TokenKind::RBracket))?;
        Ok(Expr::Index {
            span: Span::new(receiver.span().start, close.span.end),
            receiver: Box::new(receiver),
            indices: NonEmptyVec::new(first, rest),
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
                navigation: Navigation::Direct,
            }));
        }
        self.pos += 1;
        let name = Ident {
            text,
            span: token.span,
        };
        let type_args = self.parse_explicit_call_type_args()?;
        if matches!(self.peek().kind, TokenKind::LParen) {
            let (args, end) = self.parse_args()?;
            return Ok(Expr::MethodCall {
                span: Span::new(receiver.span().start, end),
                receiver: Box::new(receiver),
                name,
                navigation: if safe {
                    Navigation::Safe
                } else {
                    Navigation::Direct
                },
                type_args,
                args,
            });
        }
        Ok(Expr::FieldAccess(FieldAccess {
            span: Span::new(receiver.span().start, token.span.end),
            receiver: Box::new(receiver),
            selector: FieldSelector::Name(name),
            navigation: if safe {
                Navigation::Safe
            } else {
                Navigation::Direct
            },
        }))
    }

    pub(crate) fn expr_into_place(expr: Expr, context: &str) -> Result<PlaceExpr, Diagnostic> {
        let span = expr.span();
        match expr {
            Expr::Var(name) => Ok(PlaceExpr::Name(name)),
            Expr::FieldAccess(access) => match (access.navigation, access.selector) {
                (Navigation::Direct, FieldSelector::Name(name)) => Ok(PlaceExpr::Field {
                    receiver: access.receiver,
                    name,
                    span: access.span,
                }),
                (Navigation::Safe, _) if context == "assignment target" => Err(Diagnostic::at(
                    span,
                    "assignments through `?.` are not allowed",
                )),
                _ => Err(Diagnostic::at(
                    span,
                    format!("{context} must be an assignable place"),
                )),
            },
            Expr::Index {
                receiver,
                indices,
                span,
            } => Ok(PlaceExpr::Index {
                receiver,
                indices,
                span,
            }),
            Expr::QualifiedInterfaceSuperAccess {
                qualifier,
                name,
                span,
                ..
            } => Ok(PlaceExpr::QualifiedInterfaceSuperProperty {
                qualifier,
                name,
                span,
            }),
            _ => Err(Diagnostic::at(
                span,
                format!("{context} must be an assignable place"),
            )),
        }
    }
}

pub(crate) fn loop_jump_expression_diagnostic(name: &str, span: Span) -> Diagnostic {
    Diagnostic::at(
        span,
        format!("`{name}` jump expressions are not supported in M22"),
    )
}
