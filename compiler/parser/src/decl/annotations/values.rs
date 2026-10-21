use super::*;

impl Parser {
    pub(in crate::decl) fn parse_annotation_path(
        &mut self,
        expected: &str,
    ) -> Result<scoop_ast::QualifiedNameSyntax, Diagnostic> {
        self.parse_qualified_name(expected)
    }

    pub(in crate::decl) fn parse_annotation_value(
        &mut self,
    ) -> Result<(AnnotationLiteral, u32), Diagnostic> {
        if matches!(
            self.peek().kind,
            TokenKind::Comma | TokenKind::RParen | TokenKind::RBracket | TokenKind::Eof
        ) {
            return self.unexpected("annotation value");
        }
        if matches!(self.peek().kind, TokenKind::LBracket) {
            return self.parse_annotation_array();
        }
        if matches!(
            self.peek().kind,
            TokenKind::Ident(_) | TokenKind::EscapedIdent(_)
        ) {
            let saved = self.checkpoint();
            let name = self.parse_annotation_path("annotation name")?;
            if matches!(self.peek().kind, TokenKind::LParen) {
                let (args, end) = self.parse_annotation_arguments(name.span.end)?;
                let span = scoop_ast::Span::new(name.span.start, end);
                return Ok((
                    AnnotationLiteral::Application(Box::new(scoop_ast::Annotation {
                        use_site: None,
                        name,
                        args,
                        span,
                    })),
                    end,
                ));
            }
            self.restore(saved);
            let reference = self.parse_postfix()?;
            if !is_constant_path(&reference) {
                return Err(Diagnostic::at(
                    reference.span(),
                    "annotation argument must be a scalar literal or const val reference",
                ));
            }
            let end = reference.span().end;
            return Ok((AnnotationLiteral::ConstReference(Box::new(reference)), end));
        }
        let token = self.bump();
        let end = token.span.end;
        let value = match token.kind {
            TokenKind::Str(value) => AnnotationLiteral::String(value),
            TokenKind::Int(lexeme) => AnnotationLiteral::Int(lexeme.with_span(token.span)),
            TokenKind::Float(lexeme) => AnnotationLiteral::Float(lexeme),
            TokenKind::True => AnnotationLiteral::Boolean(true),
            TokenKind::False => AnnotationLiteral::Boolean(false),
            TokenKind::Char(value) => AnnotationLiteral::Char(value),
            TokenKind::Minus | TokenKind::Plus => {
                let negative = matches!(token.kind, TokenKind::Minus);
                let number = self.bump();
                let value = match number.kind {
                    TokenKind::Int(lexeme) => AnnotationLiteral::SignedInt {
                        negative,
                        literal: lexeme.with_span(number.span),
                    },
                    TokenKind::Float(literal) => {
                        AnnotationLiteral::SignedFloat { negative, literal }
                    }
                    _ => {
                        return Err(Diagnostic::at(
                            number.span,
                            "annotation sign must precede a numeric literal",
                        ));
                    }
                };
                return Ok((value, number.span.end));
            }
            _ => {
                return Err(Diagnostic::at(
                    token.span,
                    "annotation argument must be a scalar literal or const val reference",
                ));
            }
        };
        Ok((value, end))
    }

    fn parse_annotation_array(&mut self) -> Result<(AnnotationLiteral, u32), Diagnostic> {
        self.with_node(scoop_syntax::SyntaxKind::AnnotationArray, |parser| {
            parser.parse_annotation_array_inner()
        })
    }

    fn parse_annotation_array_inner(&mut self) -> Result<(AnnotationLiteral, u32), Diagnostic> {
        self.bump();
        let diagnostic_start = self.diagnostics.len();
        let elements =
            self.parse_delimited_items(crate::parser::ListKind::AnnotationElements, |parser| {
                let start = parser.peek().span.start;
                let (value, end) = parser.parse_annotation_value()?;
                Ok(scoop_ast::AnnotationArrayElement {
                    value,
                    span: scoop_ast::Span::new(start, end),
                })
            });
        if let Some(diagnostic) = self.diagnostics.get(diagnostic_start) {
            return Err(diagnostic.clone());
        }
        Ok((AnnotationLiteral::Array(elements.items), elements.end))
    }
}

fn is_constant_path(mut expression: &scoop_ast::Expr) -> bool {
    use scoop_ast::{Expr, FieldSelector, Navigation};
    loop {
        match expression {
            Expr::Var(_) | Expr::TypeQualifier(_) => return true,
            Expr::FieldAccess(access)
                if access.navigation == Navigation::Direct
                    && matches!(access.selector, FieldSelector::Name(_)) =>
            {
                expression = &access.receiver
            }
            _ => return false,
        }
    }
}
