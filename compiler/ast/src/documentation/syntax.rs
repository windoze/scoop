//! Prepared documentation syntax, before ordinary HIR name resolution.

use super::DocText;
use crate::{Diagnostic, Ident, QualifiedNameSyntax, Span};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SourceDocumentation {
    pub documents: Vec<DocDocument>,
    pub warnings: Vec<Diagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocDocument {
    /// The exact AST node recorded by the parser, never a semantic identity.
    pub declaration: Span,
    pub span: Span,
    pub body: DocMarkdown,
    pub tags: Vec<DocTag>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocMarkdown {
    pub text: DocText,
    /// Original reference definitions shared by all parts of one document.
    /// Keeping their spelling lets Markdown retain its own label rules.
    pub definitions: Vec<DocText>,
    pub references: Vec<DocSymbolReference>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocSymbolReference {
    /// Range in the normalized Markdown text, including the link syntax.
    pub range: Span,
    pub path: QualifiedNameSyntax,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocTag {
    pub kind: DocTagKind,
    pub span: Span,
    pub body: DocMarkdown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocTagKind {
    Param(Ident),
    Property(Ident),
    Constructor,
    Receiver,
    Return,
    Throws(QualifiedNameSyntax),
    See(DocSeeTarget),
    Since,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocSeeTarget {
    Symbol(QualifiedNameSyntax),
    Url { text: String, span: Span },
}
