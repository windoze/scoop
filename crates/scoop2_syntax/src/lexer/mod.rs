//! Scoop lexer（词法分析，自 `scoopc_ast::syntax::lexer` 移植）。
//!
//! 与旧版 `lex(text) -> Result<Vec<Token>, LexError>` 不同，本实现**不做 fail-fast**：
//!
//! - 任何输入都不会 panic，也不会提前返回错误；
//! - 词法错误以 [`Diagnostic`] 形式收集进 [`DiagnosticSink`]（诊断码与旧版一致），
//!   同时按固定策略恢复并继续产出 token：
//!   - 非法字符：跳过该字符，继续；
//!   - 未闭合的块注释：消费到 EOF，不产出 token；
//!   - 未闭合的字符串 / Char 字面量：消费到 EOF，仍产出对应的字面量 token，
//!     让 parser 能继续工作；
//!   - 非法 Int / Float / Char 字面量：仍产出字面量 token（由诊断标记其非法）。
//! - 产出的 token 序列始终以恰好一个 [`TokenKind::Eof`] 结尾。

pub mod char_literal;
pub mod float_literal;
pub mod int_literal;
pub mod string_literal;

use scoop2_base::Span;
use scoop2_base::diag::{Diagnostic, DiagnosticSink};

use self::char_literal::parse_char_literal;
use self::float_literal::parse_float_literal_checked;
use self::int_literal::parse_int_literal_checked;
use super::token::{Keyword, StringKind, Symbol, Token, TokenKind};

/// 词法分析结果：完整 token 流 + 收集到的诊断。
#[derive(Debug)]
pub struct LexResult {
    /// token 序列；始终以恰好一个 [`TokenKind::Eof`] 结尾。
    pub tokens: Vec<Token>,
    /// 词法诊断（可能为空）。
    pub diagnostics: DiagnosticSink,
}

/// 词法分析入口：将 `text` 转换为 token 序列（带错误恢复，绝不 fail-fast）。
pub fn lex(text: &str) -> LexResult {
    Lexer::new(text).lex_all()
}

struct Lexer<'a> {
    text: &'a str,
    pos: usize,
    force_next_number_integer: bool,
    diagnostics: DiagnosticSink,
}

impl<'a> Lexer<'a> {
    fn new(text: &'a str) -> Self {
        Self {
            text,
            pos: 0,
            force_next_number_integer: false,
            diagnostics: DiagnosticSink::new(),
        }
    }

    fn lex_all(mut self) -> LexResult {
        let mut tokens = Vec::new();
        loop {
            self.skip_trivia();
            let start = self.pos;

            if self.is_eof() {
                tokens.push(Token {
                    kind: TokenKind::Eof,
                    span: Span::new(self.pos, self.pos),
                });
                return LexResult {
                    tokens,
                    diagnostics: self.diagnostics,
                };
            }

            // invariant: 上面已排除 EOF，故必然存在下一个字符。
            let ch = self.peek_char().expect("peek after EOF check");

            // identifiers / keywords
            if is_ident_start(ch) {
                let kind = self.lex_ident_or_keyword();
                self.force_next_number_integer = false;
                tokens.push(Token {
                    kind,
                    span: Span::new(start, self.pos),
                });
                continue;
            }

            // numbers
            if ch.is_ascii_digit() {
                let force_integer = self.force_next_number_integer;
                self.force_next_number_integer = false;
                let kind = self.lex_number_literal(force_integer);
                tokens.push(Token {
                    kind,
                    span: Span::new(start, self.pos),
                });
                continue;
            }

            // strings: "..." or """...""", optionally prefixed with `f`
            if ch == '"' {
                let string_kind = self.lex_string(false);
                self.force_next_number_integer = false;
                tokens.push(Token {
                    kind: TokenKind::StringLiteral(string_kind),
                    span: Span::new(start, self.pos),
                });
                continue;
            }

            if ch == '\'' {
                self.lex_char_literal();
                self.force_next_number_integer = false;
                tokens.push(Token {
                    kind: TokenKind::CharLiteral,
                    span: Span::new(start, self.pos),
                });
                continue;
            }

            // symbols (including multi-char)
            if let Some(sym) = self.lex_symbol() {
                self.force_next_number_integer = matches!(sym, Symbol::Dot | Symbol::QuestionDot);
                tokens.push(Token {
                    kind: TokenKind::Symbol(sym),
                    span: Span::new(start, self.pos),
                });
                continue;
            }

            // 恢复策略：跳过非法字符本身，继续词法分析。
            self.bump_char();
            self.force_next_number_integer = false;
            self.diagnostics.push(
                Diagnostic::error("scoop::lex::invalid_char", format!("非法字符：{ch:?}"))
                    .with_primary(Span::new(start, self.pos), "这里"),
            );
        }
    }

