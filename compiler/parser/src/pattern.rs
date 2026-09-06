//! Pattern parsing (spec 4.6 and chapter 5).
//!
//! Pattern positions — destructuring `val` declarations and `when` arm
//! conditions — are pure pattern syntax: `..` here is always the rest
//! marker, never the range operator (the spec 4.6 disambiguation rule).
//! Enum variant patterns and struct patterns share their shapes
//! (`Path(...)`, `Path { ... }`, tuple patterns); HIR resolves which is
//! which. Except for the built-in `Unit` literal spelling, an unqualified
//! bare identifier stays unresolved binding-shaped syntax; HIR classifies it
//! according to the binding or match context. A dotted path without an
//! argument list (`E.V`) is a unit variant pattern, encoded as a positional
//! pattern with no elements.

use scoop_ast::{Diagnostic, Expr, FieldPattern, Ident, Pattern, Span};

use crate::lexer::TokenKind;
use crate::parser::Parser;

/// Every pattern carries a span; `Binding` uses its identifier's.
pub(crate) fn pattern_span(pattern: &Pattern) -> Span {
    match pattern {
        Pattern::Binding(ident) => ident.span,
        Pattern::Wildcard { span }
        | Pattern::Literal { span, .. }
        | Pattern::Positional { span, .. }
        | Pattern::Named { span, .. }
        | Pattern::Tuple { span, .. } => *span,
    }
}

impl Parser {
    pub(crate) fn parse_pattern(&mut self) -> Result<Pattern, Diagnostic> {
        let token = self.peek().clone();
        match token.kind {
            TokenKind::Minus => self.parse_negated_integer_literal_pattern(token),
            TokenKind::Int(lexeme) => {
                self.pos += 1;
                Ok(Pattern::Literal {
                    expr: Box::new(Expr::IntLiteral(lexeme.with_span(token.span))),
                    span: token.span,
                })
            }
            TokenKind::Str(value) => {
                self.pos += 1;
                Ok(Pattern::Literal {
                    expr: Box::new(Expr::StringLiteral {
                        value,
                        span: token.span,
                    }),
                    span: token.span,
                })
            }
            TokenKind::True => {
                self.pos += 1;
                Ok(Pattern::Literal {
                    expr: Box::new(Expr::BoolLiteral {
                        value: true,
                        span: token.span,
                    }),
                    span: token.span,
                })
            }
            TokenKind::False => {
                self.pos += 1;
                Ok(Pattern::Literal {
                    expr: Box::new(Expr::BoolLiteral {
                        value: false,
                        span: token.span,
                    }),
                    span: token.span,
                })
            }
            TokenKind::LParen => self.parse_tuple_pattern(),
            TokenKind::LBrace => self.parse_named_pattern(Vec::new(), token.span.start),
            TokenKind::Ident(text) => self.parse_ident_pattern(text, token.span),
            _ => self.unexpected("pattern"),
        }
    }

    /// Unary minus is part of literal-pattern syntax only when its operand is
    /// one integer token modulo parenthesized disambiguation. Parentheses do
    /// not survive in AST, but their closing delimiter belongs to the unary
    /// expression's source span.
    fn parse_negated_integer_literal_pattern(
        &mut self,
        prefix: crate::lexer::Token,
    ) -> Result<Pattern, Diagnostic> {
        debug_assert!(matches!(prefix.kind, TokenKind::Minus));
        self.pos += 1;
        let (literal, end) = self.parse_integer_literal_pattern_operand()?;
        let span = Span::new(prefix.span.start, end);
        Ok(Pattern::Literal {
            expr: Box::new(Expr::Unary {
                op: scoop_ast::UnOp::Neg,
                operand: Box::new(Expr::IntLiteral(literal)),
                span,
            }),
            span,
        })
    }

