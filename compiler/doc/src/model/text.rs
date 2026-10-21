use super::{SourceKey, SymbolKey};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextRange {
    pub start: u32,
    pub end: u32,
}

impl From<scoop_ast::Span> for TextRange {
    fn from(span: scoop_ast::Span) -> Self {
        Self {
            start: span.start,
            end: span.end,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceLocation {
    pub source: SourceKey,
    pub range: TextRange,
    /// One-based line number, computed from the already-read UTF-8 source.
    pub line: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocWarning {
    pub location: SourceLocation,
    pub message: String,
    pub notes: Vec<DocWarningNote>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocWarningNote {
    pub location: SourceLocation,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Comment {
    pub body: Markdown,
    pub tags: Vec<CommentTag>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Markdown {
    pub text: String,
    pub definitions: Vec<String>,
    pub references: Vec<SymbolReference>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SymbolReference {
    pub range: TextRange,
    pub target: LinkTarget,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LinkTarget {
    Symbols(Vec<SymbolKey>),
    Url(String),
    Unresolved,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommentTag {
    pub kind: TagKind,
    pub label: String,
    pub target: Option<LinkTarget>,
    pub body: Markdown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TagKind {
    Param,
    Property,
    Constructor,
    Receiver,
    Return,
    Throws,
    See,
    Since,
}
