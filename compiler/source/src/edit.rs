use crate::{SourceVersion, Span};

/// A replacement in one immutable source version. Applying it belongs to the host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceEdit {
    pub source: SourceVersion,
    pub span: Span,
    pub replacement: String,
}
