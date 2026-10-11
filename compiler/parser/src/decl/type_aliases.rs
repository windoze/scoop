use super::*;

impl Parser {
    /// Parse a declaration-site alias with its own parameters and bounds.
    pub(super) fn parse_type_alias(
        &mut self,
        prefix: MemberPrefix,
    ) -> Result<TypeAliasDecl, Diagnostic> {
        if let Some(annotation) = prefix.annotations.first() {
            return Err(Diagnostic::at(
                annotation.span,
                "annotations are not allowed on typealias declarations",
            ));
        }

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
