use super::*;
use crate::parser::ListKind;
use scoop_syntax::SyntaxKind;

impl Parser {
    pub(super) fn parse_function_parameters(&mut self) -> Result<(Vec<Param>, u32), Diagnostic> {
        self.with_node(SyntaxKind::ParameterList, |parser| {
            parser.expect("`(`", |kind| matches!(kind, TokenKind::LParen))?;
            let mut vararg_span = None;
            Ok(parser
                .parse_delimited_items(ListKind::FunctionParameters, |parser| {
                    parser.with_node(SyntaxKind::Parameter, |parser| {
                        parser.parse_function_parameter(&mut vararg_span)
                    })
                })
                .into_parts())
        })
    }

    fn parse_function_parameter(
        &mut self,
        vararg_span: &mut Option<Span>,
    ) -> Result<Param, Diagnostic> {
        let checkpoint = self.checkpoint();
        let doc_start = self.pos;
        let annotations = self.parse_annotations()?;
        let modifier_span = self.parse_vararg_modifier(vararg_span)?;
        let name = self.expect_ident("parameter name")?;
        self.record_declaration_name(&name);
        self.expect("`:`", |kind| matches!(kind, TokenKind::Colon))?;
        let ty = self.parse_type_ref()?;
        let (syntax, end) = self.parse_parameter_syntax(modifier_span, ty.span.end)?;
        let parameter = Param {
            annotations,
            span: Span::new(
                modifier_span.map_or(name.span.start, |span| span.start),
                end,
            ),
            name,
            ty,
            syntax,
        };
        self.attach_documentation(doc_start, parameter.span);
        self.record_declaration_source_span(parameter.span, checkpoint);
        Ok(parameter)
    }
}
