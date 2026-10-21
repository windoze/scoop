use super::*;
use scoop_ast::{AnnotationClassDecl, AnnotationParameterDecl};

impl Parser {
    pub(in crate::decl) fn parse_annotation_class(
        &mut self,
        prefix: MemberPrefix,
    ) -> Result<AnnotationClassDecl, Diagnostic> {
        self.with_node(scoop_syntax::SyntaxKind::AnnotationClassDecl, |parser| {
            parser.parse_annotation_class_inner(prefix)
        })
    }

    fn parse_annotation_class_inner(
        &mut self,
        prefix: MemberPrefix,
    ) -> Result<AnnotationClassDecl, Diagnostic> {
        self.require_unmodified_nominal_prefix(&prefix, "annotation class")?;
        let start = self.bump().span.start;
        self.expect("`class` after `annotation`", |kind| {
            matches!(kind, TokenKind::Class)
        })?;
        let name = self.expect_ident("annotation class name")?;
        let mut end = name.span.end;
        let mut parameters = Vec::new();
        if matches!(self.peek().kind, TokenKind::Less) {
            return Err(Diagnostic::at(
                self.peek().span,
                "annotation classes cannot have type parameters",
            ));
        }
        if matches!(self.peek().kind, TokenKind::LParen) {
            self.bump();
            while !matches!(self.peek().kind, TokenKind::RParen) {
                let doc_start = self.pos;
                self.skip_parameter_compatibility_modifiers();
                let start = self
                    .expect("`val` annotation parameter", |kind| {
                        matches!(kind, TokenKind::Val)
                    })?
                    .span
                    .start;
                let name = self.expect_ident("annotation parameter name")?;
                self.expect("`:`", |kind| matches!(kind, TokenKind::Colon))?;
                let ty = self.parse_type_ref()?;
                let (default, end) = if matches!(self.peek().kind, TokenKind::Equal) {
                    self.bump();
                    let (value, end) = self.parse_annotation_value()?;
                    (Some(value), end)
                } else {
                    (None, ty.span.end)
                };
                let documented = AnnotationParameterDecl {
                    name,
                    ty,
                    default,
                    span: Span::new(start, end),
                };
                self.attach_documentation(doc_start, documented.span);
                parameters.push(documented);
                if !matches!(self.peek().kind, TokenKind::Comma) {
                    break;
                }
                self.bump();
            }
            end = self
                .expect("`)`", |kind| matches!(kind, TokenKind::RParen))?
                .span
                .end;
        }
        if matches!(self.peek().kind, TokenKind::Colon | TokenKind::LBrace) {
            return Err(Diagnostic::at(
                self.peek().span,
                "annotation classes cannot have supertypes or a body",
            ));
        }
        Ok(AnnotationClassDecl {
            annotations: prefix.annotations,
            visibility: prefix.visibility,
            name,
            parameters,
            span: Span::new(start, end),
        })
    }
}
