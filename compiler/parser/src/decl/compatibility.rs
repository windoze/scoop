//! Syntax-only Kotlin compatibility modifiers. None reaches semantic IR.

use super::*;

impl Parser {
    pub(crate) fn skip_parameter_compatibility_modifiers(&mut self) {
        while matches!(&self.peek().kind, TokenKind::Ident(text)
            if matches!(text.as_str(), "crossinline" | "noinline"))
            && matches!(
                self.tokens[self.pos + 1].kind,
                TokenKind::Ident(_)
                    | TokenKind::EscapedIdent(_)
                    | TokenKind::Val
                    | TokenKind::Var
                    | TokenKind::Vararg
            )
        {
            self.bump();
        }
    }

    pub(crate) fn parse_vararg_modifier(
        &mut self,
        existing: &mut Option<Span>,
    ) -> Result<Option<Span>, Diagnostic> {
        self.skip_parameter_compatibility_modifiers();
        if !matches!(self.peek().kind, TokenKind::Vararg) {
            return Ok(None);
        }
        let modifier = self.bump();
        self.skip_parameter_compatibility_modifiers();
        if matches!(self.peek().kind, TokenKind::Vararg) {
            return Err(Diagnostic::at(
                self.peek().span,
                "duplicate `vararg` modifier on parameter",
            ));
        }
        if existing.is_some() {
            return Err(Diagnostic::at(
                modifier.span,
                "a parameter list may declare at most one `vararg` parameter",
            ));
        }
        *existing = Some(modifier.span);
        Ok(Some(modifier.span))
    }

    pub(crate) fn starts_modified_local_function(&self) -> bool {
        let mut tokens = self.tokens[self.pos..].iter().peekable();
        for token in tokens.by_ref() {
            match &token.kind {
                TokenKind::Suspend | TokenKind::Infix => {}
                TokenKind::Ident(text)
                    if matches!(text.as_str(), "inline" | "tailrec" | "operator") => {}
                TokenKind::Fun => {
                    return !matches!(
                        tokens.peek().map(|token| &token.kind),
                        Some(TokenKind::LParen)
                    );
                }
                _ => return false,
            }
        }
        false
    }

    pub(super) fn parse_accessor_prefix(
        &mut self,
    ) -> Result<(Vec<Annotation>, VisibilitySyntax), Diagnostic> {
        let mut annotations = Vec::new();
        let mut visibility = VisibilitySyntax::Omitted;
        loop {
            match &self.peek().kind {
                TokenKind::At => annotations.extend(self.parse_annotations()?),
                TokenKind::Ident(text) if text == "inline" => {
                    self.bump();
                }
                kind if visibility_token(kind).is_some() => {
                    if !matches!(visibility, VisibilitySyntax::Omitted) {
                        return Err(Diagnostic::at(
                            self.peek().span,
                            "a declaration may have only one visibility modifier",
                        ));
                    }
                    visibility = self.parse_visibility()?;
                }
                _ => break,
            }
        }
        Ok((annotations, visibility))
    }
}
