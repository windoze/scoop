use scoop_source::Span;

use crate::SyntaxKind;

/// An index within one parse result, never a persistent language identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DeclarationIndex(pub usize);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclarationName {
    pub text: String,
    pub span: Span,
}

/// Facts recorded by the declaration grammar, including incomplete declarations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclarationSyntax {
    pub parent: Option<DeclarationIndex>,
    pub kind: SyntaxKind,
    pub span: Span,
    pub name: Option<DeclarationName>,
}
