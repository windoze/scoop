use std::fmt;

use scoop_identity::{
    ConeIdentity, DependencyCallableDeclarationId, ExactCallableSignatureResolutionError,
    IdentityReferenceError, StrongCallableDefinitionOwner,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParamFreeMirCallableBuildError {
    ImplementationMismatch {
        declaration: DependencyCallableDeclarationId,
        expected: StrongCallableDefinitionOwner,
        actual: StrongCallableDefinitionOwner,
    },
    Suspend {
        declaration: DependencyCallableDeclarationId,
    },
}

impl fmt::Display for ParamFreeMirCallableBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ImplementationMismatch {
                declaration,
                expected,
                actual,
            } => write!(
                formatter,
                "dependency callable {declaration:?} requires implementation {expected:?}, found {actual:?}"
            ),
            Self::Suspend { declaration } => write!(
                formatter,
                "dependency callable {declaration:?} is suspend and cannot use the param-free bridge"
            ),
        }
    }
}

impl std::error::Error for ParamFreeMirCallableBuildError {}

#[derive(Debug, Eq, PartialEq)]
pub enum ParamFreeMirCallableResolutionError {
    Resource(scoop_wire::WireError),
    Declaration(IdentityReferenceError),
    Implementation(IdentityReferenceError),
    Signature(ExactCallableSignatureResolutionError<IdentityReferenceError>),
    Shape(ParamFreeMirCallableBuildError),
}

impl fmt::Display for ParamFreeMirCallableResolutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(source) => source.fmt(formatter),
            Self::Declaration(source) => write!(formatter, "invalid declaration: {source}"),
            Self::Implementation(source) => write!(formatter, "invalid implementation: {source}"),
            Self::Signature(source) => write!(formatter, "invalid exact signature: {source}"),
            Self::Shape(source) => source.fmt(formatter),
        }
    }
}

impl std::error::Error for ParamFreeMirCallableResolutionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(source) => Some(source),
            Self::Declaration(source) | Self::Implementation(source) => Some(source),
            Self::Signature(source) => Some(source),
            Self::Shape(source) => Some(source),
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum SelectedDependencyMirCallableResolutionError {
    Resource(scoop_wire::WireError),
    Provider(IdentityReferenceError),
    Declaration(IdentityReferenceError),
    Implementation(IdentityReferenceError),
    Signature(ExactCallableSignatureResolutionError<IdentityReferenceError>),
    Shape(ParamFreeMirCallableBuildError),
}

impl fmt::Display for SelectedDependencyMirCallableResolutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(source) => source.fmt(formatter),
            Self::Provider(source) => write!(formatter, "invalid provider: {source}"),
            Self::Declaration(source) => write!(formatter, "invalid declaration: {source}"),
            Self::Implementation(source) => write!(formatter, "invalid implementation: {source}"),
            Self::Signature(source) => write!(formatter, "invalid exact signature: {source}"),
            Self::Shape(source) => source.fmt(formatter),
        }
    }
}

impl std::error::Error for SelectedDependencyMirCallableResolutionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(source) => Some(source),
            Self::Provider(source) | Self::Declaration(source) | Self::Implementation(source) => {
                Some(source)
            }
            Self::Signature(source) => Some(source),
            Self::Shape(source) => Some(source),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CrossConeMirBridgeRelationError {
    CoreExportsOrdinaryDependencyCallable,
    MissingExportImplementation {
        index: usize,
        implementation: StrongCallableDefinitionOwner,
    },
    ExportSignatureMismatch {
        index: usize,
        implementation: StrongCallableDefinitionOwner,
    },
    SelectedCurrentProvider {
        index: usize,
        provider: ConeIdentity,
    },
    SelectedTrustedCore {
        index: usize,
    },
}

