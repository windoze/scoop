use super::*;

impl Parser {
    /// Parse compiler annotations without assigning them language semantics.
    /// Target, schema and coexistence checks belong to HIR (M12 design 1.2).
    pub(crate) fn parse_annotations(&mut self) -> Result<Vec<Annotation>, Diagnostic> {
        let mut annotations = Vec::new();
        while matches!(self.peek().kind, TokenKind::At) {
            let at = self.bump();
            let name = self.expect_ident("annotation name")?;
            let (args, end) = self.parse_annotation_arguments(name.span.end)?;
            annotations.push(Annotation {
                name,
                args,
                span: Span::new(at.span.start, end),
            });
        }
        Ok(annotations)
    }

    fn parse_annotation_arguments(
        &mut self,
        marker_end: u32,
    ) -> Result<(Vec<AnnotationArg>, u32), Diagnostic> {
        if !matches!(self.peek().kind, TokenKind::LParen) {
            return Ok((Vec::new(), marker_end));
        }
        self.bump();
        let mut args = Vec::new();
        let mut saw_named = false;
        if !matches!(self.peek().kind, TokenKind::RParen) {
            loop {
                let start = self.peek().span.start;
                let name = if matches!(self.peek().kind, TokenKind::Ident(_))
                    && matches!(self.tokens[self.pos + 1].kind, TokenKind::Equal)
                {
                    let name = self.expect_ident("annotation argument name")?;
                    self.bump();
                    saw_named = true;
                    Some(name)
                } else {
                    if saw_named {
                        return Err(Diagnostic::at(
                            self.peek().span,
                            "positional annotation arguments must precede named arguments",
                        ));
                    }
                    None
                };
                let token = self.bump();
                let value = match token.kind {
                    TokenKind::Str(value) => AnnotationLiteral::String(value),
                    TokenKind::Int(value) => AnnotationLiteral::Int(value),
                    TokenKind::True => AnnotationLiteral::Boolean(true),
                    TokenKind::False => AnnotationLiteral::Boolean(false),
                    _ => {
                        return Err(Diagnostic::at(
                            token.span,
                            "annotation argument must be a string, integer or boolean literal",
                        ));
                    }
                };
                args.push(AnnotationArg {
                    name,
                    value,
                    span: Span::new(start, token.span.end),
                });
                if matches!(self.peek().kind, TokenKind::Comma) {
                    self.bump();
                    if matches!(self.peek().kind, TokenKind::RParen) {
                        break;
                    }
                } else {
                    break;
                }
            }
        }
        let close = self.expect("`)`", |kind| matches!(kind, TokenKind::RParen))?;
        Ok((args, close.span.end))
    }
}
