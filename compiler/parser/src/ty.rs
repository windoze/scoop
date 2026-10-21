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
        self.with_node(scoop_syntax::SyntaxKind::Type, |parser| {
            parser.parse_type_ref_inner()
        })
    }

    fn parse_type_ref_inner(&mut self) -> Result<TypeRef, Diagnostic> {
        let start = self.peek().span.start;
        let annotations = self.parse_annotations()?;
        let is_suspend = matches!(self.peek().kind, TokenKind::Suspend);
        if is_suspend {
            self.bump();
        }
        let parenthesized = matches!(self.peek().kind, TokenKind::LParen);
        let mut ty = self.parse_type_atom_with_suffix()?;
        if matches!(self.peek().kind, TokenKind::Dot | TokenKind::QuestionDot)
            && matches!(
                self.tokens.get(self.pos + 1).map(|token| &token.kind),
                Some(TokenKind::LParen)
            )
        {
            ty = self.parse_receiver_separator(ty);
            let function = self.parse_paren_type(is_suspend, start)?;
            let TypeRefKind::Function(mut signature) = function.kind else {
                return Err(Diagnostic::at(
                    function.span,
                    "receiver function type requires `->` and a result type",
                ));
            };
            signature.receiver = Some(Box::new(ty));
            return Ok(TypeRef {
                annotations,
                kind: TypeRefKind::Function(signature),
                span: Span::new(start, function.span.end),
            });
        }
        if is_suspend {
            let TypeRefKind::Function(signature) = &mut ty.kind else {
                if parenthesized {
                    return Err(Diagnostic::at(
                        self.peek().span,
                        format!(
                            "expected `->` in suspend function type, found {}",
                            self.peek().describe()
                        ),
                    ));
                }
                return Err(Diagnostic::at(
                    ty.span,
                    "`suspend` requires a function type",
                ));
            };
            signature.is_suspend = true;
            ty.span.start = start;
        }
        ty.annotations.splice(0..0, annotations);
        ty.span.start = start;
        Ok(ty)
    }

    pub(crate) fn parse_receiver_separator(&mut self, receiver: TypeRef) -> TypeRef {
        let separator = self.bump();
        if matches!(separator.kind, TokenKind::QuestionDot) {
            TypeRef {
                annotations: Vec::new(),
                span: Span::new(receiver.span.start, separator.span.start + 1),
                kind: TypeRefKind::Nullable(Box::new(receiver)),
            }
        } else {
            receiver
        }
    }

    pub(crate) fn parse_type_atom_with_suffix(&mut self) -> Result<TypeRef, Diagnostic> {
        let start = self.peek().span.start;
        let annotations = self.parse_annotations()?;
        let mut ty = self.parse_type_atom()?;
        while matches!(self.peek().kind, TokenKind::Question) {
            let question = self.bump();
            ty = TypeRef {
                annotations: Vec::new(),
                span: Span::new(ty.span.start, question.span.end),
                kind: TypeRefKind::Nullable(Box::new(ty)),
            };
        }
        ty.annotations.splice(0..0, annotations);
        ty.span.start = start;
        Ok(ty)
    }

    fn parse_type_atom(&mut self) -> Result<TypeRef, Diagnostic> {
        let token = self.peek().clone();
        match token.kind {
            TokenKind::Ident(ref text) if text == "_" => Err(Diagnostic::at(
                token.span,
                "`_` is only allowed in call type argument lists",
            )),
            TokenKind::Ident(text) | TokenKind::EscapedIdent(text) => {
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
                annotations: Vec::new(),
                span: Span::new(start, return_type.span.end),
                kind: TypeRefKind::Function(scoop_ast::FunctionTypeRef {
                    receiver: None,
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
                annotations: Vec::new(),
                kind: TypeRefKind::Unit,
                span: Span::new(start, close.span.end),
            });
        }
        if elements.len() == 1 && !had_comma {
            let first = elements.pop().expect("one parenthesized type");
            return Ok(TypeRef {
                annotations: first.annotations,
                kind: first.kind,
                span: Span::new(start, close.span.end),
            });
        }
        Ok(TypeRef {
            annotations: Vec::new(),
            kind: TypeRefKind::Tuple(elements),
            span: Span::new(start, close.span.end),
        })
    }

    /// Complete a named type after its identifier was consumed. Supertype
    /// lists use the same generic-application grammar as annotations.
    pub(crate) fn parse_named_type_ref_tail(&mut self, name: Ident) -> Result<TypeRef, Diagnostic> {
        let start = name.span.start;
        let (arguments, end) = self.parse_nominal_arguments(name.span.end)?;
        let kind = if !arguments.is_empty() {
            TypeRefKind::Generic(name, arguments)
        } else {
            TypeRefKind::Named(name)
        };
        let mut ty = TypeRef {
            annotations: Vec::new(),
            kind,
            span: Span::new(start, end),
        };
        while matches!(self.peek().kind, TokenKind::Dot)
            && matches!(
                self.tokens.get(self.pos + 1).map(|token| &token.kind),
                Some(TokenKind::Ident(_) | TokenKind::EscapedIdent(_))
            )
        {
            self.bump();
            let name = self.expect_ident("nested type name after `.`")?;
            let (arguments, end) = self.parse_nominal_arguments(name.span.end)?;
            ty = ty.with_member(name, arguments, end);
        }
        if matches!(&ty.kind, TypeRefKind::Named(name) if name.text == "Unit") {
            ty.kind = TypeRefKind::Unit;
        }
        Ok(ty)
    }

    pub(crate) fn parse_nominal_arguments(
        &mut self,
        end: u32,
    ) -> Result<(Vec<TypeRef>, u32), Diagnostic> {
        self.with_node(scoop_syntax::SyntaxKind::TypeArgumentList, |parser| {
            parser.parse_nominal_arguments_inner(end)
        })
    }

    fn parse_nominal_arguments_inner(
        &mut self,
        end: u32,
    ) -> Result<(Vec<TypeRef>, u32), Diagnostic> {
        if !matches!(self.peek().kind, TokenKind::Less) {
            return Ok((Vec::new(), end));
        }
        self.bump();
        let diagnostic_start = self.diagnostics.len();
        let arguments = self.parse_delimited_items(
            crate::parser::ListKind::TypeArguments,
            Self::parse_nominal_type_argument,
        );
        // An erroneous type must not become a successful speculative type
        // argument. Its siblings remain in the CST for editor consumers.
        match self.diagnostics.get(diagnostic_start) {
            Some(diagnostic) => Err(diagnostic.clone()),
            None => Ok(arguments),
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
