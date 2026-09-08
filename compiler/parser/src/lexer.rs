//! Hand-written lexer for the M6 source subset.
//!
//! Produces a flat token vector for the parser. Invalid characters and
//! recoverable literal errors are skipped so one pass can report multiple
//! independent lexical diagnostics. An unterminated block comment consumes
//! the rest of the file and therefore ends recovery naturally.

use scoop_ast::{Diagnostic, IntegerRadix, IntegerSuffix, Span};

mod token;

pub(crate) use token::{IntegerLiteralLexeme, Token, TokenKind};

pub(crate) fn lex(source: &str) -> (Vec<Token>, Vec<Diagnostic>) {
    Lexer::new(source).run()
}

struct Lexer<'a> {
    source: &'a str,
    pos: usize,
    newline_before: bool,
}

impl<'a> Lexer<'a> {
    fn new(source: &'a str) -> Self {
        Lexer {
            source,
            pos: 0,
            newline_before: false,
        }
    }

    fn run(mut self) -> (Vec<Token>, Vec<Diagnostic>) {
        let mut tokens = Vec::new();
        let mut diagnostics = Vec::new();
        loop {
            if let Err(diagnostic) = self.skip_trivia() {
                diagnostics.push(diagnostic);
                tokens.push(Token {
                    kind: TokenKind::Eof,
                    span: self.span_from(self.pos),
                    newline_before: std::mem::take(&mut self.newline_before),
                });
                return (tokens, diagnostics);
            }
            let start = self.pos;
            let newline_before = std::mem::take(&mut self.newline_before);
            let Some(c) = self.peek_char() else {
                tokens.push(Token {
                    kind: TokenKind::Eof,
                    span: self.span_from(start),
                    newline_before,
                });
                return (tokens, diagnostics);
            };
            let kind = match self.lex_token(c, start) {
                Ok(kind) => kind,
                Err(diagnostic) => {
                    diagnostics.push(diagnostic);
                    // String failures leave the cursor either inside the
                    // literal or at its opening quote (`f"..."`). Skip the
                    // remainder to avoid tokenizing its contents as code.
                    if c == '"' || self.peek_char() == Some('"') {
                        self.skip_bad_string();
                    }
                    continue;
                }
            };
            tokens.push(Token {
                kind,
                span: self.span_from(start),
                newline_before,
            });
        }
    }

