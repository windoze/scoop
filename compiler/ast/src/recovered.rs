//! Independently complete syntax retained alongside a failed file projection.

use crate::{Decl, DocComment, FunctionHeader, ImportSyntax, PackageSyntax};

/// File context for recovered declarations, never a successful SourceFile.
#[derive(Debug, Clone, PartialEq)]
pub struct RecoveredFileHeader {
    /// None means the actual package header failed, not the root package.
    pub package: Option<PackageSyntax>,
    pub imports: Vec<ImportSyntax>,
    pub imports_complete: bool,
    pub documentation: Vec<DocComment>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DeclarationFragment {
    Complete(Box<Decl>),
    FunctionHeader(Box<FunctionHeader>),
}
