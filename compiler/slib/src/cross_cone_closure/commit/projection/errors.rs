use std::fmt;

use scoop_identity::{
    ConeIdentity, DependencyCallableDeclarationId, StrongCallableDefinitionOwner,
};

#[derive(Debug)]
pub enum CrossConeMirSelectionProjectionError {
    Resource(scoop_wire::WireError),
    Occurrences(scoop_hir::DependencyCallOccurrenceError),
    Expressions(scoop_hir::concrete::ExecutableExpressionStructureError),
    RuntimeConstructorKind(scoop_hir::ImportedCoreProtocolCallableDefinition),
    InitializationFunctionKind(scoop_hir::ImportedCoreProtocolCallableDefinition),
    MissingSingleton {
        provider: ConeIdentity,
        value: scoop_identity::PersistentObjectValueId,
    },
    SingletonType(scoop_identity::PersistentObjectValueId),
    ConsumerMismatch {
        closure: ConeIdentity,
        selected: ConeIdentity,
    },
    MissingProvider {
        provider: ConeIdentity,
    },
    MissingExport {
        provider: ConeIdentity,
        target: StrongCallableDefinitionOwner,
    },
    ImplementationMismatch {
        provider: ConeIdentity,
        target: StrongCallableDefinitionOwner,
    },
    SignatureMismatch {
        provider: ConeIdentity,
        target: StrongCallableDefinitionOwner,
    },
    Record(scoop_mir::ParamFreeMirCallableBuildError),
    Selection(scoop_mir::SelectedExternalMirSetBuildError),
}

impl fmt::Display for CrossConeMirSelectionProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingSingleton { provider, value } => write!(
                formatter,
                "provider {provider} has no MIR object value {value}"
            ),
            Self::SingletonType(value) => write!(
                formatter,
                "dependency singleton {value} has an inconsistent result type"
            ),
            Self::Resource(source) => source.fmt(formatter),
            Self::Occurrences(source) => source.fmt(formatter),
            Self::Expressions(source) => source.fmt(formatter),
            Self::RuntimeConstructorKind(definition) => write!(
                formatter,
                "runtime exception target is not a constructor: {definition:?}"
            ),
            Self::InitializationFunctionKind(definition) => write!(
                formatter,
                "initialization target is not a source function: {definition:?}"
            ),
            Self::ConsumerMismatch { closure, selected } => write!(
                formatter,
                "dependency HIR selection belongs to consumer {selected}, not closure {closure}"
            ),
            Self::MissingProvider { provider } => write!(
                formatter,
                "dependency HIR selection names provider {provider} outside the committed closure"
            ),
            Self::MissingExport { provider, target } => write!(
                formatter,
                "provider {provider} has no MIR export for selected callable {target:?}"
            ),
            Self::ImplementationMismatch { provider, target } => write!(
                formatter,
                "provider {provider} changed the implementation of selected callable {target:?} between HIR and MIR"
            ),
            Self::SignatureMismatch { provider, target } => write!(
                formatter,
                "provider {provider} changed the signature of selected callable {target:?} between HIR and MIR"
            ),
            Self::Record(source) => write!(
                formatter,
                "cannot construct a selected dependency MIR record: {source}"
            ),
            Self::Selection(source) => write!(
                formatter,
                "cannot seal the selected dependency MIR set: {source}"
            ),
        }
    }
}

impl std::error::Error for CrossConeMirSelectionProjectionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(source) => Some(source),
            Self::Occurrences(source) => Some(source),
            Self::Expressions(source) => Some(source),
            Self::RuntimeConstructorKind(_) => None,
            Self::InitializationFunctionKind(_) => None,
            Self::MissingSingleton { .. } | Self::SingletonType(_) => None,
            Self::Record(source) => Some(source),
            Self::Selection(source) => Some(source),
            Self::ConsumerMismatch { .. }
            | Self::MissingProvider { .. }
            | Self::MissingExport { .. }
            | Self::ImplementationMismatch { .. }
            | Self::SignatureMismatch { .. } => None,
        }
    }
}

#[derive(Debug)]
pub enum CrossConeLirSelectionProjectionError {
    ConsumerMismatch {
        closure: ConeIdentity,
        selected: ConeIdentity,
    },
    MissingProvider {
        provider: ConeIdentity,
    },
    MissingExport {
        provider: ConeIdentity,
        declaration: DependencyCallableDeclarationId,
    },
    BridgeMismatch {
        provider: ConeIdentity,
        declaration: DependencyCallableDeclarationId,
    },
    Record(scoop_lir::ParamFreeLirCallableBuildError),
    Selection(scoop_lir::SelectedExternalLirSetBuildError),
}

impl fmt::Display for CrossConeLirSelectionProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConsumerMismatch { closure, selected } => write!(
                formatter,
                "dependency MIR selection belongs to consumer {selected}, not closure {closure}"
            ),
            Self::MissingProvider { provider } => write!(
                formatter,
                "dependency MIR selection names provider {provider} outside the committed closure"
            ),
            Self::MissingExport {
                provider,
                declaration,
            } => write!(
                formatter,
                "provider {provider} has no LIR export for selected callable {declaration:?}"
            ),
            Self::BridgeMismatch {
                provider,
                declaration,
            } => write!(
                formatter,
                "provider {provider} changed the bridge contract of selected callable {declaration:?} between MIR and LIR"
            ),
            Self::Record(source) => write!(
                formatter,
                "cannot construct a selected dependency LIR record: {source}"
            ),
            Self::Selection(source) => write!(
                formatter,
                "cannot seal the selected dependency LIR set: {source}"
            ),
        }
    }
}

impl std::error::Error for CrossConeLirSelectionProjectionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Record(source) => Some(source),
            Self::Selection(source) => Some(source),
            Self::ConsumerMismatch { .. }
            | Self::MissingProvider { .. }
            | Self::MissingExport { .. }
            | Self::BridgeMismatch { .. } => None,
        }
    }
}
