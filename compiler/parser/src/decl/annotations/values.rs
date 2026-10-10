use super::*;

impl Parser {
    pub(in crate::decl) fn parse_annotation_path(
        &mut self,
        expected: &str,
    ) -> Result<scoop_ast::Ident, Diagnostic> {
        let mut name = self.expect_ident(expected)?;
        while matches!(self.peek().kind, TokenKind::Dot) {
            self.bump();
            let next = self.expect_ident("qualified annotation name segment")?;
            name.text.push('.');
            name.text.push_str(&next.text);
            name.span.end = next.span.end;
        }
        Ok(name)
    }

    pub(in crate::decl) fn parse_annotation_value(
        &mut self,
    ) -> Result<(AnnotationLiteral, u32), Diagnostic> {
        if matches!(self.peek().kind, TokenKind::LBracket) {
            return self.parse_annotation_array();
        }
        if matches!(self.peek().kind, TokenKind::Ident(_)) {
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
        self.bump();
        let mut elements = Vec::new();
        while !matches!(self.peek().kind, TokenKind::RBracket) {
            let start = self.peek().span.start;
            let (value, end) = self.parse_annotation_value()?;
            elements.push(scoop_ast::AnnotationArrayElement {
                value,
                span: scoop_ast::Span { start, end },
            });
            if !matches!(self.peek().kind, TokenKind::Comma) {
                break;
            }
            self.bump();
        }
        let end = self
            .expect("`]` after annotation array", |kind| {
                matches!(kind, TokenKind::RBracket)
            })?
            .span
            .end;
        Ok((AnnotationLiteral::Array(elements), end))
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
