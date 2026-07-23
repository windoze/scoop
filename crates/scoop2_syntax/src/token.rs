//! Token 定义。
//!
//! 设计目标：
//! - token 尽量轻量：不复制字符串内容，主要依靠 `Span` 回切源文本
//! - 为 parser 提供“足够多”的信息（关键字/符号/字面量类型）

use scoop2_base::Span;

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
    /// `operator`（声明修饰符；语义 gate 在 parser 之后处理）。
    Operator,
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
    On,
    With,
    /// Removed source keyword retained so the parser can report a targeted diagnostic.
    Perform,
    Try,
    Catch,
    Finally,

    // control flow / misc
    /// `do`（spec §7.6）：引入局部 block 表达式 `do { ... }`。
    ///
    /// 说明：裸 `{ ... }` 在表达式位置统一按 closure/lambda 规则解析；
    /// 普通局部 block 必须由 `do` 引入。
    Do,
    Return,
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

impl std::str::FromStr for Keyword {
    /// 非关键字标识符。
    type Err = ();

    /// 从标识符文本查找关键字。
    ///
    /// 说明：`as?`（[`Keyword::AsQ`]）依赖“`?` 紧跟在 `as` 后面”的词法上下文，
    /// 不在此表中，由 lexer 特判；`true` / `false` 是普通标识符，也不是关键字。
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let kw = match text {
            "public" => Keyword::Public,
            "internal" => Keyword::Internal,
            "private" => Keyword::Private,
            "open" => Keyword::Open,
            "abstract" => Keyword::Abstract,
            "sealed" => Keyword::Sealed,
            "inline" => Keyword::Inline,
            "override" => Keyword::Override,
            "operator" => Keyword::Operator,
            "vararg" => Keyword::Vararg,
            "annotation" => Keyword::Annotation,
            "package" => Keyword::Package,
            "import" => Keyword::Import,
            "typealias" => Keyword::Typealias,
            "fun" => Keyword::Fun,
            "val" => Keyword::Val,
            "var" => Keyword::Var,
            "class" => Keyword::Class,
            "interface" => Keyword::Interface,
            "struct" => Keyword::Struct,
            "enum" => Keyword::Enum,
            "effect" => Keyword::Effect,
            "object" => Keyword::Object,
            "companion" => Keyword::Companion,
            "handle" => Keyword::Handle,
            "on" => Keyword::On,
            "with" => Keyword::With,
            "perform" => Keyword::Perform,
            "try" => Keyword::Try,
            "catch" => Keyword::Catch,
            "finally" => Keyword::Finally,
            "do" => Keyword::Do,
            "return" => Keyword::Return,
            "if" => Keyword::If,
            "else" => Keyword::Else,
            "when" => Keyword::When,
            "for" => Keyword::For,
            "in" => Keyword::In,
            "out" => Keyword::Out,
            "where" => Keyword::Where,
            "while" => Keyword::While,
            "break" => Keyword::Break,
            "continue" => Keyword::Continue,
            "is" => Keyword::Is,
            "as" => Keyword::As,
            _ => return Err(()),
        };
        Ok(kw)
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyword_from_str_roundtrips_all_keywords() {
        // `as?` 由 lexer 特判，不在表中。
        let cases: &[(&str, Keyword)] = &[
            ("public", Keyword::Public),
            ("internal", Keyword::Internal),
            ("private", Keyword::Private),
            ("open", Keyword::Open),
            ("abstract", Keyword::Abstract),
            ("sealed", Keyword::Sealed),
            ("inline", Keyword::Inline),
            ("override", Keyword::Override),
            ("operator", Keyword::Operator),
            ("vararg", Keyword::Vararg),
            ("annotation", Keyword::Annotation),
            ("package", Keyword::Package),
            ("import", Keyword::Import),
            ("typealias", Keyword::Typealias),
            ("fun", Keyword::Fun),
            ("val", Keyword::Val),
            ("var", Keyword::Var),
            ("class", Keyword::Class),
            ("interface", Keyword::Interface),
            ("struct", Keyword::Struct),
            ("enum", Keyword::Enum),
            ("effect", Keyword::Effect),
            ("object", Keyword::Object),
            ("companion", Keyword::Companion),
            ("handle", Keyword::Handle),
            ("on", Keyword::On),
            ("with", Keyword::With),
            ("perform", Keyword::Perform),
            ("try", Keyword::Try),
            ("catch", Keyword::Catch),
            ("finally", Keyword::Finally),
            ("do", Keyword::Do),
            ("return", Keyword::Return),
            ("if", Keyword::If),
            ("else", Keyword::Else),
            ("when", Keyword::When),
            ("for", Keyword::For),
            ("in", Keyword::In),
            ("out", Keyword::Out),
            ("where", Keyword::Where),
            ("while", Keyword::While),
            ("break", Keyword::Break),
            ("continue", Keyword::Continue),
            ("is", Keyword::Is),
            ("as", Keyword::As),
        ];
        for (text, expected) in cases {
            assert_eq!(text.parse::<Keyword>(), Ok(*expected), "text={text:?}");
        }
    }

    #[test]
    fn keyword_from_str_rejects_non_keywords() {
        assert!("as?".parse::<Keyword>().is_err());
        assert!("true".parse::<Keyword>().is_err());
        assert!("false".parse::<Keyword>().is_err());
        assert!("comptime".parse::<Keyword>().is_err());
        assert!("foo".parse::<Keyword>().is_err());
        assert!("".parse::<Keyword>().is_err());
    }
}
