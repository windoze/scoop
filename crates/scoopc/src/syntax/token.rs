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
    FloatLiteral,
    CharLiteral,
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
    // modifiers
    Public,
    Internal,
    Private,
    Open,
    Abstract,
    Sealed,
    Inline,
    Override,
    Const,
    /// `vararg`（Appendix B.5.5）。
    ///
    /// 说明：该关键字目前仅在“形参位置”作为修饰符使用；语义由 typecheck 负责。
    Vararg,
    /// `annotation`（用于 `annotation class`）。
    ///
    /// 说明：当前阶段把它当作 modifier 解析与存储；语义限制（只能用于 class 等）
    /// 留给后续 typecheck/resolve。
    Annotation,

    // declarations
    Package,
    Import,
    /// `typealias`（Appendix B.10）。
    Typealias,
    Fun,
    Val,
    Var,
    Class,
    Interface,
    Struct,
    Enum,
    Effect,
    Object,
    Companion,

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
    Comptime,
    If,
    Else,
    When,
    For,
    In,
    /// `out`（声明处变型/星投影相关语法的一部分，spec §3）。
    Out,
    /// `where`（泛型约束子句，spec §3 / Appendix B）。
    Where,
    While,
    Break,
    Continue,
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
    /// `..`（range/rest pattern，Appendix B.12 / Appendix B.11）。
    DotDot,
    Plus,
    Minus,
    Star,
    Slash,
    Percent, // %
    And,     // &
    Or,      // |
    Caret,   // ^
    Tilde,   // ~
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
    LtLt,        // <<
    GtGt,        // >>
    AndAnd,      // &&
    OrOr,        // ||
    BangBang,    // !!
    QuestionDot, // ?.
    Elvis,       // ?:
}
