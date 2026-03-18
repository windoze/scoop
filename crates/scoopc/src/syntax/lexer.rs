//! Scoop lexer（词法分析）。
//!
//! 当前目标：
//! - 支持 Scoop 规范里用到的基础 token（关键字、符号、数字、字符串、注释）
//! - 产出 `Vec<Token>`，供后续 parser 使用

use miette::Diagnostic;
use thiserror::Error;

use crate::span::Span;

use super::token::{Keyword, StringKind, Symbol, Token, TokenKind};

#[derive(Debug, Error, Diagnostic)]
pub enum LexError {
    #[error("非法字符：{ch:?}")]
    #[diagnostic(code(scoop::lex::invalid_char))]
    InvalidChar {
        ch: char,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("未闭合的块注释")]
    #[diagnostic(code(scoop::lex::unterminated_block_comment))]
    UnterminatedBlockComment {
        #[label("从这里开始")]
        span: miette::SourceSpan,
    },

    #[error("未闭合的字符串字面量")]
    #[diagnostic(code(scoop::lex::unterminated_string))]
    UnterminatedString {
        #[label("从这里开始")]
        span: miette::SourceSpan,
    },
}

/// 词法分析入口：将 `text` 转换为 token 序列。
pub fn lex(text: &str) -> Result<Vec<Token>, LexError> {
    Lexer::new(text).lex_all()
}

struct Lexer<'a> {
    text: &'a str,
    pos: usize,
}

impl<'a> Lexer<'a> {
    fn new(text: &'a str) -> Self {
        Self { text, pos: 0 }
    }

    fn lex_all(mut self) -> Result<Vec<Token>, LexError> {
        let mut tokens = Vec::new();
        loop {
            self.skip_trivia()?;
            let start = self.pos;

            if self.is_eof() {
                tokens.push(Token {
                    kind: TokenKind::Eof,
                    span: Span::new(self.pos, self.pos),
                });
                return Ok(tokens);
            }

            let ch = self.peek_char().unwrap();

            // identifiers / keywords
            if is_ident_start(ch) {
                let kind = self.lex_ident_or_keyword()?;
                tokens.push(Token {
                    kind,
                    span: Span::new(start, self.pos),
                });
                continue;
            }

            // numbers
            if ch.is_ascii_digit() {
                self.lex_int_literal();
                tokens.push(Token {
                    kind: TokenKind::IntLiteral,
                    span: Span::new(start, self.pos),
                });
                continue;
            }

            // strings: "..." or """...""", optionally prefixed with `f`
            if ch == '"' {
                let string_kind = self.lex_string(false)?;
                tokens.push(Token {
                    kind: TokenKind::StringLiteral(string_kind),
                    span: Span::new(start, self.pos),
                });
                continue;
            }

            // symbols (including multi-char)
            if let Some(sym) = self.lex_symbol() {
                tokens.push(Token {
                    kind: TokenKind::Symbol(sym),
                    span: Span::new(start, self.pos),
                });
                continue;
            }

            return Err(LexError::InvalidChar {
                ch,
                span: Span::new(start, start + ch.len_utf8()).into(),
            });
        }
    }

    fn skip_trivia(&mut self) -> Result<(), LexError> {
        loop {
            // whitespace
            while self.peek_char().is_some_and(|c| c.is_whitespace()) {
                self.bump_char();
            }

            // line comment
            if self.peek_bytes2() == Some([b'/', b'/']) {
                self.bump_bytes(2);
                while self.peek_char().is_some_and(|c| c != '\n') {
                    self.bump_char();
                }
                continue;
            }

            // block comment (non-nested)
            if self.peek_bytes2() == Some([b'/', b'*']) {
                let start = self.pos;
                self.bump_bytes(2);
                loop {
                    if self.is_eof() {
                        return Err(LexError::UnterminatedBlockComment {
                            span: Span::new(start, self.pos).into(),
                        });
                    }
                    if self.peek_bytes2() == Some([b'*', b'/']) {
                        self.bump_bytes(2);
                        break;
                    }
                    self.bump_char();
                }
                continue;
            }

            return Ok(());
        }
    }

