//! Scalar character and string literals share escape decoding and recovery.

use super::*;

impl Lexer<'_> {
    pub(super) fn lex_string(&mut self) -> Result<TokenKind, Diagnostic> {
        if self.source[self.pos..].starts_with("\"\"\"") {
            return self.lex_raw_string();
        }
        self.quoted_text('"', "string").map(TokenKind::Str)
    }

    fn lex_raw_string(&mut self) -> Result<TokenKind, Diagnostic> {
        let start = self.pos;
        self.pos += 3;
        let content = self.pos;
        let Some(end) = self.source[content..].find("\"\"\"") else {
            self.pos = self.source.len();
            return Err(Diagnostic::at(
                self.span_from(start),
                "unterminated raw string literal",
            ));
        };
        self.pos += end;
        let quotes = self.source[self.pos..]
            .bytes()
            .take_while(|byte| *byte == b'"')
            .count();
        self.pos += quotes;
        Ok(TokenKind::Str(
            self.source[content..self.pos - 3].to_owned(),
        ))
    }

    pub(super) fn lex_character(&mut self) -> Result<TokenKind, Diagnostic> {
        let start = self.pos;
        let value = self.quoted_text('\'', "character")?;
        let mut chars = value.chars();
        match (chars.next(), chars.next()) {
            (Some(value), None) => Ok(TokenKind::Char(value)),
            _ => Err(Diagnostic::at(
                self.span_from(start),
                "a character literal must contain exactly one Unicode scalar value",
            )),
        }
    }

    fn quoted_text(&mut self, quote: char, subject: &str) -> Result<String, Diagnostic> {
        let start = self.pos;
        self.pos += 1;
        let mut value = String::new();
        loop {
            match self.peek_char() {
                None | Some('\n' | '\r') => {
                    return Err(Diagnostic::at(
                        self.span_from(start),
                        format!("unterminated {subject} literal"),
                    ));
                }
                Some(c) if c == quote => {
                    self.pos += 1;
                    return Ok(value);
                }
                Some('\\') => match self.escape() {
                    Ok(c) => value.push(c),
                    Err(error) => {
                        self.skip_quoted_remainder(quote);
                        return Err(error);
                    }
                },
                Some(c) => {
                    self.pos += c.len_utf8();
                    value.push(c);
                }
            }
        }
    }

    pub(super) fn skip_quoted_remainder(&mut self, quote: char) {
        while let Some(c) = self.peek_char() {
            if matches!(c, '\n' | '\r') {
                return;
            }
            self.pos += c.len_utf8();
            if c == quote {
                return;
            }
            if c == '\\'
                && let Some(next) = self.peek_char()
                && !matches!(next, '\n' | '\r')
            {
                self.pos += next.len_utf8();
            }
        }
    }

    pub(super) fn escape(&mut self) -> Result<char, Diagnostic> {
        let start = self.pos;
        self.pos += 1;
        let Some(c) = self.peek_char() else {
            return Err(Diagnostic::at(
                self.span_from(start),
                "unterminated escape sequence",
            ));
        };
        if matches!(c, '\n' | '\r') {
            return Err(Diagnostic::at(
                self.span_from(start),
                "unterminated escape sequence",
            ));
        }
        self.pos += c.len_utf8();
        Ok(match c {
            't' => '\t',
            'b' => '\u{8}',
            'n' => '\n',
            'r' => '\r',
            '\'' => '\'',
            '"' => '"',
            '\\' => '\\',
            '$' => '$',
            'u' => return self.unicode_escape(start),
            _ => {
                return Err(Diagnostic::at(
                    self.span_from(start),
                    format!("unsupported escape sequence `\\{c}`"),
                ));
            }
        })
    }

    fn unicode_escape(&mut self, start: usize) -> Result<char, Diagnostic> {
        let braced = self.eat('{');
        let mut value = 0u32;
        let mut digits = 0;
        while let Some(c) = self.peek_char() {
            let Some(digit) = c.to_digit(16) else {
                break;
            };
            if digits == if braced { 6 } else { 4 } {
                break;
            }
            self.pos += c.len_utf8();
            value = value * 16 + digit;
            digits += 1;
        }
        let valid_length = if braced {
            digits != 0 && self.eat('}')
        } else {
            digits == 4
        };
        if !valid_length {
            return Err(Diagnostic::at(
                self.span_from(start),
                "Unicode escape requires four hex digits or one to six hex digits in braces",
            ));
        }
        char::from_u32(value).ok_or_else(|| {
            Diagnostic::at(
                self.span_from(start),
                "Unicode escape must name a Unicode scalar value",
            )
        })
    }
}
