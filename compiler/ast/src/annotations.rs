use crate::{FloatLiteralSyntax, Ident, IntegerLiteralSyntax, Span, TypeRef, VisibilitySyntax};

/// A static declaration with constant parameters and no runtime constructor.
#[derive(Debug, Clone, PartialEq)]
pub struct AnnotationClassDecl {
    pub annotations: Vec<Annotation>,
    pub visibility: VisibilitySyntax,
    pub name: Ident,
    pub parameters: Vec<AnnotationParameterDecl>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AnnotationParameterDecl {
    pub name: Ident,
    pub ty: TypeRef,
    pub default: Option<AnnotationLiteral>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PositionalVariantFieldDecl {
    pub annotations: Vec<Annotation>,
    pub ty: TypeRef,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Annotation {
    pub use_site: Option<AnnotationUseSite>,
    pub name: crate::QualifiedNameSyntax,
    pub args: Vec<AnnotationArg>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnnotationUseSite {
    pub target: AnnotationUseSiteTarget,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnnotationUseSiteTarget {
    Property,
    Field,
    Get,
    Set,
    Param,
    SetParam,
    Receiver,
    Delegate,
}

impl AnnotationUseSiteTarget {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Property => "property",
            Self::Field => "field",
            Self::Get => "get",
            Self::Set => "set",
            Self::Param => "param",
            Self::SetParam => "setparam",
            Self::Receiver => "receiver",
            Self::Delegate => "delegate",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AnnotationArg {
    /// A named argument (`name = value`), or `None` for a positional one.
    pub name: Option<Ident>,
    pub value: AnnotationLiteral,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AnnotationLiteral {
    String(String),
    Int(IntegerLiteralSyntax),
    Float(FloatLiteralSyntax),
    Boolean(bool),
    Char(char),
    SignedInt {
        negative: bool,
        literal: IntegerLiteralSyntax,
    },
    SignedFloat {
        negative: bool,
        literal: FloatLiteralSyntax,
    },
    ConstReference(Box<crate::Expr>),
    Application(Box<Annotation>),
    Array(Vec<AnnotationArrayElement>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct AnnotationArrayElement {
    pub value: AnnotationLiteral,
    pub span: Span,
}
