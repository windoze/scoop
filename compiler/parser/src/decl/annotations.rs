use super::*;
use scoop_ast::{AnnotationUseSite, AnnotationUseSiteTarget};
mod declaration;
mod values;

impl Parser {
    /// Preserve source annotations; target and constant checks belong to HIR.
    pub(crate) fn parse_annotations(&mut self) -> Result<Vec<Annotation>, Diagnostic> {
        self.with_node(scoop_syntax::SyntaxKind::AnnotationList, |parser| {
            parser.parse_annotations_inner()
        })
    }

    fn parse_annotations_inner(&mut self) -> Result<Vec<Annotation>, Diagnostic> {
        let mut annotations = Vec::new();
        while matches!(self.peek().kind, TokenKind::At) {
            annotations.push(self.parse_annotation()?);
        }
        Ok(annotations)
    }

    fn parse_annotation(&mut self) -> Result<Annotation, Diagnostic> {
        self.with_node(scoop_syntax::SyntaxKind::Annotation, |parser| {
            let at = parser.bump();
            let use_site = parser.parse_annotation_use_site()?;
            let name = parser.parse_annotation_path("annotation name")?;
            let (args, end) = parser.parse_annotation_arguments(name.span.end)?;
            Ok(Annotation {
                use_site,
                name,
                args,
                span: Span::new(at.span.start, end),
            })
        })
    }

    fn parse_annotation_use_site(&mut self) -> Result<Option<AnnotationUseSite>, Diagnostic> {
        if !matches!(
            self.tokens.get(self.pos + 1).map(|token| &token.kind),
            Some(TokenKind::Colon)
        ) {
            return Ok(None);
        }
        let name = self.expect_ident("annotation use-site target")?;
        self.bump();
        let target = match name.text.as_str() {
            "property" => AnnotationUseSiteTarget::Property,
            "field" => AnnotationUseSiteTarget::Field,
            "get" => AnnotationUseSiteTarget::Get,
            "set" => AnnotationUseSiteTarget::Set,
            "param" => AnnotationUseSiteTarget::Param,
            "setparam" => AnnotationUseSiteTarget::SetParam,
            "receiver" => AnnotationUseSiteTarget::Receiver,
            "delegate" => AnnotationUseSiteTarget::Delegate,
            _ => {
                return Err(Diagnostic::at(
                    name.span,
                    "unknown annotation use-site target",
                ));
            }
        };
        Ok(Some(AnnotationUseSite {
            target,
            span: name.span,
        }))
    }

    fn parse_annotation_arguments(
        &mut self,
        marker_end: u32,
    ) -> Result<(Vec<AnnotationArg>, u32), Diagnostic> {
        self.with_node(scoop_syntax::SyntaxKind::ArgumentList, |parser| {
            if !matches!(parser.peek().kind, TokenKind::LParen) {
                return Ok((Vec::new(), marker_end));
            }
            parser.bump();
            let mut saw_named = false;
            Ok(parser
                .parse_delimited_items(crate::parser::ListKind::AnnotationArguments, |parser| {
                    parser.with_node(scoop_syntax::SyntaxKind::Argument, |parser| {
                        parser.parse_annotation_argument(&mut saw_named)
                    })
                })
                .into_parts())
        })
    }

    fn parse_annotation_argument(
        &mut self,
        saw_named: &mut bool,
    ) -> Result<AnnotationArg, Diagnostic> {
        let start = self.peek().span.start;
        let name = if matches!(
            self.peek().kind,
            TokenKind::Ident(_) | TokenKind::EscapedIdent(_)
        ) && matches!(self.tokens[self.pos + 1].kind, TokenKind::Equal)
        {
            let name = self.expect_ident("annotation argument name")?;
            self.bump();
            *saw_named = true;
            Some(name)
        } else {
            if *saw_named {
                return Err(Diagnostic::at(
                    self.peek().span,
                    "positional annotation arguments must precede named arguments",
                ));
            }
            None
        };
        let (value, end) = self.parse_annotation_value()?;
        Ok(AnnotationArg {
            name,
            value,
            span: Span::new(start, end),
        })
    }
}
