//! Scoop lexer（词法分析）。
//!
//! 当前目标：
//! - 支持 Scoop 规范里用到的基础 token（关键字、符号、数字、字符串、注释）
//! - 产出 `Vec<Token>`，供后续 parser 使用

use miette::Diagnostic;
use thiserror::Error;

use crate::span::Span;

use super::char_literal::parse_char_literal;
use super::float_literal::{FloatLiteralParseError, parse_float_literal_checked};
use super::int_literal::{IntLiteralParseError, parse_int_literal_checked};
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

    #[error("未闭合的 Char 字面量")]
    #[diagnostic(code(scoop::lex::unterminated_char_literal))]
    UnterminatedCharLiteral {
        #[label("从这里开始")]
        span: miette::SourceSpan,
    },

    #[error("非法 Char 字面量：{reason}")]
    #[diagnostic(code(scoop::lex::invalid_char_literal))]
    InvalidCharLiteral {
        reason: &'static str,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("非法 Int 字面量：{reason}")]
    #[diagnostic(code(scoop::lex::invalid_int_literal))]
    InvalidIntLiteral {
        reason: &'static str,
        #[label("这里")]
        span: miette::SourceSpan,
    },

    #[error("非法 Float 字面量：{reason}")]
    #[diagnostic(code(scoop::lex::invalid_float_literal))]
    InvalidFloatLiteral {
        reason: &'static str,
        #[label("这里")]
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
                let kind = self.lex_number_literal()?;
                tokens.push(Token {
                    kind,
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

            if ch == '\'' {
                self.lex_char_literal()?;
                tokens.push(Token {
                    kind: TokenKind::CharLiteral,
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
            "public" => Some(Keyword::Public),
            "internal" => Some(Keyword::Internal),
            "private" => Some(Keyword::Private),
            "open" => Some(Keyword::Open),
            "abstract" => Some(Keyword::Abstract),
            "sealed" => Some(Keyword::Sealed),
            "inline" => Some(Keyword::Inline),
            "override" => Some(Keyword::Override),
            "const" => Some(Keyword::Const),
            "vararg" => Some(Keyword::Vararg),
            "annotation" => Some(Keyword::Annotation),
            "package" => Some(Keyword::Package),
            "import" => Some(Keyword::Import),
            "typealias" => Some(Keyword::Typealias),
            "fun" => Some(Keyword::Fun),
            "val" => Some(Keyword::Val),
            "var" => Some(Keyword::Var),
            "class" => Some(Keyword::Class),
            "interface" => Some(Keyword::Interface),
            "struct" => Some(Keyword::Struct),
            "enum" => Some(Keyword::Enum),
            "effect" => Some(Keyword::Effect),
            "object" => Some(Keyword::Object),
            "companion" => Some(Keyword::Companion),
            "handle" => Some(Keyword::Handle),
            "with" => Some(Keyword::With),
            "perform" => Some(Keyword::Perform),
            "try" => Some(Keyword::Try),
            "catch" => Some(Keyword::Catch),
            "finally" => Some(Keyword::Finally),
            "async" => Some(Keyword::Async),
            "do" => Some(Keyword::Do),
            "return" => Some(Keyword::Return),
            "comptime" => Some(Keyword::Comptime),
            "if" => Some(Keyword::If),
            "else" => Some(Keyword::Else),
            "when" => Some(Keyword::When),
            "for" => Some(Keyword::For),
            "in" => Some(Keyword::In),
            "out" => Some(Keyword::Out),
            "where" => Some(Keyword::Where),
            "while" => Some(Keyword::While),
            "break" => Some(Keyword::Break),
            "continue" => Some(Keyword::Continue),
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

    fn lex_number_literal(&mut self) -> Result<TokenKind, LexError> {
        let start = self.pos;
        let Some(first) = self.bump_char() else {
            return Ok(TokenKind::IntLiteral);
        };
        if first == '0'
            && self
                .peek_char()
                .is_some_and(|ch| matches!(ch, 'x' | 'X' | 'b' | 'B'))
        {
            self.bump_char();
            while self
                .peek_char()
                .is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_')
            {
                self.bump_char();
            }
            return self.finish_int_literal(start);
        }
        self.lex_decimal_digits_candidate();

        let mut is_float = false;
        if let Some([b'.', next]) = self.peek_bytes2()
            && char::from(next).is_ascii_digit()
        {
            is_float = true;
            self.bump_bytes(1);
            self.lex_decimal_digits_candidate();
        }

        if self.peek_char().is_some_and(|ch| matches!(ch, 'e' | 'E')) {
            is_float = true;
            self.bump_char();
            if self.peek_char().is_some_and(|ch| matches!(ch, '+' | '-')) {
                self.bump_char();
            }
            while self
                .peek_char()
                .is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_')
            {
                self.bump_char();
            }
        }

        if is_float {
            if self.text[self.pos..].starts_with("f32") {
                self.bump_bytes(3);
            } else if self.peek_char() == Some('f') {
                self.bump_char();
            }

            if self
                .peek_char()
                .is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_')
            {
                self.lex_number_literal_tail_as_ident_candidate();
            }
            self.finish_float_literal(start)
        } else {
            self.finish_int_literal(start)
        }
    }

    fn lex_decimal_digits_candidate(&mut self) {
        while self
            .peek_char()
            .is_some_and(|ch| ch.is_ascii_digit() || ch == '_')
        {
            self.bump_char();
        }
    }

    fn lex_number_literal_tail_as_ident_candidate(&mut self) {
        while self
            .peek_char()
            .is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        {
            self.bump_char();
        }
    }

    fn finish_int_literal(&self, start: usize) -> Result<TokenKind, LexError> {
        let span = Span::new(start, self.pos);
        let text = &self.text[start..self.pos];
        parse_int_literal_checked(text).map_err(|err| LexError::InvalidIntLiteral {
            reason: int_literal_reason(err),
            span: span.into(),
        })?;
        Ok(TokenKind::IntLiteral)
    }

    fn finish_float_literal(&self, start: usize) -> Result<TokenKind, LexError> {
        let span = Span::new(start, self.pos);
        let text = &self.text[start..self.pos];
        parse_float_literal_checked(text).map_err(|err| LexError::InvalidFloatLiteral {
            reason: float_literal_reason(err),
            span: span.into(),
        })?;
        Ok(TokenKind::FloatLiteral)
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

    fn lex_char_literal(&mut self) -> Result<(), LexError> {
        let start = self.pos;
        debug_assert_eq!(self.peek_char(), Some('\''));
        self.bump_char();

        let mut escaped = false;
        while let Some(ch) = self.peek_char() {
            if ch == '\n' {
                return Err(LexError::UnterminatedCharLiteral {
                    span: Span::new(start, self.pos).into(),
                });
            }

            if ch == '\'' && !escaped {
                self.bump_char();
                let text = &self.text[start..self.pos];
                return parse_char_literal(text).map(|_| ()).map_err(|err| {
                    LexError::InvalidCharLiteral {
                        reason: err.reason(),
                        span: Span::new(start, self.pos).into(),
                    }
                });
            }

            escaped = ch == '\\' && !escaped;
            self.bump_char();
        }

        Err(LexError::UnterminatedCharLiteral {
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
        if self.peek_bytes2() == Some([b'<', b'<']) {
            self.bump_bytes(2);
            return Some(Symbol::LtLt);
        }
        if self.peek_bytes2() == Some([b'>', b'>']) {
            self.bump_bytes(2);
            return Some(Symbol::GtGt);
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
        if self.peek_bytes2() == Some([b'.', b'.']) {
            self.bump_bytes(2);
            return Some(Symbol::DotDot);
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
            '%' => Symbol::Percent,
            '&' => Symbol::And,
            '|' => Symbol::Or,
            '^' => Symbol::Caret,
            '~' => Symbol::Tilde,
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

fn int_literal_reason(err: IntLiteralParseError) -> &'static str {
    err.reason()
}

fn float_literal_reason(err: FloatLiteralParseError) -> &'static str {
    err.reason()
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
            let seed = if seed == 0 {
                0x9E37_79B9_7F4A_7C15
            } else {
                seed
            };
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
            ' ', '\t', '\n', '\r', '_', '@', '.', ',', ':', ';', '(', ')', '{', '}', '[', ']', '+',
            '-', '*', '/', '=', '<', '>', '!', '?', '&', '|', '"', '\'', '\\', 'a', 'b', 'c', 'x',
            'y', 'z', 'A', 'B', 'C', '0', '1', '2', '3', '9', '中', 'é',
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
    fn lex_bitwise_and_shift_symbols() {
        assert_eq!(
            kinds("a & b && c | d || e ^ f ~g << 1 >> 2"),
            vec![
                TokenKind::Ident,
                TokenKind::Symbol(Symbol::And),
                TokenKind::Ident,
                TokenKind::Symbol(Symbol::AndAnd),
                TokenKind::Ident,
                TokenKind::Symbol(Symbol::Or),
                TokenKind::Ident,
                TokenKind::Symbol(Symbol::OrOr),
                TokenKind::Ident,
                TokenKind::Symbol(Symbol::Caret),
                TokenKind::Ident,
                TokenKind::Symbol(Symbol::Tilde),
                TokenKind::Ident,
                TokenKind::Symbol(Symbol::LtLt),
                TokenKind::IntLiteral,
                TokenKind::Symbol(Symbol::GtGt),
                TokenKind::IntLiteral,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn lex_prefixed_int_literals() {
        assert_eq!(
            kinds("0xFF 0b1010 0Xca_fe 0B10_01"),
            vec![
                TokenKind::IntLiteral,
                TokenKind::IntLiteral,
                TokenKind::IntLiteral,
                TokenKind::IntLiteral,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn lex_float_literals() {
        assert_eq!(
            kinds("3.14 1e3 1_2.3_4e5_6 0.5f 1.0f32"),
            vec![
                TokenKind::FloatLiteral,
                TokenKind::FloatLiteral,
                TokenKind::FloatLiteral,
                TokenKind::FloatLiteral,
                TokenKind::FloatLiteral,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn lex_int_member_call_and_range_do_not_become_float_literals() {
        assert_eq!(
            kinds("1.toString() 1..2"),
            vec![
                TokenKind::IntLiteral,
                TokenKind::Symbol(Symbol::Dot),
                TokenKind::Ident,
                TokenKind::Symbol(Symbol::LParen),
                TokenKind::Symbol(Symbol::RParen),
                TokenKind::IntLiteral,
                TokenKind::Symbol(Symbol::DotDot),
                TokenKind::IntLiteral,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn lex_char_literals() {
        assert_eq!(
            kinds(r"'a' '\n' '\u0041' '\''"),
            vec![
                TokenKind::CharLiteral,
                TokenKind::CharLiteral,
                TokenKind::CharLiteral,
                TokenKind::CharLiteral,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn lex_invalid_char_literal_reports_reason() {
        let err = lex("''").expect_err("empty char literal should fail");
        match err {
            LexError::InvalidCharLiteral { reason, .. } => assert_eq!(reason, "不能为空"),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn lex_unterminated_char_literal() {
        let err = lex("'a").expect_err("unterminated char literal should fail");
        assert!(matches!(err, LexError::UnterminatedCharLiteral { .. }));
    }

    #[test]
    fn lex_invalid_int_literals_report_reason() {
        let err = lex("0x").expect_err("missing hex digits should fail");
        match err {
            LexError::InvalidIntLiteral { reason, .. } => assert_eq!(reason, "前缀后缺少数字"),
            other => panic!("unexpected error: {other:?}"),
        }

        let err = lex("0b102").expect_err("invalid binary digit should fail");
        match err {
            LexError::InvalidIntLiteral { reason, .. } => assert_eq!(reason, "包含无效数字"),
            other => panic!("unexpected error: {other:?}"),
        }

        let err = lex("1__2").expect_err("invalid separator should fail");
        match err {
            LexError::InvalidIntLiteral { reason, .. } => {
                assert_eq!(reason, "下划线只能出现在数字之间")
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn lex_invalid_float_literals_report_reason() {
        let err = lex("1e+").expect_err("missing exponent digits should fail");
        match err {
            LexError::InvalidFloatLiteral { reason, .. } => {
                assert_eq!(reason, "指数部分缺少数字")
            }
            other => panic!("unexpected error: {other:?}"),
        }

        let err = lex("1e9999").expect_err("overflow float should fail");
        match err {
            LexError::InvalidFloatLiteral { reason, .. } => {
                assert_eq!(reason, "超出 Float64 可表示范围")
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn lex_percent_symbol() {
        assert_eq!(
            kinds("a % b"),
            vec![
                TokenKind::Ident,
                TokenKind::Symbol(Symbol::Percent),
                TokenKind::Ident,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn lex_at_annotation() {
        let ks = kinds("@Unsafe fun f() {}");
        assert!(ks.contains(&TokenKind::Symbol(Symbol::At)));
        assert!(ks.contains(&TokenKind::Keyword(Keyword::Fun)));
        assert!(ks.contains(&TokenKind::Ident));
    }

    #[test]
    fn lex_annotation_keyword() {
        let ks = kinds("annotation class A");
        assert!(ks.contains(&TokenKind::Keyword(Keyword::Annotation)));
        assert!(ks.contains(&TokenKind::Keyword(Keyword::Class)));
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
