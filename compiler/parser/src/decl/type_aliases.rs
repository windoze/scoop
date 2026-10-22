use super::*;

impl Parser {
    /// Parse a declaration-site alias with its own parameters and bounds.
    pub(super) fn parse_type_alias(
        &mut self,
        prefix: MemberPrefix,
    ) -> Result<TypeAliasDecl, Diagnostic> {
        self.with_node(scoop_syntax::SyntaxKind::TypeAliasDecl, |parser| {
            parser.record_declaration_prefix(
                prefix.visibility,
                scoop_ast::DeclarationHeaderKind::Type,
            );
            parser.parse_type_alias_inner(prefix)
        })
    }

    fn parse_type_alias_inner(
        &mut self,
        prefix: MemberPrefix,
    ) -> Result<TypeAliasDecl, Diagnostic> {
        let modifier_span = prefix
            .const_span
            .into_iter()
            .chain(prefix.modifiers.suspend_span)
            .chain(prefix.modifiers.operator.map(|modifier| modifier.span))
            .chain(prefix.modifiers.infix.map(|modifier| modifier.span))
            .chain(
                prefix
                    .modifiers
                    .method_modifier
                    .map(|modifier| modifier.span),
            )
            .chain(prefix.modifiers.override_span)
            .min_by_key(|span| span.start);
        if let Some(span) = modifier_span {
            return Err(Diagnostic::at(
                span,
                "only visibility modifiers are allowed on typealias declarations",
            ));
        }

        let keyword = self.expect(
            "`typealias`",
            |kind| matches!(kind, TokenKind::Ident(text) if text == "typealias"),
        )?;
        let name = self.expect_ident("typealias name")?;
        self.record_declaration_name(&name);
        let type_params = self.parse_type_params()?;
        let where_clause = self.parse_where_clause()?;
        self.expect("`=` in typealias declaration", |kind| {
            matches!(kind, TokenKind::Equal)
        })?;
        let target = self.parse_type_ref()?;
        let start = match prefix.visibility {
            VisibilitySyntax::Explicit { span, .. } => span.start,
            VisibilitySyntax::Omitted => keyword.span.start,
        };
        let span = Span::new(start, target.span.end);
        Ok(TypeAliasDecl {
            annotations: prefix.annotations,
            visibility: prefix.visibility,
            name,
            type_params,
            where_clause,
            target,
            span,
        })
    }

    pub(crate) fn parse_local_type_alias(&mut self) -> Result<TypeAliasDecl, Diagnostic> {
        let prefix = self.parse_member_prefix()?;
        if let VisibilitySyntax::Explicit { span, .. } = prefix.visibility {
            return Err(Diagnostic::at(
                span,
                "local typealias declarations cannot have visibility modifiers",
            ));
        }
        self.parse_type_alias(prefix)
    }
}