    fn lex_token(&mut self, c: char, start: usize) -> Result<TokenKind, Diagnostic> {
        let kind = match c {
            '(' => {
                self.pos += 1;
                TokenKind::LParen
            }
            ')' => {
                self.pos += 1;
                TokenKind::RParen
            }
            '{' => {
                self.pos += 1;
                TokenKind::LBrace
            }
            '}' => {
                self.pos += 1;
                TokenKind::RBrace
            }
            '[' => {
                self.pos += 1;
                TokenKind::LBracket
            }
            ']' => {
                self.pos += 1;
                TokenKind::RBracket
            }
            ',' => {
                self.pos += 1;
                TokenKind::Comma
            }
            ';' => {
                self.pos += 1;
                TokenKind::Semicolon
            }
            ':' => {
                self.pos += 1;
                if self.peek_char() == Some(':') {
                    self.pos += 1;
                    TokenKind::DoubleColon
                } else {
                    TokenKind::Colon
                }
            }
            '@' => {
                self.pos += 1;
                TokenKind::At
            }
            // `..` is the rest marker in pattern positions (spec 4.6);
            // the range operator shares the token but only appears in
            // expression positions (a dedicated diagnostic in M5).
            '.' => {
                self.pos += 1;
                if self.eat('.') {
                    if self.eat('<') {
                        TokenKind::DotDotLess
                    } else {
                        TokenKind::DotDot
                    }
                } else {
                    TokenKind::Dot
                }
            }
            '?' => {
                self.pos += 1;
                if self.eat('.') {
                    TokenKind::QuestionDot
                } else if self.eat(':') {
                    TokenKind::QuestionColon
                } else {
                    TokenKind::Question
                }
            }
            '+' => {
                self.pos += 1;
                if self.eat('+') {
                    TokenKind::PlusPlus
                } else if self.eat('=') {
                    TokenKind::PlusEqual
                } else {
                    TokenKind::Plus
                }
            }
            '-' => {
                self.pos += 1;
                // `->` separates a `when` arm's pattern from its body.
                // No valid expression has `-` immediately before `>`.
                if self.eat('>') {
                    TokenKind::Arrow
                } else if self.eat('-') {
                    TokenKind::MinusMinus
                } else if self.eat('=') {
                    TokenKind::MinusEqual
                } else {
                    TokenKind::Minus
                }
            }
            '*' => {
                self.pos += 1;
                if self.eat('=') {
                    TokenKind::StarEqual
                } else {
                    TokenKind::Star
                }
            }
            // No `/*` ambiguity: comment openers are consumed by
            // `skip_trivia`, so a `/` reaching here is always division.
            '/' => {
                self.pos += 1;
                if self.eat('=') {
                    TokenKind::SlashEqual
                } else {
                    TokenKind::Slash
                }
            }
            '%' => {
                self.pos += 1;
                if self.eat('=') {
                    TokenKind::PercentEqual
                } else {
                    TokenKind::Percent
                }
            }
            '!' => {
                self.pos += 1;
                if self.source[self.pos..].starts_with("in")
                    && self.source[self.pos + 2..]
                        .chars()
                        .next()
                        .is_none_or(|next| !is_ident_continue(next))
                {
                    self.pos += 2;
                    TokenKind::BangIn
                } else if self.eat('=') {
                    if self.eat('=') {
                        TokenKind::BangEqualEqual
                    } else {
                        TokenKind::BangEqual
                    }
                } else {
                    TokenKind::Bang
                }
            }
            '=' => {
                self.pos += 1;
                if self.eat('=') {
                    if self.eat('=') {
                        TokenKind::EqualEqualEqual
                    } else {
                        TokenKind::EqualEqual
                    }
                } else {
                    TokenKind::Equal
                }
            }
            '<' => {
                self.pos += 1;
                if self.eat('=') {
                    TokenKind::LessEqual
                } else {
                    TokenKind::Less
                }
            }
            '>' => {
                self.pos += 1;
                if self.eat('=') {
                    TokenKind::GreaterEqual
                } else {
                    TokenKind::Greater
                }
            }
            '&' if self.source[self.pos + 1..].starts_with('&') => {
                self.pos += 2;
                TokenKind::AmpAmp
            }
            '|' if self.source[self.pos + 1..].starts_with('|') => {
                self.pos += 2;
                TokenKind::PipePipe
            }
            '"' => return self.lex_string(),
            c if c.is_ascii_digit() => return self.lex_int(),
            c if is_ident_start(c) => return self.lex_ident(),
            c => {
                self.pos += c.len_utf8();
                return Err(Diagnostic::at(
                    self.span_from(start),
                    format!("unexpected character `{c}`"),
                ));
            }
        };
        Ok(kind)
    }

    /// Consume the rest of a malformed string through its closing quote or
    /// leave the newline for trivia handling. The opening quote may still be
    /// current for an unsupported interpolation prefix.
    fn skip_bad_string(&mut self) {
        if self.peek_char() == Some('"') {
            self.pos += 1;
        }
        while let Some(c) = self.peek_char() {
            match c {
                '\n' => return,
                '"' => {
                    self.pos += 1;
                    return;
                }
                '\\' => {
                    self.pos += 1;
                    if let Some(escaped) = self.peek_char() {
                        if escaped != '\n' {
                            self.pos += escaped.len_utf8();
                        }
                    }
                }
                _ => self.pos += c.len_utf8(),
            }
        }
    }

    fn peek_char(&self) -> Option<char> {
        self.source[self.pos..].chars().next()
    }

    /// Consumes `expected` if it is the next char; returns whether it did.
    fn eat(&mut self, expected: char) -> bool {
        if self.peek_char() == Some(expected) {
            self.pos += expected.len_utf8();
            true
        } else {
            false
        }
    }

    fn span_from(&self, start: usize) -> Span {
        Span::new(start as u32, self.pos as u32)
    }

    /// Skips whitespace and comments, recording newlines in
    /// `newline_before`.
    fn skip_trivia(&mut self) -> Result<(), Diagnostic> {
        loop {
            match self.peek_char() {
                Some(' ' | '\t' | '\r') => self.pos += 1,
                Some('\n') => {
                    self.pos += 1;
                    self.newline_before = true;
                }
                Some('/') if self.source[self.pos + 1..].starts_with('/') => {
                    while let Some(c) = self.peek_char() {
                        if c == '\n' {
                            break;
                        }
                        self.pos += c.len_utf8();
                    }
                }
                Some('/') if self.source[self.pos + 1..].starts_with('*') => {
                    self.skip_block_comment()?;
                }
                _ => return Ok(()),
            }
        }
    }

