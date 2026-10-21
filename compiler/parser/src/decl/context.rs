use super::*;
use scoop_ast::{ContextParameter, ContextParameterLabel};

impl Parser {
    pub(super) fn parse_context_parameters(
        &mut self,
    ) -> Result<(Vec<ContextParameter>, Span), Diagnostic> {
        self.with_node(scoop_syntax::SyntaxKind::ContextParameters, |parser| {
            parser.parse_context_parameters_inner()
        })
    }

    fn parse_context_parameters_inner(
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
        let parameters = self.parse_delimited_items(
            crate::parser::ListKind::FunctionParameters,
            Self::parse_context_parameter,
        );
        Ok((
            parameters.items,
            Span::new(keyword.span.start, parameters.end),
        ))
    }

    fn parse_context_parameter(&mut self) -> Result<ContextParameter, Diagnostic> {
        self.with_node(
            scoop_syntax::SyntaxKind::Parameter,
            Self::parse_context_parameter_inner,
        )
    }

    fn parse_context_parameter_inner(&mut self) -> Result<ContextParameter, Diagnostic> {
        let doc_start = self.pos;
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
        self.attach_documentation(doc_start, span);
        if matches!(self.peek().kind, TokenKind::Equal) {
            return Err(Diagnostic::at(
                self.peek().span,
                "context parameters cannot have defaults",
            ));
        }
        Ok(ContextParameter { label, ty, span })
    }

    pub(crate) fn parse_context_local_function(&mut self) -> Result<FunctionDecl, Diagnostic> {
        let prefix = self.parse_top_level_prefix()?;
        if !matches!(self.peek().kind, TokenKind::Fun)
            || !prefix.annotations.is_empty()
            || !matches!(prefix.visibility, VisibilitySyntax::Omitted)
            || prefix.modifiers.method_modifier.is_some()
            || prefix.modifiers.override_span.is_some()
        {
            return Err(Diagnostic::at(
                self.peek().span,
                "a local context declaration must be a named function",
            ));
        }
        self.parse_prefixed_member_function(prefix, FunctionContext::Local)
    }
}
