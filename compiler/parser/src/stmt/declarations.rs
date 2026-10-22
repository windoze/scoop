use super::*;

impl Parser {
    pub(super) fn parse_annotated_local_declaration(
        &mut self,
    ) -> Result<Option<Statement>, Diagnostic> {
        let saved = self.checkpoint();
        let annotations = self.parse_annotations()?;
        let alias = matches!(&self.peek().kind, TokenKind::Ident(text) if text == "typealias")
            || matches!(&self.peek().kind, TokenKind::Ident(text) if matches!(text.as_str(), "public" | "internal" | "private" | "protected"))
                && matches!(&self.tokens[self.pos + 1].kind, TokenKind::Ident(text) if text == "typealias");
        if alias {
            self.restore(saved);
            let declaration = self.parse_local_type_alias()?;
            return Ok(Some(Statement {
                span: declaration.span,
                kind: StatementKind::TypeAlias(declaration),
            }));
        }
        if self.starts_modified_local_function() && !self.starts_anonymous_function() {
            let function = self.parse_non_member_function(
                annotations,
                VisibilitySyntax::Omitted,
                crate::decl::FunctionContext::Local,
                self.has_errors_since(saved),
            )?;
            return Ok(Some(Statement {
                span: function.span,
                kind: StatementKind::LocalFunction(function),
            }));
        }
        self.restore(saved);
        Ok(None)
    }
}
