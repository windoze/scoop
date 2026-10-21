use super::*;
use crate::parser::ListKind;
use scoop_syntax::SyntaxKind;

impl Parser {
    pub(super) fn parse_parenthesized_variant_fields(
        &mut self,
    ) -> Result<(VariantDeclKind, u32), Diagnostic> {
        self.with_node(SyntaxKind::ParameterList, |parser| {
            parser.bump();
            let doc_start = parser.pos;
            let annotations = parser.parse_annotations()?;
            parser.skip_parameter_compatibility_modifiers();
            let mut first = Some((annotations, doc_start));
            if matches!(
                parser.peek().kind,
                TokenKind::Val | TokenKind::Var | TokenKind::Vararg
            ) {
                let mut vararg_span = None;
                let parsed = parser.parse_delimited_items(ListKind::FunctionParameters, |parser| {
                    parser.with_node(SyntaxKind::FieldDecl, |parser| {
                        let (annotations, doc_start) = parser.variant_field_prefix(&mut first)?;
                        parser.parse_constructor_variant_field(
                            annotations,
                            doc_start,
                            &mut vararg_span,
                        )
                    })
                });
                Ok((VariantDeclKind::Constructor(parsed.items), parsed.end))
            } else {
                let parsed =
                    parser.parse_delimited_items(ListKind::PositionalVariantFields, |parser| {
                        parser.with_node(SyntaxKind::FieldDecl, |parser| {
                            let (annotations, doc_start) =
                                parser.variant_field_prefix(&mut first)?;
                            let ty = parser.parse_type_ref()?;
                            let span = ty.span;
                            parser.attach_documentation(doc_start, span);
                            Ok(scoop_ast::PositionalVariantFieldDecl {
                                annotations,
                                ty,
                                span,
                            })
                        })
                    });
                Ok((VariantDeclKind::Positional(parsed.items), parsed.end))
            }
        })
    }

    fn variant_field_prefix(
        &mut self,
        first: &mut Option<(Vec<Annotation>, usize)>,
    ) -> Result<(Vec<Annotation>, usize), Diagnostic> {
        if let Some(prefix) = first.take() {
            return Ok(prefix);
        }
        let doc_start = self.pos;
        Ok((self.parse_annotations()?, doc_start))
    }

    fn parse_constructor_variant_field(
        &mut self,
        annotations: Vec<Annotation>,
        doc_start: usize,
        vararg_span: &mut Option<Span>,
    ) -> Result<VariantFieldDecl, Diagnostic> {
        let modifier_span = self.parse_vararg_modifier(vararg_span)?;
        if matches!(self.peek().kind, TokenKind::Var) {
            return Err(Diagnostic::at(
                self.peek().span,
                "`var` variant fields are not supported (value types are immutable)",
            ));
        }
        let val = self.expect("`val`", |kind| matches!(kind, TokenKind::Val))?;
        let name = self.expect_ident("field name")?;
        self.expect("`:`", |kind| matches!(kind, TokenKind::Colon))?;
        let ty = self.parse_type_ref()?;
        let (syntax, end) = self.parse_parameter_syntax(modifier_span, ty.span.end)?;
        let span = Span::new(modifier_span.map_or(val.span.start, |span| span.start), end);
        self.attach_documentation(doc_start, span);
        Ok(VariantFieldDecl {
            annotations,
            name,
            ty,
            syntax,
            span,
        })
    }

    /// Named variant fields allow commas or newlines, and require at least one field.
    pub(super) fn parse_named_fields(
        &mut self,
    ) -> Result<(Vec<VariantFieldDecl>, u32), Diagnostic> {
        self.with_node(SyntaxKind::ParameterList, |parser| {
            parser.bump();
            if matches!(parser.peek().kind, TokenKind::RBrace) {
                return parser.unexpected("field declaration");
            }
            Ok(parser
                .parse_delimited_items(ListKind::NamedVariantFields, |parser| {
                    parser.with_node(SyntaxKind::FieldDecl, Self::parse_named_variant_field)
                })
                .into_parts())
        })
    }

    fn parse_named_variant_field(&mut self) -> Result<VariantFieldDecl, Diagnostic> {
        let doc_start = self.pos;
        let annotations = self.parse_annotations()?;
        let name = self.expect_ident("field name")?;
        self.expect("`:`", |kind| matches!(kind, TokenKind::Colon))?;
        let ty = self.parse_type_ref()?;
        if matches!(self.peek().kind, TokenKind::Equal) {
            return Err(Diagnostic::at(
                self.peek().span,
                "variant field defaults require the constructor-style form (milestone M4)",
            ));
        }
        let span = Span::new(name.span.start, ty.span.end);
        self.attach_documentation(doc_start, span);
        Ok(VariantFieldDecl {
            annotations,
            name,
            ty,
            syntax: ParameterSyntax::Required,
            span,
        })
    }
}