    fn parse_integer_literal_pattern_operand(
        &mut self,
    ) -> Result<(scoop_ast::IntegerLiteralSyntax, u32), Diagnostic> {
        let token = self.peek().clone();
        match token.kind {
            TokenKind::Int(lexeme) => {
                self.pos += 1;
                Ok((lexeme.with_span(token.span), token.span.end))
            }
            TokenKind::LParen => {
                self.pos += 1;
                let (literal, _) = self.parse_integer_literal_pattern_operand()?;
                let close = self.expect("`)`", |kind| matches!(kind, TokenKind::RParen))?;
                Ok((literal, close.span.end))
            }
            _ => self.unexpected("integer literal after unary minus"),
        }
    }

    /// An identifier-started pattern: `_`, the `Unit` literal, a binding
    /// name, or a (possibly dotted) variant/struct path with a positional
    /// or field argument list.
    fn parse_ident_pattern(&mut self, text: String, span: Span) -> Result<Pattern, Diagnostic> {
        self.pos += 1;
        if text == "_" {
            return Ok(Pattern::Wildcard { span });
        }
        if text == "Unit" {
            return Ok(Pattern::Literal {
                expr: Box::new(Expr::UnitLiteral { span }),
                span,
            });
        }
        let start = span.start;
        let mut path = vec![Ident { text, span }];
        // `E.V` — an enum type prefix plus the variant name. The dot must
        // be followed by an identifier; otherwise the path ends.
        while matches!(self.peek().kind, TokenKind::Dot)
            && matches!(
                self.tokens.get(self.pos + 1).map(|token| &token.kind),
                Some(TokenKind::Ident(_))
            )
        {
            self.bump(); // `.`
            path.push(self.expect_ident("variant name")?);
        }
        match self.peek().kind {
            TokenKind::LParen => self.parse_positional_pattern(path, start),
            TokenKind::LBrace => self.parse_named_pattern(path, start),
            _ if path.len() == 1 => {
                let name = path.into_iter().next().expect("path holds one ident");
                Ok(Pattern::Binding(name))
            }
            // `E.V` without an argument list is a unit variant pattern.
            _ => {
                let end = path.last().expect("path is non-empty").span.end;
                Ok(Pattern::Positional {
                    path,
                    elements: Vec::new(),
                    rest: None,
                    span: Span::new(start, end),
                })
            }
        }
    }

    /// `Path(p1, p2, ..)` — the `(` is the current token. `..` may appear
    /// at any position but at most once (spec 4.6).
    fn parse_positional_pattern(
        &mut self,
        path: Vec<Ident>,
        start: u32,
    ) -> Result<Pattern, Diagnostic> {
        self.bump(); // `(`
        let mut elements = Vec::new();
        let mut rest = None;
        if !matches!(self.peek().kind, TokenKind::RParen) {
            loop {
                if matches!(self.peek().kind, TokenKind::DotDot) {
                    let dotdot = self.bump();
                    if rest.is_some() {
                        return Err(Diagnostic::at(
                            dotdot.span,
                            "`..` may appear at most once in a pattern",
                        ));
                    }
                    rest = Some(dotdot.span);
                } else {
                    elements.push(self.parse_pattern()?);
                }
                if matches!(self.peek().kind, TokenKind::Comma) {
                    self.bump();
                    if matches!(self.peek().kind, TokenKind::RParen) {
                        break; // trailing comma
                    }
                } else {
                    break;
                }
            }
        }
        let close = self.expect("`)`", |k| matches!(k, TokenKind::RParen))?;
        Ok(Pattern::Positional {
            path,
            elements,
            rest,
            span: Span::new(start, close.span.end),
        })
    }

