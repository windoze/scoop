//! Token 定义。
//!
//! 设计目标：
//! - token 尽量轻量：不复制字符串内容，主要依靠 `Span` 回切源文本
//! - 为 parser 提供“足够多”的信息（关键字/符号/字面量类型）

use crate::span::Span;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    Ident,
    IntLiteral,
    StringLiteral(StringKind),

    Keyword(Keyword),
    Symbol(Symbol),

    Eof,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StringKind {
    /// `"..."` / `f"..."`
    Normal { interpolated: bool },
    /// `"""..."""` / `f"""..."""`
    Raw { interpolated: bool },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Keyword {
    // declarations
    Package,
    Import,
    Fun,
    Val,
    Var,
    Class,
    Interface,
    Struct,
    Enum,
    Effect,

    // effects
    Handle,
    With,
    Perform,
    Try,
    Catch,
    Finally,
    Async,
    Await,

    // control flow / misc
    Return,
    If,
    Else,
    When,
    Is,

    // casts
    As,
    AsQ, // `as?`
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Symbol {
    // one-char
    At,
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Comma,
    Colon,
    Semicolon,
    Dot,
    Plus,
    Minus,
    Star,
    Slash,
    Eq,
    Lt,
    Gt,
    Bang,
    Question,

    // multi-char
    Arrow,       // ->
    EqEq,        // ==
    BangEq,      // !=
    LtEq,        // <=
    GtEq,        // >=
    AndAnd,      // &&
    OrOr,        // ||
    BangBang,    // !!
    QuestionDot, // ?.
    Elvis,       // ?:
}
