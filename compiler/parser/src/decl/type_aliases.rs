use super::*;

impl Parser {
    /// Parse the M22 alias subset: `typealias Name = Type` at top level.
    ///
    /// Only visibility may precede the declaration. Keeping unsupported
    /// annotations/modifiers and generic syntax out of the successful AST
    /// prevents later stages from having to represent partial alias forms.
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
            .chain(prefix.modifiers.method_modifier_span)
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
        if matches!(self.peek().kind, TokenKind::Less) {
            return Err(Diagnostic::at(
                self.peek().span,
                "generic typealias declarations are not supported in M22; typealias declarations must be non-generic",
            ));
        }
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
            target,
            span,
        })
    }

    pub(super) fn unsupported_nested_type_alias<T>(&self) -> Result<T, Diagnostic> {
        Err(Diagnostic::at(
            self.peek().span,
            "nested typealias declarations are not supported in M22; only top-level non-generic typealias declarations are supported",
        ))
    }
}