    fn skip_trivia(&mut self) {
        loop {
            // whitespace
            while self.peek_char().is_some_and(|c| c.is_whitespace()) {
                self.bump_char();
            }

            // line comment
            if self.peek_bytes2() == Some(*b"//") {
                self.bump_bytes(2);
                while self.peek_char().is_some_and(|c| c != '\n') {
                    self.bump_char();
                }
                continue;
            }

            // block comment (non-nested)
            if self.peek_bytes2() == Some(*b"/*") {
                let start = self.pos;
                self.bump_bytes(2);
                let mut terminated = false;
                while !self.is_eof() {
                    if self.peek_bytes2() == Some(*b"*/") {
                        self.bump_bytes(2);
                        terminated = true;
                        break;
                    }
                    self.bump_char();
                }
                if !terminated {
                    // 恢复策略：已消费到 EOF，主循环随即产出 Eof。
                    self.diagnostics.push(
                        Diagnostic::error(
                            "scoop::lex::unterminated_block_comment",
                            "未闭合的块注释",
                        )
                        .with_primary(Span::new(start, self.pos), "从这里开始"),
                    );
                }
                continue;
            }

            return;
        }
    }

    fn lex_ident_or_keyword(&mut self) -> TokenKind {
        let start = self.pos;
        self.bump_char(); // first char
        while self.peek_char().is_some_and(is_ident_continue) {
            self.bump_char();
        }
        let ident = &self.text[start..self.pos];

        // f"..." / f"""..."""（`f` 与 `"` 必须相邻）
        if ident == "f" && self.peek_char() == Some('"') {
            let string_kind = self.lex_string(true);
            return TokenKind::StringLiteral(string_kind);
        }

        // `as?` safe cast：要求 `?` 紧跟在 as 后面。
        if ident == "as" && self.peek_char() == Some('?') {
            self.bump_char();
            return TokenKind::Keyword(Keyword::AsQ);
        }

        match ident.parse::<Keyword>() {
            Ok(kw) => TokenKind::Keyword(kw),
            Err(()) => TokenKind::Ident,
        }
    }

    fn lex_number_literal(&mut self, force_integer: bool) -> TokenKind {
        let start = self.pos;
        let Some(first) = self.bump_char() else {
            return TokenKind::IntLiteral;
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

        if force_integer {
            return self.finish_int_literal(start);
        }

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
            self.lex_int_suffix_candidate();
            self.finish_int_literal(start)
        }
    }

