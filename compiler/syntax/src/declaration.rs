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
    /// The AST item's source range, recorded by a completed production.
    /// Prefixes consumed outside this CST node may extend this range.
    pub source_span: Option<Span>,
}

/// Grammar-produced data attached to its actual declaration in this parse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclarationData<T> {
    pub declaration: DeclarationIndex,
    pub value: T,
}

/// File-level context and independently completed declaration projections.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileProjections<S, D> {
    pub source: S,
    pub declarations: Vec<DeclarationData<D>>,
}
