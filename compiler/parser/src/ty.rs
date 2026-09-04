use scoop_ast::{Diagnostic, Ident, Span, TypeRef, TypeRefKind};

use crate::lexer::TokenKind;
use crate::parser::Parser;

impl Parser {
    /// A type annotation: a named type, `Unit` (also written `()`), a
    /// tuple type `(T1, T2, ...)`, or a function type `(P...) -> R` /
    /// `suspend (P...) -> R`, each with any number of `?` suffixes
    /// (`T?` is `Option<T>`, and `T??` does not collapse — spec 7.1).
    /// Parenthesized disambiguation mirrors expressions (spec section
    /// 4.3): `(T)` is just `T` in parentheses, `(T,)` a 1-tuple type.
    pub(crate) fn parse_type_ref(&mut self) -> Result<TypeRef, Diagnostic> {
        let mut ty = self.parse_type_atom()?;
        while matches!(self.peek().kind, TokenKind::Question) {
            let question = self.bump();
            ty = TypeRef {
                span: Span::new(ty.span.start, question.span.end),
                kind: TypeRefKind::Nullable(Box::new(ty)),
            };
        }
        Ok(ty)
    }

    fn parse_type_atom(&mut self) -> Result<TypeRef, Diagnostic> {
        let token = self.peek().clone();
        match token.kind {
            TokenKind::Suspend => {
                self.pos += 1;
                self.parse_paren_type(true, token.span.start)
            }
            TokenKind::Ident(text) => {
                self.pos += 1;
                self.parse_named_type_ref_tail(Ident {
                    text,
                    span: token.span,
                })
            }
            TokenKind::LParen => self.parse_paren_type(false, token.span.start),
            _ => self.unexpected("type"),
        }
    }

    /// Parse a parenthesized/tuple type or a function type parameter list.
    /// `suspend` has already been consumed when `is_suspend` is true.
    fn parse_paren_type(&mut self, is_suspend: bool, start: u32) -> Result<TypeRef, Diagnostic> {
        self.expect("`(`", |k| matches!(k, TokenKind::LParen))?;
        let mut elements = Vec::new();
        let mut had_comma = false;
        if !matches!(self.peek().kind, TokenKind::RParen) {
            elements.push(self.parse_type_ref()?);
            while matches!(self.peek().kind, TokenKind::Comma) {
                had_comma = true;
                self.bump();
                if matches!(self.peek().kind, TokenKind::RParen) {
                    break;
                }
                elements.push(self.parse_type_ref()?);
            }
        }
        let close = self.expect("`)`", |k| matches!(k, TokenKind::RParen))?;

        if matches!(self.peek().kind, TokenKind::Arrow) {
            self.bump();
            let return_type = self.parse_type_ref()?;
            return Ok(TypeRef {
                span: Span::new(start, return_type.span.end),
                kind: TypeRefKind::Function(scoop_ast::FunctionTypeRef {
                    is_suspend,
                    parameters: elements,
                    return_type: Box::new(return_type),
                }),
            });
        }

        if is_suspend {
            return Err(Diagnostic::at(
                self.peek().span,
                format!(
                    "expected `->` in suspend function type, found {}",
                    self.peek().describe()
                ),
            ));
        }
        if elements.is_empty() {
            return Ok(TypeRef {
                kind: TypeRefKind::Unit,
                span: Span::new(start, close.span.end),
            });
        }
        if elements.len() == 1 && !had_comma {
            let first = elements.pop().expect("one parenthesized type");
            return Ok(TypeRef {
                kind: first.kind,
                span: Span::new(start, close.span.end),
            });
        }
        Ok(TypeRef {
            kind: TypeRefKind::Tuple(elements),
            span: Span::new(start, close.span.end),
        })
    }

    /// Complete a named type after its identifier was consumed. Supertype
    /// lists use the same generic-application grammar as annotations.
    pub(crate) fn parse_named_type_ref_tail(&mut self, name: Ident) -> Result<TypeRef, Diagnostic> {
        if name.text == "Unit" {
            Ok(TypeRef {
                kind: TypeRefKind::Unit,
                span: name.span,
            })
        } else if matches!(self.peek().kind, TokenKind::Less) {
            let start = name.span.start;
            self.bump();
            let mut args = vec![self.parse_nominal_type_argument()?];
            while matches!(self.peek().kind, TokenKind::Comma) {
                self.bump();
                args.push(self.parse_nominal_type_argument()?);
            }
            let close = self.expect("`>`", |k| matches!(k, TokenKind::Greater))?;
            Ok(TypeRef {
                kind: TypeRefKind::Generic(name, args),
                span: Span::new(start, close.span.end),
            })
        } else {
            Ok(TypeRef {
                span: name.span,
                kind: TypeRefKind::Named(name),
            })
        }
    }

    fn parse_nominal_type_argument(&mut self) -> Result<TypeRef, Diagnostic> {
        let token = self.peek().clone();
        match &token.kind {
            TokenKind::In => Err(Diagnostic::at(
                token.span,
                "`in` projections are not supported; nominal generic applications are invariant",
            )),
            TokenKind::Ident(text) if text == "out" => Err(Diagnostic::at(
                token.span,
                "`out` projections are not supported; nominal generic applications are invariant",
            )),
            TokenKind::Star => Err(Diagnostic::at(
                token.span,
                "star projections are not supported; use a bounded generic callable, an exact interface, or an explicit wrapper",
            )),
            _ => self.parse_type_ref(),
        }
    }
}
