use super::*;
use scoop_ast::{ContextParameter, ContextParameterLabel};

impl Parser {
    pub(super) fn parse_context_parameters(
        &mut self,
    ) -> Result<(Vec<ContextParameter>, Span), Diagnostic> {
        let keyword = self.bump();
        self.expect("`(` after `context`", |kind| {
            matches!(kind, TokenKind::LParen)
        })?;
        if matches!(self.peek().kind, TokenKind::RParen) {
            return Err(Diagnostic::at(
                self.peek().span,
                "a context parameter list cannot be empty",
            ));
        }
        let mut parameters = Vec::new();
        loop {
            let name = self.expect_ident("context parameter name or `_`")?;
            self.expect(
                "`:` after context parameter name (modifiers are not allowed)",
                |kind| matches!(kind, TokenKind::Colon),
            )?;
            let ty = self.parse_type_ref()?;
            let span = Span::new(name.span.start, ty.span.end);
            let label = if name.text == "_" {
                ContextParameterLabel::Unnamed(name.span)
            } else {
                ContextParameterLabel::Named(name)
            };
            parameters.push(ContextParameter { label, ty, span });
            if matches!(self.peek().kind, TokenKind::Equal) {
                return Err(Diagnostic::at(
                    self.peek().span,
                    "context parameters cannot have defaults",
                ));
            }
            if !matches!(self.peek().kind, TokenKind::Comma) {
                break;
            }
            self.bump();
        }
        let close = self.expect("`)` after context parameters", |kind| {
            matches!(kind, TokenKind::RParen)
        })?;
        Ok((parameters, Span::new(keyword.span.start, close.span.end)))
    }

    pub(crate) fn parse_context_local_function(&mut self) -> Result<FunctionDecl, Diagnostic> {
        let prefix = self.parse_top_level_prefix()?;
        if !matches!(self.peek().kind, TokenKind::Fun)
            || !prefix.annotations.is_empty()
            || !matches!(prefix.visibility, VisibilitySyntax::Omitted)
            || prefix.modifiers.method_modifier.is_some()
            || prefix.modifiers.is_override
        {
            return Err(Diagnostic::at(
                self.peek().span,
                "a local context declaration must be a named function",
            ));
        }
        self.parse_prefixed_member_function(prefix, FunctionContext::Local)
    }
}
