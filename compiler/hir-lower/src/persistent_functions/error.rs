use std::fmt;

use scoop_ast::Span;
use scoop_hir as hir;

#[derive(Debug)]
pub(crate) struct PersistentFunctionIdentityError {
    pub(super) file: usize,
    pub(super) span: Span,
    pub(super) detail: PersistentFunctionIdentityErrorDetail,
}

impl PersistentFunctionIdentityError {
    pub(crate) const fn file(&self) -> usize {
        self.file
    }

    pub(crate) const fn span(&self) -> Span {
        self.span
    }
}

impl fmt::Display for PersistentFunctionIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "cannot derive persistent function identity: {}",
            self.detail
        )
    }
}

impl std::error::Error for PersistentFunctionIdentityError {}

#[derive(Debug)]
pub(super) enum PersistentFunctionIdentityErrorDetail {
    MissingSignature,
    MissingSourceFile,
    InvalidName(scoop_identity::CanonicalIdentifierError),
    InvalidSite(scoop_identity::SourceDeclarationKeyError),
    InvalidSignatureType(hir::HirSignatureTypeMappingError),
    InvalidIdentity(hir::HirFunctionIdentityError),
    GeneratedDeclarationOwner,
    OwnerSiteMismatch,
    TooManyTypeParameters,
    BinderDepthOverflow,
    InvalidBinderInheritance,
    AmbiguousLexicalParent,
    CyclicLexicalParent,
    InvalidLexicalRoot,
    InvalidFunctionOrigin,
}

impl fmt::Display for PersistentFunctionIdentityErrorDetail {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingSignature => formatter.write_str("function has no resolved signature"),
            Self::MissingSourceFile => formatter.write_str("declaration has no source file"),
            Self::InvalidName(error) => error.fmt(formatter),
            Self::InvalidSite(error) => error.fmt(formatter),
            Self::InvalidSignatureType(error) => error.fmt(formatter),
            Self::InvalidIdentity(error) => error.fmt(formatter),
            Self::GeneratedDeclarationOwner => {
                formatter.write_str("a source function has a generated declaration owner")
            }
            Self::OwnerSiteMismatch => formatter
                .write_str("function source Cone or package differs from its declaration owner"),
            Self::TooManyTypeParameters => {
                formatter.write_str("type parameter count exceeds the identity schema")
            }
            Self::BinderDepthOverflow => {
                formatter.write_str("lexical binder nesting exceeds the identity schema")
            }
            Self::InvalidBinderInheritance => formatter
                .write_str("function inherited type parameters differ from its lexical parent"),
            Self::AmbiguousLexicalParent => {
                formatter.write_str("definition path has multiple nearest callable parents")
            }
            Self::CyclicLexicalParent => {
                formatter.write_str("lexical callable parent relation is cyclic")
            }
            Self::InvalidLexicalRoot => {
                formatter.write_str("definition path has an invalid lexical root")
            }
            Self::InvalidFunctionOrigin => {
                formatter.write_str("function is not owned by exactly one identity source")
            }
        }
    }
}
