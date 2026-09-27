use std::fmt;

use scoop_identity::{
    ConeIdentity, DependencyCallableDeclarationId, GcEffect, IdentityReferenceError,
    ObjectDefinitionPlanId, PersistentCallableBodyId, PersistentSymbolRequest,
    StrongCallableDefinitionOwner,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ParamFreeLirCallableBuildError {
    TargetMismatch {
        declaration: DependencyCallableDeclarationId,
        expected: StrongCallableDefinitionOwner,
        actual: StrongCallableDefinitionOwner,
    },
    Suspend {
        declaration: DependencyCallableDeclarationId,
    },
    RootProtocolMismatch {
        declaration: DependencyCallableDeclarationId,
        abi: GcEffect,
        root: GcEffect,
    },
    Contract(crate::CallableLinkContractError),
}

impl fmt::Display for ParamFreeLirCallableBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TargetMismatch {
                declaration,
                expected,
                actual,
            } => write!(
                formatter,
                "dependency callable {declaration:?} requires target {expected:?}, found {actual:?}"
            ),
            Self::Suspend { declaration } => write!(
                formatter,
                "dependency callable {declaration:?} is suspend and cannot use the param-free LIR bridge"
            ),
            Self::RootProtocolMismatch {
                declaration,
                abi,
                root,
            } => write!(
                formatter,
                "dependency callable {declaration:?} has ABI GC effect {abi:?}, but root plan has {root:?}"
            ),
            Self::Contract(source) => source.fmt(formatter),
        }
    }
}

impl std::error::Error for ParamFreeLirCallableBuildError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Contract(source) => Some(source),
            Self::TargetMismatch { .. }
            | Self::Suspend { .. }
            | Self::RootProtocolMismatch { .. } => None,
        }
    }
}

#[derive(Debug)]
pub enum ParamFreeLirCallableResolutionError {
    Declaration(IdentityReferenceError),
    Callable(crate::CallableAbiDecodeError),
    Shape(ParamFreeLirCallableBuildError),
}

impl fmt::Display for ParamFreeLirCallableResolutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Declaration(source) => write!(formatter, "invalid declaration: {source}"),
            Self::Callable(source) => source.fmt(formatter),
            Self::Shape(source) => source.fmt(formatter),
        }
    }
}

impl std::error::Error for ParamFreeLirCallableResolutionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Declaration(source) => Some(source),
            Self::Callable(source) => Some(source),
            Self::Shape(source) => Some(source),
        }
    }
}

#[derive(Debug)]
pub enum SelectedDependencyLirCallableResolutionError {
    Provider(IdentityReferenceError),
    Bridge(ParamFreeLirCallableResolutionError),
}

impl fmt::Display for SelectedDependencyLirCallableResolutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Provider(source) => write!(formatter, "invalid provider: {source}"),
            Self::Bridge(source) => write!(formatter, "invalid provider bridge: {source}"),
        }
    }
}

impl std::error::Error for SelectedDependencyLirCallableResolutionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Provider(source) => Some(source),
            Self::Bridge(source) => Some(source),
        }
    }
}

#[derive(Debug)]
pub enum CrossConeLirBridgeRelationError {
    Contract(crate::CallableLinkContractError),
    ExportContractMismatch {
        index: usize,
        declaration: DependencyCallableDeclarationId,
    },
    MissingExportBody {
        index: usize,
        body: PersistentCallableBodyId,
    },
    MissingExportSymbol {
        index: usize,
        symbol: PersistentSymbolRequest,
    },
    MissingExportDefinition {
        index: usize,
        definition: ObjectDefinitionPlanId,
    },
    ExportDefinitionMismatch {
        index: usize,
        definition: ObjectDefinitionPlanId,
    },
    SelectedCurrentProvider {
        index: usize,
        provider: ConeIdentity,
    },
    SelectedContractMismatch {
        index: usize,
        provider: ConeIdentity,
        declaration: DependencyCallableDeclarationId,
    },
}

impl fmt::Display for CrossConeLirBridgeRelationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid cross-Cone LIR bridge relation: {self:?}"
        )
    }
}

impl std::error::Error for CrossConeLirBridgeRelationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Contract(source) => Some(source),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub enum CrossConeLirBridgeBuildError {
    Callable(crate::CallableAbiValidationError),
    Resource(scoop_wire::WireError),
    DuplicateExport(DependencyCallableDeclarationId),
    DuplicateSelected {
        provider: ConeIdentity,
        declaration: DependencyCallableDeclarationId,
    },
    Relation(CrossConeLirBridgeRelationError),
}

impl fmt::Display for CrossConeLirBridgeBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "cannot build cross-Cone LIR bridge: {self:?}")
    }
}

impl std::error::Error for CrossConeLirBridgeBuildError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Callable(source) => Some(source),
            Self::Resource(source) => Some(source),
            Self::Relation(source) => Some(source),
            Self::DuplicateExport(_) | Self::DuplicateSelected { .. } => None,
        }
    }
}

#[derive(Debug)]
pub enum CrossConeLirBridgeValidationError {
    Resource(scoop_wire::WireError),
    Encode(scoop_wire::cbor::EncodeError),
    Export {
        index: usize,
        source: ParamFreeLirCallableResolutionError,
    },
    Selected {
        index: usize,
        source: SelectedDependencyLirCallableResolutionError,
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
    Relation(CrossConeLirBridgeRelationError),
    SectionMismatch,
}

impl fmt::Display for CrossConeLirBridgeValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid cross-Cone LIR bridge: {self:?}")
    }
}

impl std::error::Error for CrossConeLirBridgeValidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(source) => Some(source),
            Self::Encode(source) => Some(source),
            Self::Export { source, .. } => Some(source),
            Self::Selected { source, .. } => Some(source),
            Self::Relation(source) => Some(source),
            Self::DuplicateExport { .. }
            | Self::NonCanonicalExportOrder { .. }
            | Self::DuplicateSelected { .. }
            | Self::NonCanonicalSelectedOrder { .. }
            | Self::SectionMismatch => None,
        }
    }
}