    /// `Path? { f1, f2: subpattern, .. }` — the `{` is the current token.
    /// `..` is allowed only as the last element (spec 4.6).
    fn parse_named_pattern(&mut self, path: Vec<Ident>, start: u32) -> Result<Pattern, Diagnostic> {
        self.bump(); // `{`
        let mut fields = Vec::new();
        let mut rest = None;
        loop {
            match self.peek().kind {
                TokenKind::RBrace => break,
                TokenKind::Eof => return self.unexpected("`}`"),
                TokenKind::DotDot => {
                    let dotdot = self.bump();
                    if rest.is_some() {
                        return Err(Diagnostic::at(
                            dotdot.span,
                            "`..` may appear at most once in a pattern",
                        ));
                    }
                    rest = Some(dotdot.span);
                    if matches!(self.peek().kind, TokenKind::Comma) {
                        self.bump();
                    }
                    if !matches!(self.peek().kind, TokenKind::RBrace) {
                        return Err(Diagnostic::at(
                            self.peek().span,
                            "`..` must be the last element in a field pattern",
                        ));
                    }
                }
                _ => {
                    let field = self.expect_ident("field name")?;
                    if field.text == "_" {
                        return Err(Diagnostic::at(
                            field.span,
                            "`_` is not allowed in a field pattern",
                        ));
                    }
                    let subpattern = if matches!(self.peek().kind, TokenKind::Colon) {
                        self.bump();
                        self.parse_pattern()?
                    } else if field.text == "Unit" {
                        // Shorthand is exactly `field: field`, including the
                        // built-in Unit-literal classification of its RHS.
                        Pattern::Literal {
                            expr: Box::new(Expr::UnitLiteral { span: field.span }),
                            span: field.span,
                        }
                    } else {
                        Pattern::Binding(field.clone())
                    };
                    let end = pattern_span(&subpattern).end;
                    fields.push(FieldPattern {
                        span: Span::new(field.span.start, end),
                        field,
                        subpattern: Box::new(subpattern),
                    });
                    if matches!(self.peek().kind, TokenKind::Comma) {
                        self.bump();
                    } else {
                        break;
                    }
                }
            }
        }
        let close = self.expect("`}`", |k| matches!(k, TokenKind::RBrace))?;
        Ok(Pattern::Named {
            path,
            fields,
            rest,
            span: Span::new(start, close.span.end),
        })
    }

    /// `(p1, p2, ..)` — the `(` is the current token. Parenthesized
    /// disambiguation mirrors expressions and types (spec 4.3): `()` is
    /// the `Unit` literal pattern, `(p)` is just `p`, `(p,)` a 1-tuple.
    fn parse_tuple_pattern(&mut self) -> Result<Pattern, Diagnostic> {
        let open = self.bump(); // `(`
        if matches!(self.peek().kind, TokenKind::RParen) {
            let close = self.bump();
            let span = Span::new(open.span.start, close.span.end);
            return Ok(Pattern::Literal {
                expr: Box::new(Expr::UnitLiteral { span }),
                span,
            });
        }
        let mut elements = Vec::new();
        let mut rest = None;
        if matches!(self.peek().kind, TokenKind::DotDot) {
            // `(..)` / `(.., p)` — a leading rest marker; tuple for sure.
            rest = Some(self.bump().span);
        } else {
            let first = self.parse_pattern()?;
            if !matches!(self.peek().kind, TokenKind::Comma) {
                // `(p)` — parentheses around a single pattern.
                self.expect("`)`", |k| matches!(k, TokenKind::RParen))?;
                return Ok(first);
            }
            elements.push(first);
        }
        while matches!(self.peek().kind, TokenKind::Comma) {
            self.bump();
            if matches!(self.peek().kind, TokenKind::RParen) {
                break; // trailing comma: `(p,)` / `(p1, p2,)`
            }
            if matches!(self.peek().kind, TokenKind::DotDot) {
                let dotdot = self.bump();
                if rest.is_some() {
                    return Err(Diagnostic::at(
                        dotdot.span,
                        "`..` may appear at most once in a pattern",
                    ));
                }
                rest = Some(dotdot.span);
            } else {
                elements.push(self.parse_pattern()?);
            }
        }
        let close = self.expect("`)`", |k| matches!(k, TokenKind::RParen))?;
        Ok(Pattern::Tuple {
            elements,
            rest,
            span: Span::new(open.span.start, close.span.end),
        })
    }
}
