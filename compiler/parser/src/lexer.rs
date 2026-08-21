//! Hand-written lexer for the M1 source subset.
//!
//! Produces a flat token vector for the parser. Lexing is fail-fast: the
//! first un-lexable input yields one diagnostic and no tokens at all.

use scoop_ast::{Diagnostic, Span};

/// Token kinds of the M1 subset. `Colon` exists only so the parser can
/// reject return type annotations with a dedicated diagnostic instead of a
/// generic lex error.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum TokenKind {
    Fun,
    Ident(String),
    Str(String),
    LParen,
    RParen,
    LBrace,
    RBrace,
    Comma,
    Semicolon,
    Colon,
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
            TokenKind::Ident(name) => format!("`{name}`"),
            TokenKind::Str(_) => "string literal".to_string(),
            TokenKind::LParen => "`(`".to_string(),
            TokenKind::RParen => "`)`".to_string(),
            TokenKind::LBrace => "`{`".to_string(),
            TokenKind::RBrace => "`}`".to_string(),
            TokenKind::Comma => "`,`".to_string(),
            TokenKind::Semicolon => "`;`".to_string(),
            TokenKind::Colon => "`:`".to_string(),
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
                '"' => self.lex_string()?,
                c if is_ident_start(c) => self.lex_ident(),
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

    /// `pos` is at the opening `/*`. M1 block comments do not nest.
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

    fn lex_ident(&mut self) -> TokenKind {
        let start = self.pos;
        while let Some(c) = self.peek_char() {
            if !is_ident_continue(c) {
                break;
            }
            self.pos += c.len_utf8();
        }
        let text = &self.source[start..self.pos];
        if text == "fun" {
            TokenKind::Fun
        } else {
            TokenKind::Ident(text.to_string())
        }
    }

    /// `pos` is at the opening `"`. The returned string is unescaped; M1
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