    fn lex_ident_or_keyword(&mut self) -> Result<TokenKind, LexError> {
        let start = self.pos;
        self.bump_char(); // first char
        while self.peek_char().is_some_and(is_ident_continue) {
            self.bump_char();
        }
        let ident = &self.text[start..self.pos];

        // f"..." / f"""..."""
        if ident == "f" && self.peek_char() == Some('"') {
            let string_kind = self.lex_string(true)?;
            return Ok(TokenKind::StringLiteral(string_kind));
        }

        let kw = match ident {
            "open" => Some(Keyword::Open),
            "abstract" => Some(Keyword::Abstract),
            "sealed" => Some(Keyword::Sealed),
            "package" => Some(Keyword::Package),
            "import" => Some(Keyword::Import),
            "fun" => Some(Keyword::Fun),
            "val" => Some(Keyword::Val),
            "var" => Some(Keyword::Var),
            "class" => Some(Keyword::Class),
            "interface" => Some(Keyword::Interface),
            "struct" => Some(Keyword::Struct),
            "enum" => Some(Keyword::Enum),
            "effect" => Some(Keyword::Effect),
            "handle" => Some(Keyword::Handle),
            "with" => Some(Keyword::With),
            "perform" => Some(Keyword::Perform),
            "try" => Some(Keyword::Try),
            "catch" => Some(Keyword::Catch),
            "finally" => Some(Keyword::Finally),
            "async" => Some(Keyword::Async),
            "await" => Some(Keyword::Await),
            "return" => Some(Keyword::Return),
            "if" => Some(Keyword::If),
            "else" => Some(Keyword::Else),
            "when" => Some(Keyword::When),
            "is" => Some(Keyword::Is),
            "as" => {
                // `as?` safe cast：要求 `?` 紧跟在 as 后面。
                if self.peek_char() == Some('?') {
                    self.bump_char();
                    Some(Keyword::AsQ)
                } else {
                    Some(Keyword::As)
                }
            }
            _ => None,
        };

        Ok(match kw {
            Some(k) => TokenKind::Keyword(k),
            None => TokenKind::Ident,
        })
    }

    fn lex_int_literal(&mut self) {
        self.bump_char();
        while self.peek_char().is_some_and(|c| c.is_ascii_digit() || c == '_') {
            self.bump_char();
        }
    }

    fn lex_string(&mut self, interpolated: bool) -> Result<StringKind, LexError> {
        // 当前位置应指向第一个 `"`
        let start = self.pos;

        // raw string: """ ... """
        if self.peek_bytes3() == Some([b'"', b'"', b'"']) {
            self.bump_bytes(3);
            loop {
                if self.is_eof() {
                    return Err(LexError::UnterminatedString {
                        span: Span::new(start, self.pos).into(),
                    });
                }
                if self.peek_bytes3() == Some([b'"', b'"', b'"']) {
                    self.bump_bytes(3);
                    break;
                }
                self.bump_char();
            }
            return Ok(StringKind::Raw { interpolated });
        }

        // normal string: " ... "
        debug_assert_eq!(self.peek_char(), Some('"'));
        self.bump_char(); // opening "

        while let Some(ch) = self.peek_char() {
            match ch {
                '"' => {
                    self.bump_char();
                    return Ok(StringKind::Normal { interpolated });
                }
                '\\' => {
                    // escape sequence: consume '\' + next char if present
                    self.bump_char();
                    if self.is_eof() {
                        return Err(LexError::UnterminatedString {
                            span: Span::new(start, self.pos).into(),
                        });
                    }
                    self.bump_char();
                }
                '\n' => {
                    // Kotlin-like：普通字符串不允许裸换行
                    return Err(LexError::UnterminatedString {
                        span: Span::new(start, self.pos).into(),
                    });
                }
                _ => {
                    self.bump_char();
                }
            }
        }

        Err(LexError::UnterminatedString {
            span: Span::new(start, self.pos).into(),
        })
    }

