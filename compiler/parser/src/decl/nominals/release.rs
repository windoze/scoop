use super::*;

impl Parser {
    pub(super) fn parse_release_block(&mut self) -> Result<ReleaseBlock, Diagnostic> {
        let keyword = self.bump();
        if !matches!(self.peek().kind, TokenKind::LBrace) {
            return Err(Diagnostic::at(
                self.peek().span,
                "`release` must be followed by a block without parameters or a return type",
            ));
        }
        let body = self.parse_block()?;
        Ok(ReleaseBlock {
            span: Span::new(keyword.span.start, body.span.end),
            body,
        })
    }

    pub(in crate::decl) fn release_block_starts_here(&self) -> bool {
        matches!(&self.peek().kind, TokenKind::Ident(name) if name == "release")
            && self
                .tokens
                .get(self.pos + 1)
                .is_some_and(|token| matches!(token.kind, TokenKind::LBrace))
    }

    pub(in crate::decl) fn invalid_release_owner<T>(&self, owner: &str) -> Result<T, Diagnostic> {
        Err(Diagnostic::at(
            self.peek().span,
            format!(
                "`release` blocks are not allowed in {owner}; only ordinary final classes may declare them"
            ),
        ))
    }
}
