use super::*;
use crate::parser::ListKind;
use scoop_syntax::SyntaxKind;

impl Parser {
    pub(super) fn parse_primary_parameters(
        &mut self,
    ) -> Result<(Vec<PrimaryClassParameter>, Span), Diagnostic> {
        self.with_node(SyntaxKind::ParameterList, |parser| {
            let open = parser.bump();
            let mut vararg_span = None;
            let parameters = parser.parse_delimited_items(ListKind::FunctionParameters, |parser| {
                parser.with_node(SyntaxKind::Parameter, |parser| {
                    parser.parse_primary_parameter(&mut vararg_span)
                })
            });
            Ok((parameters.items, Span::new(open.span.start, parameters.end)))
        })
    }

    fn parse_primary_parameter(
        &mut self,
        vararg_span: &mut Option<Span>,
    ) -> Result<PrimaryClassParameter, Diagnostic> {
        let doc_start = self.pos;
        let annotations = self.parse_annotations()?;
        let modifier_span = self.parse_vararg_modifier(vararg_span)?;
        let member_visibility = self.parse_visibility()?;
        let override_span = if matches!(&self.peek().kind, TokenKind::Ident(text) if text == "override")
        {
            Some(self.bump().span)
        } else {
            None
        };
        let first = self.peek().clone();
        let property = match first.kind {
            TokenKind::Val => {
                self.pos += 1;
                PrimaryParameterProperty::Val
            }
            TokenKind::Var => {
                self.pos += 1;
                PrimaryParameterProperty::Var
            }
            _ => PrimaryParameterProperty::Plain,
        };
        if !property.is_property()
            && (!matches!(member_visibility, VisibilitySyntax::Omitted) || override_span.is_some())
        {
            return Err(Diagnostic::at(
                first.span,
                "primary constructor member visibility and `override` require `val` or `var`",
            ));
        }
        let parameter_name = self.expect_ident("primary constructor parameter name")?;
        self.record_declaration_name(&parameter_name);
        self.expect("`:`", |k| matches!(k, TokenKind::Colon))?;
        let ty = self.parse_type_ref()?;
        let (syntax, end) = self.parse_parameter_syntax(modifier_span, ty.span.end)?;
        let documented = PrimaryClassParameter {
            annotations,
            property,
            member_visibility: property.is_property().then_some(member_visibility),
            is_override: override_span.is_some(),
            span: Span::new(
                modifier_span
                    .map(|span| span.start)
                    .or(match member_visibility {
                        VisibilitySyntax::Explicit { span, .. } => Some(span.start),
                        VisibilitySyntax::Omitted => None,
                    })
                    .or_else(|| override_span.map(|span| span.start))
                    .unwrap_or(first.span.start),
                end,
            ),
            name: parameter_name,
            ty,
            syntax,
        };
        self.attach_documentation(doc_start, documented.span);
        Ok(documented)
    }

    pub(super) fn parse_secondary_parameters(&mut self) -> Result<Vec<Param>, Diagnostic> {
        self.with_node(SyntaxKind::ParameterList, |parser| {
            parser.expect("`(` after `constructor`", |kind| {
                matches!(kind, TokenKind::LParen)
            })?;
            let mut vararg_span = None;
            let parameters = parser.parse_delimited_items(ListKind::FunctionParameters, |parser| {
                parser.with_node(SyntaxKind::Parameter, |parser| {
                    parser.parse_secondary_parameter(&mut vararg_span)
                })
            });
            Ok(parameters.items)
        })
    }

    fn parse_secondary_parameter(
        &mut self,
        vararg_span: &mut Option<Span>,
    ) -> Result<Param, Diagnostic> {
        let doc_start = self.pos;
        let annotations = self.parse_annotations()?;
        let modifier_span = self.parse_vararg_modifier(vararg_span)?;
        if matches!(self.peek().kind, TokenKind::Val | TokenKind::Var) {
            return Err(Diagnostic::at(
                self.peek().span,
                "secondary constructor parameters cannot declare `val` or `var` properties",
            ));
        }
        let name = self.expect_ident("constructor parameter name")?;
        self.record_declaration_name(&name);
        self.expect("`:`", |kind| matches!(kind, TokenKind::Colon))?;
        let ty = self.parse_type_ref()?;
        let (syntax, end) = self.parse_parameter_syntax(modifier_span, ty.span.end)?;
        let documented = Param {
            annotations,
            span: Span::new(
                modifier_span.map_or(name.span.start, |span| span.start),
                end,
            ),
            name,
            ty,
            syntax,
        };
        self.attach_documentation(doc_start, documented.span);
        Ok(documented)
    }
}
