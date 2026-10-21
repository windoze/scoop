use super::*;
use crate::parser::ListKind;
use scoop_syntax::SyntaxKind;

impl Parser {
    pub(super) fn parse_struct_fields(&mut self) -> Result<(Vec<FieldDecl>, u32), Diagnostic> {
        self.with_node(SyntaxKind::ParameterList, |parser| {
            parser.bump();
            let mut vararg_span = None;
            Ok(parser
                .parse_delimited_items(ListKind::FunctionParameters, |parser| {
                    parser.with_node(SyntaxKind::FieldDecl, |parser| {
                        parser.parse_struct_field(&mut vararg_span)
                    })
                })
                .into_parts())
        })
    }

    fn parse_struct_field(
        &mut self,
        vararg_span: &mut Option<Span>,
    ) -> Result<FieldDecl, Diagnostic> {
        let doc_start = self.pos;
        let annotations = self.parse_annotations()?;
        let modifier_span = self.parse_vararg_modifier(vararg_span)?;
        if matches!(self.peek().kind, TokenKind::Var) {
            return Err(Diagnostic::at(
                self.peek().span,
                "`var` struct fields are not supported (value types are immutable)",
            ));
        }
        let val = self.expect("`val`", |k| matches!(k, TokenKind::Val))?;
        let field_name = self.expect_ident("field name")?;
        self.expect("`:`", |k| matches!(k, TokenKind::Colon))?;
        let ty = self.parse_type_ref()?;
        let (syntax, end) = self.parse_parameter_syntax(modifier_span, ty.span.end)?;
        let documented = FieldDecl {
            annotations,
            span: Span::new(modifier_span.map_or(val.span.start, |span| span.start), end),
            name: field_name,
            ty,
            syntax,
        };
        self.attach_documentation(doc_start, documented.span);
        Ok(documented)
    }
}