impl fmt::Display for CrossConeMirBridgeRelationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CoreExportsOrdinaryDependencyCallable => formatter.write_str(
                "trusted core must use its dedicated bridge and cannot export ordinary dependency callables",
            ),
            Self::MissingExportImplementation {
                index,
                implementation,
            } => write!(
                formatter,
                "MIR dependency export {index} has no strong signature for {implementation:?}"
            ),
            Self::ExportSignatureMismatch {
                index,
                implementation,
            } => write!(
                formatter,
                "MIR dependency export {index} disagrees with the strong signature for {implementation:?}"
            ),
            Self::SelectedCurrentProvider { index, provider } => write!(
                formatter,
                "MIR dependency selection {index} names current Cone {provider} as its provider"
            ),
            Self::SelectedTrustedCore { index } => write!(
                formatter,
                "MIR dependency selection {index} must use the dedicated trusted-core bridge"
            ),
        }
    }
}

impl std::error::Error for CrossConeMirBridgeRelationError {}

#[derive(Debug, Eq, PartialEq)]
pub enum CrossConeMirBridgeBuildError {
    DuplicateExport(DependencyCallableDeclarationId),
    DuplicateSelected {
        provider: ConeIdentity,
        declaration: DependencyCallableDeclarationId,
    },
    Relation(CrossConeMirBridgeRelationError),
}

impl fmt::Display for CrossConeMirBridgeBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateExport(declaration) => {
                write!(formatter, "duplicate MIR dependency export {declaration:?}")
            }
            Self::DuplicateSelected {
                provider,
                declaration,
            } => write!(
                formatter,
                "duplicate MIR dependency selection {provider}:{declaration:?}"
            ),
            Self::Relation(source) => source.fmt(formatter),
        }
    }
}

impl std::error::Error for CrossConeMirBridgeBuildError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Relation(source) => Some(source),
            Self::DuplicateExport(_) | Self::DuplicateSelected { .. } => None,
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum CrossConeMirBridgeValidationError {
    Resource(scoop_wire::WireError),
    Export {
        index: usize,
        source: ParamFreeMirCallableResolutionError,
    },
    Selected {
        index: usize,
        source: SelectedDependencyMirCallableResolutionError,
    },
    DuplicateExport {
        index: usize,
        declaration: DependencyCallableDeclarationId,
    },
    NonCanonicalExportOrder {
        index: usize,
    },
    DuplicateSelected {
        index: usize,
        provider: ConeIdentity,
        declaration: DependencyCallableDeclarationId,
    },
    NonCanonicalSelectedOrder {
        index: usize,
    },
    Relation(CrossConeMirBridgeRelationError),
}

impl fmt::Display for CrossConeMirBridgeValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(source) => source.fmt(formatter),
            Self::Export { index, source } => {
                write!(formatter, "invalid MIR dependency export {index}: {source}")
            }
            Self::Selected { index, source } => {
                write!(
                    formatter,
                    "invalid MIR dependency selection {index}: {source}"
                )
            }
            Self::DuplicateExport { index, declaration } => write!(
                formatter,
                "duplicate MIR dependency export {declaration:?} at index {index}"
            ),
            Self::NonCanonicalExportOrder { index } => write!(
                formatter,
                "non-canonical MIR dependency export order at index {index}"
            ),
            Self::DuplicateSelected {
                index,
                provider,
                declaration,
            } => write!(
                formatter,
                "duplicate MIR dependency selection {provider}:{declaration:?} at index {index}"
            ),
            Self::NonCanonicalSelectedOrder { index } => write!(
                formatter,
                "non-canonical MIR dependency selection order at index {index}"
            ),
            Self::Relation(source) => source.fmt(formatter),
        }
    }
}

impl std::error::Error for CrossConeMirBridgeValidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(source) => Some(source),
            Self::Export { source, .. } => Some(source),
            Self::Selected { source, .. } => Some(source),
            Self::Relation(source) => Some(source),
            Self::DuplicateExport { .. }
            | Self::NonCanonicalExportOrder { .. }
            | Self::DuplicateSelected { .. }
            | Self::NonCanonicalSelectedOrder { .. } => None,
        }
    }
}
