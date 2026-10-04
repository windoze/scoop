//! One token stream retains nested interpolation and callable identities.

use super::*;

#[derive(Clone, Copy)]
pub(super) enum InterpolationMode {
    Text { raw: bool, start: usize },
    Expression { braces: usize, start: usize },
}

impl Lexer<'_> {
    pub(super) fn start_fstring(&mut self, start: usize) -> TokenKind {
        let raw = self.source[self.pos..].starts_with("\"\"\"");
        self.pos += if raw { 3 } else { 1 };
        self.interpolations
            .push(InterpolationMode::Text { raw, start });
        TokenKind::FStringStart
    }

    pub(super) fn lex_fstring_text(
        &mut self,
        raw: bool,
        opening: usize,
    ) -> Result<TokenKind, Diagnostic> {
        let mut value = String::new();
        loop {
            let remaining = &self.source[self.pos..];
            let closes = if raw {
                let quotes = remaining.bytes().take_while(|byte| *byte == b'"').count();
                if quotes >= 3 {
                    // In a closing quote run, only the final three delimit.
                    value.extend(std::iter::repeat_n('"', quotes - 3));
                    self.pos += quotes - 3;
                    true
                } else {
                    false
                }
            } else {
                remaining.starts_with('"')
            };
            if closes {
                if !value.is_empty() {
                    return Ok(TokenKind::FStringText(value));
                }
                self.pos += if raw { 3 } else { 1 };
                self.interpolations.pop();
                return Ok(TokenKind::FStringEnd);
            }
            if remaining.starts_with("${") {
                if !value.is_empty() {
                    return Ok(TokenKind::FStringText(value));
                }
                self.interpolations.push(InterpolationMode::Expression {
                    braces: 0,
                    start: self.pos,
                });
                self.pos += 2;
                return Ok(TokenKind::InterpolationStart);
            }
            match self.peek_char() {
                None | Some('\n' | '\r') if !raw || self.pos == self.source.len() => {
                    return Err(Diagnostic::at(
                        self.span_from(opening),
                        "unterminated f-string literal",
                    ));
                }
                Some('$')
                    if self.source[self.pos + 1..]
                        .chars()
                        .next()
                        .is_some_and(is_ident_start) =>
                {
                    let start = self.pos;
                    self.pos += 1;
                    while self.peek_char().is_some_and(is_ident_continue) {
                        self.pos += 1;
                    }
                    let name = &self.source[start + 1..self.pos];
                    return Err(Diagnostic::at(
                        self.span_from(start),
                        format!("`${name}` interpolation requires braces; use `${{{name}}}`"),
                    ));
                }
                Some('\\') if !raw => value.push(self.escape()?),
                Some(c) => {
                    self.pos += c.len_utf8();
                    value.push(c);
                }
                None => unreachable!("end of source was handled above"),
            }
        }
    }

    pub(super) fn skip_fstring_remainder(&mut self, raw: bool) {
        self.interpolations.pop();
        if !raw {
            self.skip_quoted_remainder('"');
            return;
        }
        if let Some(end) = self.source[self.pos..].find("\"\"\"") {
            self.pos += end + 3;
        } else {
            self.pos = self.source.len();
        }
    }

    pub(super) fn opening_brace(&mut self) -> TokenKind {
        if let Some(InterpolationMode::Expression { braces, .. }) = self.interpolations.last_mut() {
            *braces += 1;
        }
        self.pos += 1;
        TokenKind::LBrace
    }

    pub(super) fn closing_brace(&mut self) -> TokenKind {
        self.pos += 1;
        if let Some(InterpolationMode::Expression { braces, .. }) = self.interpolations.last_mut() {
            if *braces == 0 {
                self.interpolations.pop();
                return TokenKind::InterpolationEnd;
            }
            *braces -= 1;
        }
        TokenKind::RBrace
    }
}
