use super::*;

impl Parser {
    /// Keep each initialization form until HIR resolves the nominal kind.
    pub(in crate::decl) fn parse_supertypes(
        &mut self,
        end: &mut u32,
    ) -> Result<Vec<SupertypeSpec>, Diagnostic> {
        let mut supertypes = Vec::new();
        if !matches!(self.peek().kind, TokenKind::Colon) {
            return Ok(supertypes);
        }
        self.bump();
        loop {
            let name = self.expect_ident("base class or interface name")?;
            let ty = self.parse_named_type_ref_tail(name)?;
            let start = ty.span.start;
            *end = ty.span.end;
            let initialization = if matches!(self.peek().kind, TokenKind::LParen) {
                let (arguments, arguments_end) = self.parse_args()?;
                *end = arguments_end;
                SupertypeInitialization::Constructor(arguments)
            } else if matches!(&self.peek().kind, TokenKind::Ident(name) if name == "by") {
                let by_span = self.bump().span;
                let outer = self
                    .supertype_expression_nesting
                    .replace(self.expression_nesting[self.pos]);
                let expression = self.parse_expr();
                self.supertype_expression_nesting = outer;
                let expression = expression?;
                *end = expression.span().end;
                SupertypeInitialization::Delegate {
                    expression: Box::new(expression),
                    by_span,
                }
            } else {
                SupertypeInitialization::None
            };
            supertypes.push(SupertypeSpec {
                ty,
                initialization,
                span: Span::new(start, *end),
            });
            if !matches!(self.peek().kind, TokenKind::Comma) {
                break;
            }
            self.bump();
        }
        Ok(supertypes)
    }
}
