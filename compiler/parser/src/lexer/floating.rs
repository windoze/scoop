//! Decimal floating literals, without host floating-point conversion.

use super::*;
use scoop_ast::{FloatLiteralSyntax, FloatSuffix};

impl Lexer<'_> {
    pub(super) fn starts_float(&self) -> bool {
        let tail = &self.source.as_bytes()[self.pos..];
        if tail.starts_with(b"0x")
            || tail.starts_with(b"0X")
            || tail.starts_with(b"0b")
            || tail.starts_with(b"0B")
        {
            return false;
        }
        let mut index = 0;
        while tail
            .get(index)
            .is_some_and(|b| b.is_ascii_digit() || *b == b'_')
        {
            index += 1;
        }
        matches!(tail.get(index), Some(b'e' | b'E' | b'f' | b'F'))
            || (tail.get(index) == Some(&b'.')
                && tail.get(index + 1).is_some_and(u8::is_ascii_digit))
    }

    pub(super) fn lex_float(&mut self) -> Result<TokenKind, Diagnostic> {
        let start = self.pos;
        if self.peek_char() != Some('.') {
            self.float_digits(start)?;
        }
        if self.peek_char() == Some('.') {
            self.pos += 1;
            self.float_digits(start)?;
        }
        if matches!(self.peek_char(), Some('e' | 'E')) {
            self.pos += 1;
            if matches!(self.peek_char(), Some('+' | '-')) {
                self.pos += 1;
            }
            self.float_digits(start)?;
        }
        let decimal_end = self.pos;
        let suffix = if matches!(self.peek_char(), Some('f' | 'F')) {
            self.pos += 1;
            FloatSuffix::Float
        } else {
            FloatSuffix::None
        };
        if self.peek_char().is_some_and(is_ident_continue) {
            while self.peek_char().is_some_and(is_ident_continue) {
                self.pos += 1;
            }
            return Err(Diagnostic::at(
                self.span_from(start),
                "invalid floating literal suffix",
            ));
        }
        Ok(TokenKind::Float(FloatLiteralSyntax {
            decimal: self.source[start..decimal_end].replace('_', ""),
            suffix,
            span: self.span_from(start),
        }))
    }

    fn float_digits(&mut self, start: usize) -> Result<(), Diagnostic> {
        let mut saw_digit = false;
        loop {
            match self.peek_char() {
                Some(c) if c.is_ascii_digit() => {
                    self.pos += 1;
                    saw_digit = true;
                }
                Some('_') => {
                    self.pos += 1;
                    if !saw_digit || !self.peek_char().is_some_and(|c| c.is_ascii_digit()) {
                        while self.peek_char().is_some_and(is_ident_continue) {
                            self.pos += 1;
                        }
                        return Err(Diagnostic::at(
                            self.span_from(start),
                            "floating separators must appear between two decimal digits",
                        ));
                    }
                }
                _ => break,
            }
        }
        if saw_digit {
            Ok(())
        } else {
            Err(Diagnostic::at(
                self.span_from(start),
                "floating fraction or exponent must contain a decimal digit",
            ))
        }
    }
}
