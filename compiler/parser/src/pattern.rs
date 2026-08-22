//! Pattern parsing (spec 4.6 and chapter 5).
//!
//! Pattern positions — destructuring `val` declarations and `when` arm
//! conditions — are pure pattern syntax: `..` here is always the rest
//! marker, never the range operator (the spec 4.6 disambiguation rule).
//! Enum variant patterns and struct patterns share their shapes
//! (`Path(...)`, `Path { ... }`, tuple patterns); HIR resolves which is
//! which. A bare identifier is always a binding ("binding first", spec
//! chapter 5); a dotted path without an argument list (`E.V`) is a unit
//! variant pattern, encoded as a positional pattern with no elements.

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
            TokenKind::Int(value) => {
                self.pos += 1;
                Ok(Pattern::Literal {
                    expr: Box::new(Expr::IntLiteral {
                        value,
                        span: token.span,
                    }),
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
            TokenKind::Ident(text) => self.parse_ident_pattern(text, token.span),
            _ => self.unexpected("pattern"),
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

    /// `Path { f1, f2: renamed, .. }` — the `{` is the current token.
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
                    let name = self.expect_ident("field name")?;
                    if name.text == "_" {
                        return Err(Diagnostic::at(
                            name.span,
                            "`_` is not allowed in a field pattern",
                        ));
                    }
                    let rename = if matches!(self.peek().kind, TokenKind::Colon) {
                        self.bump();
                        let rename = self.expect_ident("binding name")?;
                        if rename.text == "_" {
                            return Err(Diagnostic::at(
                                rename.span,
                                "`_` is not allowed in a field pattern",
                            ));
                        }
                        Some(rename)
                    } else {
                        None
                    };
                    let end = rename.as_ref().map(|r| r.span.end).unwrap_or(name.span.end);
                    fields.push(FieldPattern {
                        span: Span::new(name.span.start, end),
                        name,
                        rename,
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
