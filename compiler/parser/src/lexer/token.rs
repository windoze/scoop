//! Lexical token model and diagnostic rendering.

use scoop_ast::Span;

/// Token kinds through the M11 subset. Reserved words are dedicated variants;
/// `Unit` deliberately stays an [`TokenKind::Ident`] (spec section 4.3:
/// it is not a reserved word). Words that only matter in specific
/// positions (`open`, `abstract`, `override`, `super`, `object`,
/// `sealed`, `companion`, `init`, `constructor`, `try`, `catch`,
/// `finally`, `throw`) stay identifiers too — the parser matches them
/// by text where they are meaningful.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum TokenKind {
    Suspend,
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
    DoubleColon,
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
            TokenKind::Suspend => "`suspend`".to_string(),
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
            TokenKind::DoubleColon => "`::`".to_string(),
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