    fn lex_int_suffix_candidate(&mut self) {
        if self.peek_char().is_some_and(|ch| matches!(ch, 'u' | 'U')) {
            self.bump_char();
            if self.peek_char().is_some_and(|ch| matches!(ch, 'l' | 'L')) {
                self.bump_char();
            }
            return;
        }

        if self.peek_char().is_some_and(|ch| matches!(ch, 'l' | 'L')) {
            self.bump_char();
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

    /// 校验整数字面量；非法时记录诊断，但仍产出 [`TokenKind::IntLiteral`] 供 parser 继续。
    fn finish_int_literal(&mut self, start: usize) -> TokenKind {
        let span = Span::new(start, self.pos);
        let text = &self.text[start..self.pos];
        if let Err(err) = parse_int_literal_checked(text) {
            self.diagnostics.push(
                Diagnostic::error(
                    "scoop::lex::invalid_int_literal",
                    format!("非法 Int 字面量：{}", err.reason()),
                )
                .with_primary(span, "这里"),
            );
        }
        TokenKind::IntLiteral
    }

    /// 校验浮点字面量；非法时记录诊断，但仍产出 [`TokenKind::FloatLiteral`] 供 parser 继续。
    fn finish_float_literal(&mut self, start: usize) -> TokenKind {
        let span = Span::new(start, self.pos);
        let text = &self.text[start..self.pos];
        if let Err(err) = parse_float_literal_checked(text) {
            self.diagnostics.push(
                Diagnostic::error(
                    "scoop::lex::invalid_float_literal",
                    format!("非法 Float 字面量：{}", err.reason()),
                )
                .with_primary(span, "这里"),
            );
        }
        TokenKind::FloatLiteral
    }

    fn lex_string(&mut self, interpolated: bool) -> StringKind {
        // 当前位置应指向第一个 `"`
        let start = self.pos;

        // raw string: """ ... """
        if self.peek_bytes3() == Some(*b"\"\"\"") {
            self.bump_bytes(3);
            loop {
                if self.is_eof() {
                    self.report_unterminated_string(start);
                    return StringKind::Raw { interpolated };
                }
                if self.peek_bytes3() == Some(*b"\"\"\"") {
                    self.bump_bytes(3);
                    break;
                }
                self.bump_char();
            }
            return StringKind::Raw { interpolated };
        }

        // normal string: " ... "
        debug_assert_eq!(self.peek_char(), Some('"'));
        self.bump_char(); // opening "

        while let Some(ch) = self.peek_char() {
            match ch {
                '"' => {
                    self.bump_char();
                    return StringKind::Normal { interpolated };
                }
                '\\' => {
                    // escape sequence: consume '\' + next char if present
                    self.bump_char();
                    if self.is_eof() {
                        self.report_unterminated_string(start);
                        return StringKind::Normal { interpolated };
                    }
                    self.bump_char();
                }
                '\n' => {
                    // Kotlin-like：普通字符串不允许裸换行
                    self.report_unterminated_string(start);
                    return StringKind::Normal { interpolated };
                }
                _ => {
                    self.bump_char();
                }
            }
        }

        self.report_unterminated_string(start);
        StringKind::Normal { interpolated }
    }

    /// 报告未闭合字符串，并消费到 EOF 以恢复（仍由调用方产出字面量 token）。
    fn report_unterminated_string(&mut self, start: usize) {
        self.diagnostics.push(
            Diagnostic::error("scoop::lex::unterminated_string", "未闭合的字符串字面量")
                .with_primary(Span::new(start, self.pos), "从这里开始"),
        );
        self.pos = self.text.len();
    }

    fn lex_char_literal(&mut self) {
        let start = self.pos;
        debug_assert_eq!(self.peek_char(), Some('\''));
        self.bump_char();

        let mut escaped = false;
        while let Some(ch) = self.peek_char() {
            if ch == '\n' {
                self.report_unterminated_char_literal(start);
                return;
            }

            if ch == '\'' && !escaped {
                self.bump_char();
                let span = Span::new(start, self.pos);
                let text = &self.text[start..self.pos];
                // 恢复策略：字面量非法时仍产出 CharLiteral token（由诊断标记）。
                if let Err(err) = parse_char_literal(text) {
                    self.diagnostics.push(
                        Diagnostic::error(
                            "scoop::lex::invalid_char_literal",
                            format!("非法 Char 字面量：{}", err.reason()),
                        )
                        .with_primary(span, "这里"),
                    );
                }
                return;
            }

            escaped = ch == '\\' && !escaped;
            self.bump_char();
        }

        self.report_unterminated_char_literal(start);
    }

    /// 报告未闭合 Char 字面量，并消费到 EOF 以恢复（仍由调用方产出字面量 token）。
    fn report_unterminated_char_literal(&mut self, start: usize) {
        self.diagnostics.push(
            Diagnostic::error(
                "scoop::lex::unterminated_char_literal",
                "未闭合的 Char 字面量",
            )
            .with_primary(Span::new(start, self.pos), "从这里开始"),
        );
        self.pos = self.text.len();
    }

    fn lex_symbol(&mut self) -> Option<Symbol> {
        // multi-char first (longest match)
        if self.peek_bytes2() == Some(*b"->") {
            self.bump_bytes(2);
            return Some(Symbol::Arrow);
        }
        if self.peek_bytes2() == Some(*b"==") {
            self.bump_bytes(2);
            return Some(Symbol::EqEq);
        }
        if self.peek_bytes2() == Some(*b"!=") {
            self.bump_bytes(2);
            return Some(Symbol::BangEq);
        }
        if self.peek_bytes2() == Some(*b"<=") {
            self.bump_bytes(2);
            return Some(Symbol::LtEq);
        }
        if self.peek_bytes2() == Some(*b">=") {
            self.bump_bytes(2);
            return Some(Symbol::GtEq);
        }
        if self.peek_bytes2() == Some(*b"<<") {
            self.bump_bytes(2);
            return Some(Symbol::LtLt);
        }
        if self.peek_bytes2() == Some(*b">>") {
            self.bump_bytes(2);
            return Some(Symbol::GtGt);
        }
        if self.peek_bytes2() == Some(*b"&&") {
            self.bump_bytes(2);
            return Some(Symbol::AndAnd);
        }
        if self.peek_bytes2() == Some(*b"||") {
            self.bump_bytes(2);
            return Some(Symbol::OrOr);
        }
        if self.peek_bytes2() == Some(*b"!!") {
            self.bump_bytes(2);
            return Some(Symbol::BangBang);
        }
        if self.peek_bytes2() == Some(*b"?.") {
            self.bump_bytes(2);
            return Some(Symbol::QuestionDot);
        }
        if self.peek_bytes2() == Some(*b"?:") {
            self.bump_bytes(2);
            return Some(Symbol::Elvis);
        }
        if self.peek_bytes2() == Some(*b"..") {
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

fn is_ident_start(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphabetic()
}

fn is_ident_continue(ch: char) -> bool {
    is_ident_start(ch) || ch.is_ascii_digit()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 对“应当完全合法”的输入做词法分析；若出现诊断则直接失败。
    fn kinds(src: &str) -> Vec<TokenKind> {
        let result = lex(src);
        assert!(
            result.diagnostics.is_empty(),
            "unexpected diagnostics for {src:?}: {:?}",
            result.diagnostics
        );
        result.tokens.into_iter().map(|t| t.kind).collect()
    }

    /// 断言输入产生恰好一条诊断，且诊断码与消息符合预期。
    fn assert_single_diag(src: &str, code: &str, message: &str) -> LexResult {
        let result = lex(src);
        let diags: Vec<_> = result.diagnostics.iter().collect();
        assert_eq!(
            diags.len(),
            1,
            "expected exactly one diagnostic for {src:?}: {diags:?}"
        );
        assert_eq!(diags[0].code, code);
        assert_eq!(diags[0].message, message);
        assert_eq!(
            result.tokens.last().map(|t| t.kind),
            Some(TokenKind::Eof),
            "token stream must end with Eof for {src:?}"
        );
        result
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
    fn lex_comptime_as_identifier_after_surface_removal() {
        let ks = kinds("comptime");
        assert_eq!(ks, vec![TokenKind::Ident, TokenKind::Eof]);
    }

    #[test]
    fn lex_true_false_are_idents() {
        let ks = kinds("true false");
        assert_eq!(ks, vec![TokenKind::Ident, TokenKind::Ident, TokenKind::Eof]);
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
        let ks = kinds(r#"val a = f"hi ${x}"; val b = f"""raw ${x}""" "#);
        assert!(ks.contains(&TokenKind::StringLiteral(StringKind::Normal {
            interpolated: true
        })));
        assert!(ks.contains(&TokenKind::StringLiteral(StringKind::Raw {
            interpolated: true
        })));
    }

    #[test]
    fn lex_f_not_adjacent_to_string_is_ident() {
        // `f` 与 `"` 不相邻时，`f` 是普通标识符。
        let ks = kinds(r#"f "x""#);
        assert_eq!(
            ks,
            vec![
                TokenKind::Ident,
                TokenKind::StringLiteral(StringKind::Normal {
                    interpolated: false
                }),
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn lex_asq() {
        let ks = kinds("x as? T");
        assert!(ks.contains(&TokenKind::Keyword(Keyword::AsQ)));
    }

    #[test]
    fn lex_as_without_adjacent_question() {
        let ks = kinds("x as ? y");
        assert_eq!(
            ks,
            vec![
                TokenKind::Ident,
                TokenKind::Keyword(Keyword::As),
                TokenKind::Symbol(Symbol::Question),
                TokenKind::Ident,
                TokenKind::Eof,
            ]
        );
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
    fn lex_numeric_member_segments_after_dot_as_int_literals() {
        assert_eq!(
            kinds("x.1.2 y?.0 1.2"),
            vec![
                TokenKind::Ident,
                TokenKind::Symbol(Symbol::Dot),
                TokenKind::IntLiteral,
                TokenKind::Symbol(Symbol::Dot),
                TokenKind::IntLiteral,
                TokenKind::Ident,
                TokenKind::Symbol(Symbol::QuestionDot),
                TokenKind::IntLiteral,
                TokenKind::FloatLiteral,
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
        let result = assert_single_diag(
            "''",
            "scoop::lex::invalid_char_literal",
            "非法 Char 字面量：不能为空",
        );
        // 恢复策略：仍产出 CharLiteral token，parser 可以继续。
        assert_eq!(
            result.tokens.iter().map(|t| t.kind).collect::<Vec<_>>(),
            vec![TokenKind::CharLiteral, TokenKind::Eof]
        );
    }

    #[test]
    fn lex_unterminated_char_literal() {
        let result = assert_single_diag(
            "'a",
            "scoop::lex::unterminated_char_literal",
            "未闭合的 Char 字面量",
        );
        // 恢复策略：消费到 EOF，仍产出 CharLiteral token。
        assert_eq!(
            result.tokens.iter().map(|t| t.kind).collect::<Vec<_>>(),
            vec![TokenKind::CharLiteral, TokenKind::Eof]
        );
        assert_eq!(result.tokens[0].span, Span::new(0, 2));
    }

    #[test]
    fn lex_invalid_int_literals_report_reason() {
        assert_single_diag(
            "0x",
            "scoop::lex::invalid_int_literal",
            "非法 Int 字面量：前缀后缺少数字",
        );
        assert_single_diag(
            "0b102",
            "scoop::lex::invalid_int_literal",
            "非法 Int 字面量：包含无效数字",
        );
        assert_single_diag(
            "1__2",
            "scoop::lex::invalid_int_literal",
            "非法 Int 字面量：下划线只能出现在数字之间",
        );
    }

    #[test]
    fn lex_invalid_int_literal_still_emits_token_and_recovers() {
        let result = lex("0x val y = 1");
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(
            result.tokens.iter().map(|t| t.kind).collect::<Vec<_>>(),
            vec![
                TokenKind::IntLiteral,
                TokenKind::Keyword(Keyword::Val),
                TokenKind::Ident,
                TokenKind::Symbol(Symbol::Eq),
                TokenKind::IntLiteral,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn lex_invalid_float_literals_report_reason() {
        assert_single_diag(
            "1e+",
            "scoop::lex::invalid_float_literal",
            "非法 Float 字面量：指数部分缺少数字",
        );
        assert_single_diag(
            "1e9999",
            "scoop::lex::invalid_float_literal",
            "非法 Float 字面量：超出 Float64 可表示范围",
        );
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

    #[test]
    fn lex_invalid_char_is_skipped_and_lexing_continues() {
        let result = lex("val x = 1 € val y = 2");
        let diags: Vec<_> = result.diagnostics.iter().collect();
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "scoop::lex::invalid_char");
        assert_eq!(diags[0].message, "非法字符：'€'");
        assert_eq!(
            result.tokens.iter().map(|t| t.kind).collect::<Vec<_>>(),
            vec![
                TokenKind::Keyword(Keyword::Val),
                TokenKind::Ident,
                TokenKind::Symbol(Symbol::Eq),
                TokenKind::IntLiteral,
                TokenKind::Keyword(Keyword::Val),
                TokenKind::Ident,
                TokenKind::Symbol(Symbol::Eq),
                TokenKind::IntLiteral,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn lex_unterminated_string_consumes_to_eof_and_emits_token() {
        let result = assert_single_diag(
            "val a = \"x",
            "scoop::lex::unterminated_string",
            "未闭合的字符串字面量",
        );
        assert_eq!(
            result.tokens.iter().map(|t| t.kind).collect::<Vec<_>>(),
            vec![
                TokenKind::Keyword(Keyword::Val),
                TokenKind::Ident,
                TokenKind::Symbol(Symbol::Eq),
                TokenKind::StringLiteral(StringKind::Normal {
                    interpolated: false
                }),
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn lex_unterminated_raw_string_consumes_to_eof_and_emits_token() {
        let result = assert_single_diag(
            "\"\"\"abc",
            "scoop::lex::unterminated_string",
            "未闭合的字符串字面量",
        );
        assert_eq!(
            result.tokens.iter().map(|t| t.kind).collect::<Vec<_>>(),
            vec![
                TokenKind::StringLiteral(StringKind::Raw {
                    interpolated: false
                }),
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn lex_unterminated_block_comment_emits_no_token() {
        let result = assert_single_diag(
            "val x = 1 /* block",
            "scoop::lex::unterminated_block_comment",
            "未闭合的块注释",
        );
        assert_eq!(
            result.tokens.iter().map(|t| t.kind).collect::<Vec<_>>(),
            vec![
                TokenKind::Keyword(Keyword::Val),
                TokenKind::Ident,
                TokenKind::Symbol(Symbol::Eq),
                TokenKind::IntLiteral,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn lex_multiple_errors_are_all_collected() {
        let result = lex("0x € 0b2");
        let codes: Vec<_> = result.diagnostics.iter().map(|d| d.code).collect();
        assert_eq!(
            codes,
            vec![
                "scoop::lex::invalid_int_literal",
                "scoop::lex::invalid_char",
                "scoop::lex::invalid_int_literal",
            ]
        );
        assert_eq!(result.tokens.last().map(|t| t.kind), Some(TokenKind::Eof));
    }

    /// 崩溃防线：确保 lexer 对“任意输入”都不会 panic。
    ///
    /// 这不是高强度 fuzz（不追求覆盖率/错误恢复质量），只要能尽早发现 panic 即可。
    /// 同时验证错误恢复的两个基本不变量：
    /// - token 流以恰好一个 Eof 结尾；
    /// - 所有 token span 都落在输入范围内。
    #[test]
    fn lexer_random_inputs_do_not_panic() {
        let mut rng = XorShift64::new(0xC0FF_EE12_3456_789A);
        for i in 0..2_000usize {
            let src = gen_source(&mut rng, 256);
            let res = std::panic::catch_unwind(|| lex(&src));
            let Ok(result) = res else {
                panic!("lexer panic（iter={i}）: {src:?}");
            };

            let eof_count = result
                .tokens
                .iter()
                .filter(|t| t.kind == TokenKind::Eof)
                .count();
            assert_eq!(
                eof_count, 1,
                "expected exactly one Eof（iter={i}）: {src:?}"
            );
            assert_eq!(
                result.tokens.last().map(|t| t.kind),
                Some(TokenKind::Eof),
                "token stream must end with Eof（iter={i}）: {src:?}"
            );
            for token in &result.tokens {
                assert!(
                    token.span.start <= token.span.end && token.span.end <= src.len(),
                    "token span out of bounds（iter={i}）: span={:?}, len={}, src={src:?}",
                    token.span,
                    src.len()
                );
            }
        }
    }
}
