use super::*;

impl Lexer<'_> {
    pub(super) fn lex_escaped_ident(&mut self) -> Result<TokenKind, Diagnostic> {
        let start = self.pos;
        self.pos += 1;
        let content_start = self.pos;
        let mut invalid_character = None;
        while let Some(c) = self.peek_char() {
            if c == '`' {
                let text = &self.source[content_start..self.pos];
                self.pos += 1;
                return if let Some(span) = invalid_character {
                    Err(invalid_identifier_character(span))
                } else if text.is_empty() {
                    Err(Diagnostic::at(
                        self.span_from(start),
                        "escaped identifier must not be empty",
                    ))
                } else {
                    Ok(TokenKind::EscapedIdent(text.to_owned()))
                };
            }
            if c.is_control() || matches!(c, '\u{2028}' | '\u{2029}') {
                invalid_character
                    .get_or_insert(Span::new(self.pos as u32, (self.pos + c.len_utf8()) as u32));
            }
            self.pos += c.len_utf8();
        }
        Err(invalid_character
            .map(invalid_identifier_character)
            .unwrap_or_else(|| {
                Diagnostic::at(self.span_from(start), "unterminated escaped identifier")
            }))
    }
}

fn invalid_identifier_character(span: Span) -> Diagnostic {
    Diagnostic::at(
        span,
        "escaped identifier cannot contain a newline or control character",
    )
}
