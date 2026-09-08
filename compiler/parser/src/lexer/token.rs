//! Lexical token model and diagnostic rendering.

use scoop_ast::{IntegerLiteralSyntax, IntegerRadix, IntegerSuffix, Span};

/// Integer token payload. The token itself remains the sole owner of the
/// source span; parsing combines both into `IntegerLiteralSyntax`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct IntegerLiteralLexeme {
    pub magnitude: u64,
    pub radix: IntegerRadix,
    pub suffix: IntegerSuffix,
}

impl IntegerLiteralLexeme {
    pub(crate) fn with_span(self, span: Span) -> IntegerLiteralSyntax {
        IntegerLiteralSyntax {
            magnitude: self.magnitude,
            radix: self.radix,
            suffix: self.suffix,
            span,
        }
    }
}

/// Token kinds for the implemented language surface. Reserved words are dedicated variants;
/// `Unit` deliberately stays an [`TokenKind::Ident`] (spec section 4.3:
/// it is not a reserved word). Words that only matter in specific
/// positions (`open`, `abstract`, `override`, `super`, `object`,
/// `sealed`, `companion`, `init`, `constructor`, `try`, `catch`,
/// `finally`, `throw`, `do`) stay identifiers too — the parser matches them
/// by text where they are meaningful.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum TokenKind {
    Package,
    Import,
    Suspend,
    Vararg,
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
    For,
    Break,
    Continue,
    Return,
    True,
    False,
    This,
    Is,
    In,
    Infix,
    As,
    Ident(String),
    Str(String),
    Int(IntegerLiteralLexeme),
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
    DotDotLess,
    Arrow,
    Question,
    QuestionDot,
    QuestionColon,
    Plus,
    PlusPlus,
    PlusEqual,
    Minus,
    MinusMinus,
    MinusEqual,
    Star,
    StarEqual,
    Slash,
    SlashEqual,
    Percent,
    PercentEqual,
    Bang,
    BangIn,
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
            TokenKind::Package => "`package`".to_string(),
            TokenKind::Import => "`import`".to_string(),
            TokenKind::Suspend => "`suspend`".to_string(),
            TokenKind::Vararg => "`vararg`".to_string(),
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
            TokenKind::For => "`for`".to_string(),
            TokenKind::Break => "`break`".to_string(),
            TokenKind::Continue => "`continue`".to_string(),
            TokenKind::Return => "`return`".to_string(),
            TokenKind::True => "`true`".to_string(),
            TokenKind::False => "`false`".to_string(),
            TokenKind::This => "`this`".to_string(),
            TokenKind::Is => "`is`".to_string(),
            TokenKind::In => "`in`".to_string(),
            TokenKind::Infix => "`infix`".to_string(),
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
            TokenKind::DotDotLess => "`..<`".to_string(),
            TokenKind::Arrow => "`->`".to_string(),
            TokenKind::Question => "`?`".to_string(),
            TokenKind::QuestionDot => "`?.`".to_string(),
            TokenKind::QuestionColon => "`?:`".to_string(),
            TokenKind::Plus => "`+`".to_string(),
            TokenKind::PlusPlus => "`++`".to_string(),
            TokenKind::PlusEqual => "`+=`".to_string(),
            TokenKind::Minus => "`-`".to_string(),
            TokenKind::MinusMinus => "`--`".to_string(),
            TokenKind::MinusEqual => "`-=`".to_string(),
            TokenKind::Star => "`*`".to_string(),
            TokenKind::StarEqual => "`*=`".to_string(),
            TokenKind::Slash => "`/`".to_string(),
            TokenKind::SlashEqual => "`/=`".to_string(),
            TokenKind::Percent => "`%`".to_string(),
            TokenKind::PercentEqual => "`%=`".to_string(),
            TokenKind::Bang => "`!`".to_string(),
            TokenKind::BangIn => "`!in`".to_string(),
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
