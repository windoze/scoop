use crate::{Ident, IntegerLiteralSyntax, Span, TypeRef, VisibilitySyntax};

/// A static declaration with scalar parameters and no runtime constructor.
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
    pub name: Ident,
    pub args: Vec<AnnotationArg>,
    pub span: Span,
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
    Boolean(bool),
    Char(char),
    SignedInt {
        negative: bool,
        literal: IntegerLiteralSyntax,
    },
    ConstReference(Box<crate::Expr>),
}
