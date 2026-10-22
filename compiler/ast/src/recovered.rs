//! Independently complete syntax retained alongside a failed file projection.

use crate::{Decl, DocComment, FunctionHeader, ImportSyntax, PackageSyntax, SourceHeaderRef, Span};

/// File context for recovered declarations, never a successful SourceFile.
#[derive(Debug, Clone, PartialEq)]
pub struct RecoveredFileHeader {
    /// None means the actual package header failed, not the root package.
    pub package: Option<PackageSyntax>,
    pub imports: Vec<ImportSyntax>,
    pub imports_complete: bool,
    pub documentation: Vec<DocComment>,
}

impl RecoveredFileHeader {
    pub fn header(&self, span: Span) -> Option<SourceHeaderRef<'_>> {
        if !self.imports_complete {
            return None;
        }
        Some(SourceHeaderRef {
            package: self.package.as_ref()?,
            imports: &self.imports,
            span,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum DeclarationFragment {
    Complete(Box<Decl>),
    FunctionHeader(Box<FunctionHeader>),
}
