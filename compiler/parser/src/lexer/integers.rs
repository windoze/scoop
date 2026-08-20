//! Integer radix, suffix, and magnitude scanning.

use super::*;

impl Lexer<'_> {
    /// `pos` is at the first digit.
    pub(super) fn lex_int(&mut self) -> Result<TokenKind, Diagnostic> {
        if self.starts_float() {
            return self.lex_float();
        }
        let start = self.pos;
        let radix = if self.source[self.pos..].starts_with("0b")
            || self.source[self.pos..].starts_with("0B")
        {
            self.pos += 2;
            IntegerRadix::Binary
        } else if self.source[self.pos..].starts_with("0x")
            || self.source[self.pos..].starts_with("0X")
        {
            self.pos += 2;
            IntegerRadix::Hexadecimal
        } else {
            IntegerRadix::Decimal
        };
        let base = match radix {
            IntegerRadix::Decimal => 10,
            IntegerRadix::Binary => 2,
            IntegerRadix::Hexadecimal => 16,
        };

        let digits_start = self.pos;
        let mut magnitude = 0_u64;
        let mut saw_digit = false;
        let mut overflowed = false;
        while let Some(c) = self.peek_char() {
            if let Some(digit) = c.to_digit(base) {
                saw_digit = true;
                self.pos += c.len_utf8();
                if !overflowed {
                    match magnitude
                        .checked_mul(u64::from(base))
                        .and_then(|value| value.checked_add(u64::from(digit)))
                    {
                        Some(value) => magnitude = value,
                        None => overflowed = true,
                    }
                }
                continue;
            }
            if c == '_' {
                let separator_start = self.pos;
                self.pos += 1;
                let valid_next = self
                    .peek_char()
                    .and_then(|next| next.to_digit(base))
                    .is_some();
                if !saw_digit || !valid_next {
                    self.skip_integer_tail();
                    return Err(Diagnostic::at(
                        Span::new(separator_start as u32, self.pos as u32),
                        "integer separators must appear between two valid digits",
                    ));
                }
                continue;
            }
            break;
        }

        if !saw_digit {
            self.skip_integer_tail();
            let radix_name = match radix {
                IntegerRadix::Decimal => "decimal",
                IntegerRadix::Binary => "binary",
                IntegerRadix::Hexadecimal => "hexadecimal",
            };
            return Err(Diagnostic::at(
                self.span_from(start),
                format!("{radix_name} integer prefix must be followed by a valid digit"),
            ));
        }

        if radix != IntegerRadix::Decimal && self.peek_char().is_some_and(|c| c.is_ascii_digit()) {
            let invalid_start = self.pos;
            self.skip_integer_tail();
            return Err(Diagnostic::at(
                Span::new(invalid_start as u32, self.pos as u32),
                format!(
                    "invalid digit in base-{base} integer literal `{}`",
                    &self.source[start..self.pos]
                ),
            ));
        }

        let suffix_start = self.pos;
        let suffix = match self.peek_char() {
            Some('u' | 'U') => {
                self.pos += 1;
                if matches!(self.peek_char(), Some('l' | 'L')) {
                    self.pos += 1;
                    IntegerSuffix::UnsignedLong
                } else {
                    IntegerSuffix::Unsigned
                }
            }
            Some('l' | 'L') => {
                self.pos += 1;
                IntegerSuffix::Long
            }
            _ => IntegerSuffix::None,
        };

        if self.peek_char().is_some_and(is_ident_continue) {
            self.skip_integer_tail();
            return Err(Diagnostic::at(
                Span::new(suffix_start as u32, self.pos as u32),
                format!(
                    "invalid integer literal suffix `{}`",
                    &self.source[suffix_start..self.pos]
                ),
            ));
        }

        if overflowed {
            return Err(Diagnostic::at(
                self.span_from(start),
                format!(
                    "integer literal magnitude `{}` is out of range for u64",
                    &self.source[start..self.pos]
                ),
            ));
        }

        debug_assert!(self.pos > digits_start);
        Ok(TokenKind::Int(IntegerLiteralLexeme {
            magnitude,
            radix,
            suffix,
        }))
    }

    fn skip_integer_tail(&mut self) {
        while let Some(c) = self.peek_char() {
            if !is_ident_continue(c) {
                break;
            }
            self.pos += c.len_utf8();
        }
    }
}
