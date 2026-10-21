//! Concrete token and grammar node kinds. Contextual words remain identifiers.

macro_rules! syntax_kinds {
    ($($kind:ident),+ $(,)?) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        #[repr(u16)]
        pub enum SyntaxKind { $($kind),+ }

        impl SyntaxKind {
            pub(crate) fn from_raw(raw: u16) -> Self {
                const KINDS: &[SyntaxKind] = &[$(SyntaxKind::$kind),+];
                KINDS[usize::from(raw)]
            }
        }
    };
}

syntax_kinds! {
    InvalidToken,
    KwPackage,
    KwImport,
    KwSuspend,
    KwVararg,
    KwFun,
    KwStruct,
    KwEnum,
    KwClass,
    KwInterface,
    KwVal,
    KwVar,
    KwIf,
    KwElse,
    KwWhen,
    KwWhile,
    KwDo,
    KwFor,
    KwBreak,
    KwContinue,
    KwReturn,
    KwTrue,
    KwFalse,
    KwThis,
    KwIs,
    KwIn,
    KwInfix,
    KwAs,
    Ident,
    EscapedIdent,
    String,
    FStringStart,
    FStringText,
    InterpolationStart,
    InterpolationEnd,
    FStringEnd,
    Char,
    Integer,
    Float,
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
    Whitespace,
    LineComment,
    BlockComment,
    DocLineComment,
    DocBlockComment,
    Shebang,
    SourceFile,
    FunctionBody,
    TypeFragment,
    Declaration,
    FunctionDecl,
    PropertyDecl,
    FieldDecl,
    StructDecl,
    ClassDecl,
    EnumDecl,
    InterfaceDecl,
    ObjectDecl,
    CompanionDecl,
    AnnotationClassDecl,
    TypeAliasDecl,
    PackageHeader,
    ImportHeader,
    Member,
    ClassBody,
    StructBody,
    InterfaceBody,
    ParameterList,
    Parameter,
    PrimaryConstructor,
    SecondaryConstructor,
    InitBlock,
    ReleaseBlock,
    TypeParameterList,
    TypeParameter,
    TypeArgumentList,
    WhereClause,
    TypeConstraint,
    RequirementList,
    Requirement,
    SupertypeList,
    Variant,
    Block,
    Statement,
    Type,
    Pattern,
    PatternList,
    Expression,
    Atom,
    ArgumentList,
    Argument,
    ArrayExpr,
    ParenExpr,
    LambdaExpr,
    AnonymousFunctionExpr,
    InterpolatedString,
    IfExpr,
    WhenExpr,
    WhenArm,
    WhileStmt,
    ForStmt,
    TryExpr,
    CatchClause,
    AnnotationList,
    Annotation,
    AnnotationArray,
    ContextParameters,
    Name,
    Error,
    Missing,
}

impl SyntaxKind {
    pub fn is_trivia(self) -> bool {
        matches!(
            self,
            Self::Whitespace
                | Self::LineComment
                | Self::BlockComment
                | Self::DocLineComment
                | Self::DocBlockComment
                | Self::Shebang
        )
    }

    pub fn is_declaration(self) -> bool {
        matches!(
            self,
            Self::FunctionDecl
                | Self::PropertyDecl
                | Self::StructDecl
                | Self::ClassDecl
                | Self::EnumDecl
                | Self::InterfaceDecl
                | Self::ObjectDecl
                | Self::CompanionDecl
                | Self::AnnotationClassDecl
                | Self::TypeAliasDecl
        )
    }
}
