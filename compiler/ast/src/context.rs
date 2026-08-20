use crate::{Ident, Span, TypeRef};

/// One ordered requirement of a named callable or computed property.
#[derive(Debug, Clone, PartialEq)]
pub struct ContextParameter {
    pub label: ContextParameterLabel,
    pub ty: TypeRef,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ContextParameterLabel {
    Named(Ident),
    Unnamed(Span),
}

impl ContextParameterLabel {
    pub fn span(&self) -> Span {
        match self {
            Self::Named(name) => name.span,
            Self::Unnamed(span) => *span,
        }
    }
}