    /// `pos` is at the opening `/*`. M2 block comments do not nest.
    fn skip_block_comment(&mut self) -> Result<(), Diagnostic> {
        let start = self.pos;
        self.pos += 2;
        loop {
            match self.peek_char() {
                None => {
                    return Err(Diagnostic::at(
                        self.span_from(start),
                        "unterminated block comment",
                    ));
                }
                Some('*') if self.source[self.pos + 1..].starts_with('/') => {
                    self.pos += 2;
                    return Ok(());
                }
                Some('\n') => {
                    self.pos += 1;
                    self.newline_before = true;
                }
                Some(c) => self.pos += c.len_utf8(),
            }
        }
    }

    /// `pos` is at the first digit.
    fn lex_int(&mut self) -> Result<TokenKind, Diagnostic> {
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

    fn lex_ident(&mut self) -> Result<TokenKind, Diagnostic> {
        let start = self.pos;
        while let Some(c) = self.peek_char() {
            if !is_ident_continue(c) {
                break;
            }
            self.pos += c.len_utf8();
        }
        let text = &self.source[start..self.pos];
        // `f"..."` (string interpolation) lexes as `f` + a string literal;
        // catch it here for a dedicated diagnostic.
        if text == "f" && self.peek_char() == Some('"') {
            return Err(Diagnostic::at(
                self.span_from(start),
                "string interpolation is not supported yet (milestone M3)",
            ));
        }
        let kind = match text {
            "package" => TokenKind::Package,
            "import" => TokenKind::Import,
            "suspend" => TokenKind::Suspend,
            "vararg" => TokenKind::Vararg,
            "fun" => TokenKind::Fun,
            "struct" => TokenKind::Struct,
            "enum" => TokenKind::Enum,
            "class" => TokenKind::Class,
            "interface" => TokenKind::Interface,
            "val" => TokenKind::Val,
            "var" => TokenKind::Var,
            "if" => TokenKind::If,
            "else" => TokenKind::Else,
            "when" => TokenKind::When,
            "while" => TokenKind::While,
            "for" => TokenKind::For,
            "break" => TokenKind::Break,
            "continue" => TokenKind::Continue,
            "return" => TokenKind::Return,
            "true" => TokenKind::True,
            "false" => TokenKind::False,
            "this" => TokenKind::This,
            "is" => TokenKind::Is,
            "in" => TokenKind::In,
            "infix" => TokenKind::Infix,
            "as" => TokenKind::As,
            _ => TokenKind::Ident(text.to_string()),
        };
        Ok(kind)
    }

    /// `pos` is at the opening `"`. The returned string is unescaped; M2
    /// supports `\n`, `\t`, `\\` and `\"` only.
    fn lex_string(&mut self) -> Result<TokenKind, Diagnostic> {
        let start = self.pos;
        self.pos += 1;
        let mut value = String::new();
        loop {
            match self.peek_char() {
                None | Some('\n') => {
                    return Err(Diagnostic::at(
                        self.span_from(start),
                        "unterminated string literal",
                    ));
                }
                Some('"') => {
                    self.pos += 1;
                    return Ok(TokenKind::Str(value));
                }
                Some('\\') => {
                    let escape_start = self.pos;
                    self.pos += 1;
                    match self.peek_char() {
                        Some('n') => {
                            self.pos += 1;
                            value.push('\n');
                        }
                        Some('t') => {
                            self.pos += 1;
                            value.push('\t');
                        }
                        Some('\\') => {
                            self.pos += 1;
                            value.push('\\');
                        }
                        Some('"') => {
                            self.pos += 1;
                            value.push('"');
                        }
                        Some(c) => {
                            self.pos += c.len_utf8();
                            return Err(Diagnostic::at(
                                Span::new(escape_start as u32, self.pos as u32),
                                format!("unsupported escape sequence `\\{c}`"),
                            ));
                        }
                        None => {
                            return Err(Diagnostic::at(
                                self.span_from(start),
                                "unterminated string literal",
                            ));
                        }
                    }
                }
                Some(c) => {
                    self.pos += c.len_utf8();
                    value.push(c);
                }
            }
        }
    }
}

fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}

fn is_ident_continue(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}