    fn lex_symbol(&mut self) -> Option<Symbol> {
        // multi-char first (longest match)
        if self.peek_bytes2() == Some([b'-', b'>']) {
            self.bump_bytes(2);
            return Some(Symbol::Arrow);
        }
        if self.peek_bytes2() == Some([b'=', b'=']) {
            self.bump_bytes(2);
            return Some(Symbol::EqEq);
        }
        if self.peek_bytes2() == Some([b'!', b'=']) {
            self.bump_bytes(2);
            return Some(Symbol::BangEq);
        }
        if self.peek_bytes2() == Some([b'<', b'=']) {
            self.bump_bytes(2);
            return Some(Symbol::LtEq);
        }
        if self.peek_bytes2() == Some([b'>', b'=']) {
            self.bump_bytes(2);
            return Some(Symbol::GtEq);
        }
        if self.peek_bytes2() == Some([b'&', b'&']) {
            self.bump_bytes(2);
            return Some(Symbol::AndAnd);
        }
        if self.peek_bytes2() == Some([b'|', b'|']) {
            self.bump_bytes(2);
            return Some(Symbol::OrOr);
        }
        if self.peek_bytes2() == Some([b'!', b'!']) {
            self.bump_bytes(2);
            return Some(Symbol::BangBang);
        }
        if self.peek_bytes2() == Some([b'?', b'.']) {
            self.bump_bytes(2);
            return Some(Symbol::QuestionDot);
        }
        if self.peek_bytes2() == Some([b'?', b':']) {
            self.bump_bytes(2);
            return Some(Symbol::Elvis);
        }

        // single-char
        let ch = self.peek_char()?;
        let sym = match ch {
            '@' => Symbol::At,
            '(' => Symbol::LParen,
            ')' => Symbol::RParen,
            '{' => Symbol::LBrace,
            '}' => Symbol::RBrace,
            '[' => Symbol::LBracket,
            ']' => Symbol::RBracket,
            ',' => Symbol::Comma,
            ':' => Symbol::Colon,
            ';' => Symbol::Semicolon,
            '.' => Symbol::Dot,
            '+' => Symbol::Plus,
            '-' => Symbol::Minus,
            '*' => Symbol::Star,
            '/' => Symbol::Slash,
            '=' => Symbol::Eq,
            '<' => Symbol::Lt,
            '>' => Symbol::Gt,
            '!' => Symbol::Bang,
            '?' => Symbol::Question,
            _ => return None,
        };
        self.bump_char();
        Some(sym)
    }

    fn is_eof(&self) -> bool {
        self.pos >= self.text.len()
    }

    fn peek_char(&self) -> Option<char> {
        self.text[self.pos..].chars().next()
    }

    fn bump_char(&mut self) -> Option<char> {
        let ch = self.peek_char()?;
        self.pos += ch.len_utf8();
        Some(ch)
    }

    fn bump_bytes(&mut self, n: usize) {
        self.pos = (self.pos + n).min(self.text.len());
    }

    fn peek_bytes2(&self) -> Option<[u8; 2]> {
        let bytes = self.text.as_bytes();
        if self.pos + 2 <= bytes.len() {
            Some([bytes[self.pos], bytes[self.pos + 1]])
        } else {
            None
        }
    }

    fn peek_bytes3(&self) -> Option<[u8; 3]> {
        let bytes = self.text.as_bytes();
        if self.pos + 3 <= bytes.len() {
            Some([bytes[self.pos], bytes[self.pos + 1], bytes[self.pos + 2]])
        } else {
            None
        }
    }
}

fn is_ident_start(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphabetic()
}

