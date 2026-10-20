//! Source documentation and the parser's exact syntax-node attachment.

use crate::Span;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocCommentKind {
    Line,
    Block,
}

/// An attachment is local AST syntax, not a language declaration identity.
/// HIR associates the referenced node with its real typed declaration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocCommentTarget {
    Unattached,
    Declaration(Span),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocComment {
    pub kind: DocCommentKind,
    /// Original spelling, including delimiters and original line endings.
    pub text: String,
    pub span: Span,
    pub target: DocCommentTarget,
}
