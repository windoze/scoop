//! Hand-written lexer for the M6 source subset.
//!
//! Produces a flat token vector for the parser. Lexing is fail-fast: the
//! first un-lexable input yields one diagnostic and no tokens at all.

use scoop_ast::{Diagnostic, Span};

/// Token kinds of the M9 subset. Reserved words are dedicated variants;
/// `Unit` deliberately stays an [`TokenKind::Ident`] (spec section 4.3:
/// it is not a reserved word). Words that only matter in specific
/// positions (`open`, `abstract`, `override`, `super`, `object`,
/// `sealed`, `companion`, `init`, `constructor`, `try`, `catch`,
/// `finally`, `throw`) stay identifiers too — the parser matches them
/// by text where they are meaningful.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum TokenKind {
    Fun,
    Struct,
    Enum,
    Class,
    Interface,
    Val,
    Var,
    If,
    Else,
    When,
    While,
    Return,
    True,
    False,
    This,
    Is,
    As,
    Ident(String),
    Str(String),
    Int(i64),
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Comma,
    Semicolon,
    Colon,
    At,
    Dot,
    DotDot,
    Arrow,
    Question,
    QuestionDot,
    QuestionColon,
    Plus,
    Minus,
    Star,
    Slash,
    Bang,
    BangEqual,
    BangEqualEqual,
    Equal,
    EqualEqual,
    EqualEqualEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    AmpAmp,
    PipePipe,
    Eof,
}

#[derive(Debug, Clone)]
pub(crate) struct Token {
    pub kind: TokenKind,
    pub span: Span,
    /// True when at least one newline appeared between the previous token
    /// and this one (newlines inside comments count). The parser uses this
    /// as the statement separator.
    pub newline_before: bool,
}

impl Token {
    /// Human-readable token form for diagnostics, e.g. `` `)` `` or
    /// `end of file`.
    pub fn describe(&self) -> String {
        match &self.kind {
            TokenKind::Fun => "`fun`".to_string(),
            TokenKind::Struct => "`struct`".to_string(),
            TokenKind::Enum => "`enum`".to_string(),
            TokenKind::Class => "`class`".to_string(),
            TokenKind::Interface => "`interface`".to_string(),
            TokenKind::Val => "`val`".to_string(),
            TokenKind::Var => "`var`".to_string(),
            TokenKind::If => "`if`".to_string(),
            TokenKind::Else => "`else`".to_string(),
            TokenKind::When => "`when`".to_string(),
            TokenKind::While => "`while`".to_string(),
            TokenKind::Return => "`return`".to_string(),
            TokenKind::True => "`true`".to_string(),
            TokenKind::False => "`false`".to_string(),
            TokenKind::This => "`this`".to_string(),
            TokenKind::Is => "`is`".to_string(),
            TokenKind::As => "`as`".to_string(),
            TokenKind::Ident(name) => format!("`{name}`"),
            TokenKind::Str(_) => "string literal".to_string(),
            TokenKind::Int(_) => "integer literal".to_string(),
            TokenKind::LParen => "`(`".to_string(),
            TokenKind::RParen => "`)`".to_string(),
            TokenKind::LBrace => "`{`".to_string(),
            TokenKind::RBrace => "`}`".to_string(),
            TokenKind::LBracket => "`[`".to_string(),
            TokenKind::RBracket => "`]`".to_string(),
            TokenKind::Comma => "`,`".to_string(),
            TokenKind::Semicolon => "`;`".to_string(),
            TokenKind::Colon => "`:`".to_string(),
            TokenKind::At => "`@`".to_string(),
            TokenKind::Dot => "`.`".to_string(),
            TokenKind::DotDot => "`..`".to_string(),
            TokenKind::Arrow => "`->`".to_string(),
            TokenKind::Question => "`?`".to_string(),
            TokenKind::QuestionDot => "`?.`".to_string(),
            TokenKind::QuestionColon => "`?:`".to_string(),
            TokenKind::Plus => "`+`".to_string(),
            TokenKind::Minus => "`-`".to_string(),
            TokenKind::Star => "`*`".to_string(),
            TokenKind::Slash => "`/`".to_string(),
            TokenKind::Bang => "`!`".to_string(),
            TokenKind::BangEqual => "`!=`".to_string(),
            TokenKind::BangEqualEqual => "`!==`".to_string(),
            TokenKind::Equal => "`=`".to_string(),
            TokenKind::EqualEqual => "`==`".to_string(),
            TokenKind::EqualEqualEqual => "`===`".to_string(),
            TokenKind::Less => "`<`".to_string(),
            TokenKind::LessEqual => "`<=`".to_string(),
            TokenKind::Greater => "`>`".to_string(),
            TokenKind::GreaterEqual => "`>=`".to_string(),
            TokenKind::AmpAmp => "`&&`".to_string(),
            TokenKind::PipePipe => "`||`".to_string(),
            TokenKind::Eof => "end of file".to_string(),
        }
    }
}

pub(crate) fn lex(source: &str) -> Result<Vec<Token>, Diagnostic> {
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

    fn run(mut self) -> Result<Vec<Token>, Diagnostic> {
        let mut tokens = Vec::new();
        loop {
            self.skip_trivia()?;
            let start = self.pos;
            let newline_before = std::mem::take(&mut self.newline_before);
            let Some(c) = self.peek_char() else {
                tokens.push(Token {
                    kind: TokenKind::Eof,
                    span: self.span_from(start),
                    newline_before,
                });
                return Ok(tokens);
            };
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
                    TokenKind::Colon
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
                        TokenKind::DotDot
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
                    TokenKind::Plus
                }
                '-' => {
                    self.pos += 1;
                    // `->` separates a `when` arm's pattern from its body.
                    // No valid expression has `-` immediately before `>`.
                    if self.eat('>') {
                        TokenKind::Arrow
                    } else {
                        TokenKind::Minus
                    }
                }
                '*' => {
                    self.pos += 1;
                    TokenKind::Star
                }
                // No `/*` ambiguity: comment openers are consumed by
                // `skip_trivia`, so a `/` reaching here is always division.
                '/' => {
                    self.pos += 1;
                    TokenKind::Slash
                }
                '!' => {
                    self.pos += 1;
                    if self.eat('=') {
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
                '"' => self.lex_string()?,
                c if c.is_ascii_digit() => self.lex_int()?,
                c if is_ident_start(c) => self.lex_ident()?,
                c => {
                    self.pos += c.len_utf8();
                    return Err(Diagnostic::at(
                        self.span_from(start),
                        format!("unexpected character `{c}`"),
                    ));
                }
            };
            tokens.push(Token {
                kind,
                span: self.span_from(start),
                newline_before,
            });
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

    /// `pos` is at the first digit. M2 integers are decimal i64 only.
    fn lex_int(&mut self) -> Result<TokenKind, Diagnostic> {
        let start = self.pos;
        while let Some(c) = self.peek_char() {
            if !c.is_ascii_digit() {
                break;
            }
            self.pos += 1;
        }
        let text = &self.source[start..self.pos];
        match text.parse::<i64>() {
            Ok(value) => Ok(TokenKind::Int(value)),
            Err(_) => Err(Diagnostic::at(
                self.span_from(start),
                format!("integer literal `{text}` is out of range (Int is i64)"),
            )),
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
            "return" => TokenKind::Return,
            "true" => TokenKind::True,
            "false" => TokenKind::False,
            "this" => TokenKind::This,
            "is" => TokenKind::Is,
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