fn is_ident_continue(ch: char) -> bool {
    is_ident_start(ch) || ch.is_ascii_digit()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(src: &str) -> Vec<TokenKind> {
        lex(src).unwrap().into_iter().map(|t| t.kind).collect()
    }

    /// 一个极简、可复现的伪随机数生成器（避免引入 `rand` 依赖）。
    #[derive(Clone)]
    struct XorShift64 {
        state: u64,
    }

    impl XorShift64 {
        fn new(seed: u64) -> Self {
            // xorshift 在 0 种子下会卡住；这里做一次扰动。
            let seed = if seed == 0 { 0x9E37_79B9_7F4A_7C15 } else { seed };
            Self { state: seed }
        }

        fn next_u64(&mut self) -> u64 {
            // https://en.wikipedia.org/wiki/Xorshift
            let mut x = self.state;
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            self.state = x;
            x
        }

        fn gen_usize(&mut self, upper_exclusive: usize) -> usize {
            if upper_exclusive == 0 {
                return 0;
            }
            (self.next_u64() as usize) % upper_exclusive
        }
    }

    fn gen_source(rng: &mut XorShift64, max_len: usize) -> String {
        // 尽量覆盖 lexer 关心的字符：注释/字符串/括号/运算符/空白等。
        const CHARS: &[char] = &[
            ' ', '\t', '\n', '\r', '_', '@', '.', ',', ':', ';', '(', ')', '{', '}', '[', ']',
            '+', '-', '*', '/', '=', '<', '>', '!', '?', '&', '|', '"', '\\', 'a', 'b', 'c',
            'x', 'y', 'z', 'A', 'B', 'C', '0', '1', '2', '3', '9', '中', 'é',
        ];

        let len = rng.gen_usize(max_len + 1);
        let mut s = String::with_capacity(len);
        for _ in 0..len {
            let ch = CHARS[rng.gen_usize(CHARS.len())];
            s.push(ch);
        }
        s
    }

    #[test]
    fn lex_keywords_and_idents() {
        let ks = kinds("package p\nfun f() { val x = 1 }");
        assert!(ks.contains(&TokenKind::Keyword(Keyword::Package)));
        assert!(ks.contains(&TokenKind::Keyword(Keyword::Fun)));
        assert!(ks.contains(&TokenKind::Keyword(Keyword::Val)));
        assert!(ks.contains(&TokenKind::Ident));
        assert!(ks.contains(&TokenKind::IntLiteral));
        assert_eq!(ks.last().copied(), Some(TokenKind::Eof));
    }

    #[test]
    fn lex_comments_are_skipped() {
        let ks = kinds("val x = 1 // comment\nval y = 2 /* block */");
        let vals = ks
            .into_iter()
            .filter(|k| *k == TokenKind::Keyword(Keyword::Val))
            .count();
        assert_eq!(vals, 2);
    }

    #[test]
    fn lex_strings_normal_and_raw() {
        let ks = kinds(r#"val a = "x"; val b = """y""" "#);
        assert!(ks.contains(&TokenKind::StringLiteral(StringKind::Normal {
            interpolated: false
        })));
        assert!(ks.contains(&TokenKind::StringLiteral(StringKind::Raw {
            interpolated: false
        })));
    }

    #[test]
    fn lex_f_strings() {
        let ks = kinds(r#"val a = f"hi {x}"; val b = f"""raw {x}""" "#);
        assert!(ks.contains(&TokenKind::StringLiteral(StringKind::Normal {
            interpolated: true
        })));
        assert!(ks.contains(&TokenKind::StringLiteral(StringKind::Raw {
            interpolated: true
        })));
    }

    #[test]
    fn lex_asq() {
        let ks = kinds("x as? T");
        assert!(ks.contains(&TokenKind::Keyword(Keyword::AsQ)));
    }

    #[test]
    fn lex_bangbang_and_elvis() {
        let ks = kinds("x!! ?: y");
        assert!(ks.contains(&TokenKind::Symbol(Symbol::BangBang)));
        assert!(ks.contains(&TokenKind::Symbol(Symbol::Elvis)));
    }

    #[test]
    fn lex_at_annotation() {
        let ks = kinds("@Unsafe fun f() {}");
        assert!(ks.contains(&TokenKind::Symbol(Symbol::At)));
        assert!(ks.contains(&TokenKind::Keyword(Keyword::Fun)));
        assert!(ks.contains(&TokenKind::Ident));
    }

    /// 崩溃防线：确保 lexer 对“任意输入”都不会 panic。
    ///
    /// 这不是高强度 fuzz（不追求覆盖率/错误恢复质量），只要能尽早发现 panic 即可。
    #[test]
    fn lexer_random_inputs_do_not_panic() {
        let mut rng = XorShift64::new(0xC0FF_EE12_3456_789A);
        for i in 0..2_000usize {
            let src = gen_source(&mut rng, 256);
            let res = std::panic::catch_unwind(|| {
                let _ = lex(&src);
            });
            if res.is_err() {
                panic!("lexer panic（iter={i}）: {src:?}");
            }
        }
    }
}
