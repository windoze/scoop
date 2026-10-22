//! Independently complete syntax retained alongside a failed file projection.

use crate::{
    Decl, DocComment, FunctionHeader, FunctionHeaderRef, ImportSyntax, PackageSyntax,
    SourceHeaderRef, Span, VisibilitySyntax,
};

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
    FunctionBody(Box<crate::RecoveredFunctionBody>),
    Unavailable(DeclarationHeader),
}

/// Independently complete input to declaration collection, never a full file.
#[derive(Debug, Clone, Copy)]
pub enum DeclarationInput<'a> {
    Complete(&'a Decl),
    FunctionHeader(FunctionHeaderRef<'a>),
    RecoveredFunction {
        header: FunctionHeaderRef<'a>,
        body: &'a crate::RecoveredFunctionBody,
    },
    Unavailable(&'a DeclarationHeader),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeclarationNamespaces {
    Type,
    Value,
    TypeAndValue,
}

impl DeclarationNamespaces {
    pub const fn contains(self, namespace: scoop_identity::BindingNamespace) -> bool {
        matches!(
            (self, namespace),
            (Self::TypeAndValue, _)
                | (Self::Type, scoop_identity::BindingNamespace::Type)
                | (Self::Value, scoop_identity::BindingNamespace::Value)
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeclarationHeaderKind {
    Type,
    Singleton,
    Property {
        is_extension: bool,
    },
    Function {
        is_extension: bool,
        is_operator: bool,
        is_infix: bool,
    },
}

impl DeclarationHeaderKind {
    pub const fn namespaces(self) -> DeclarationNamespaces {
        match self {
            Self::Type => DeclarationNamespaces::Type,
            Self::Singleton => DeclarationNamespaces::TypeAndValue,
            Self::Property { .. } | Self::Function { .. } => DeclarationNamespaces::Value,
        }
    }

    pub const fn is_value(self) -> bool {
        matches!(
            self,
            Self::Singleton
                | Self::Property {
                    is_extension: false
                }
        )
    }
}

/// Known name and scope facts for a declaration whose semantic input failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclarationHeader {
    pub name: crate::Ident,
    pub visibility: VisibilitySyntax,
    pub kind: DeclarationHeaderKind,
    pub span: Span,
}
