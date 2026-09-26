use std::fmt;

use scoop_lir as lir;
use scoop_mir as mir;

use crate::StrongLirCapabilityError;

mod storage;
pub use storage::StorageLoweringError;
pub(crate) use storage::StorageResult;

#[derive(Debug)]
pub enum StrongLirLoweringError {
    Capability(StrongLirCapabilityError),
    DependencyLayout(lir::LayoutExternalMaterializationError),
    DependencyLayoutConsumer {
        expected: scoop_identity::ConeIdentity,
        actual: scoop_identity::ConeIdentity,
    },
    DependencyLayoutTarget {
        expected: lir::LirTargetProfile,
        actual: lir::LirTargetProfile,
    },
    MissingDependencyLayoutSelection {
        provider: scoop_identity::ConeIdentity,
        exact: scoop_identity::PersistentExactTypeId,
    },
    MissingDependencyValueLayout {
        provider: scoop_identity::ConeIdentity,
        layout: scoop_identity::PersistentLayoutId,
    },
    DependencyDescriptorBinding(scoop_identity::PersistentExactTypeId),
    Diagnostic(scoop_identity::ExactTypeDiagnosticError),
    StorageReplay(StorageLoweringError),

    ForeignExternalLirSelection {
        expected: scoop_identity::ConeIdentity,
        actual: scoop_identity::ConeIdentity,
    },
    ExternalCallableCountMismatch {
        mir: usize,
        lir: usize,
    },
    MissingExternalCallable {
        index: usize,
        provider: scoop_identity::ConeIdentity,
        target: scoop_identity::StrongCallableDefinitionOwner,
    },
    ExternalCallableMismatch {
        index: usize,
        provider: scoop_identity::ConeIdentity,
        target: scoop_identity::StrongCallableDefinitionOwner,
    },
    ExternalCallableGcEffectMismatch {
        index: usize,
        provider: scoop_identity::ConeIdentity,
        target: scoop_identity::StrongCallableDefinitionOwner,
        mir: mir::GcEffect,
        lir: scoop_identity::GcEffect,
    },
    MissingExternalArgumentType {
        index: usize,
        argument: usize,
        exact: scoop_identity::PersistentExactTypeId,
    },
    MissingExternalResultType {
        index: usize,
        exact: scoop_identity::PersistentExactTypeId,
    },
    ExternalCallable(lir::ExternalCallableBuildError),
    MissingRuntimeStringDescriptor {
        producer: scoop_identity::ConeIdentity,
    },

    InvalidInitializationCallable(scoop_identity::CallableOwner),
    CallableAbi(crate::CallableAbiProjectionError),
    Output(lir::SingleConeStrongLirOutputError),
}

impl fmt::Display for StrongLirLoweringError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Capability(source) => source.fmt(formatter),
            Self::DependencyLayout(source) => source.fmt(formatter),
            Self::DependencyLayoutConsumer { expected, actual } => write!(
                formatter,
                "layout selection belongs to consumer {actual}, expected {expected}",
            ),
            Self::DependencyLayoutTarget { expected, actual } => write!(
                formatter,
                "layout selection target {actual:?} differs from {expected:?}",
            ),
            Self::MissingDependencyLayoutSelection { provider, exact } => write!(
                formatter,
                "dependency descriptor {exact} from {provider} requires a complete layout selection",
            ),
            Self::MissingDependencyValueLayout { provider, layout } => write!(
                formatter,
                "value layout {layout} from {provider} is absent from the dependency selection"
            ),
            Self::DependencyDescriptorBinding(exact) => write!(
                formatter,
                "dependency descriptor {exact} disagrees with its MIR or external arena binding",
            ),
            Self::Diagnostic(source) => source.fmt(formatter),
            Self::StorageReplay(source) => source.fmt(formatter),
            Self::ExternalCallable(source) => source.fmt(formatter),
            Self::CallableAbi(source) => source.fmt(formatter),

            Self::ForeignExternalLirSelection { expected, actual } => write!(
                formatter,
                "external callable LIR selection belongs to consumer {actual}, expected {expected}"
            ),
            Self::ExternalCallableCountMismatch { mir, lir } => write!(
                formatter,
                "external callable selection count mismatch: MIR has {mir}, LIR authority has {lir}"
            ),
            Self::MissingExternalCallable {
                index,
                provider,
                target,
            } => write!(
                formatter,
                "external callable MIR callable {index} ({provider}, {target:?}) has no LIR authority"
            ),
            Self::ExternalCallableMismatch {
                index,
                provider,
                target,
            } => write!(
                formatter,
                "external callable MIR callable {index} ({provider}, {target:?}) disagrees with its LIR authority"
            ),
            Self::ExternalCallableGcEffectMismatch {
                index,
                provider,
                target,
                mir,
                lir,
            } => write!(
                formatter,
                "external callable MIR callable {index} ({provider}, {target:?}) has GC effect {mir:?}, but its LIR authority requires {lir:?}"
            ),
            Self::MissingExternalArgumentType {
                index,
                argument,
                exact,
            } => write!(
                formatter,
                "external callable MIR callable {index} argument {argument} exact type {exact} has no MIR type relation"
            ),
            Self::MissingExternalResultType { index, exact } => write!(
                formatter,
                "external callable MIR callable {index} result {exact} has no exact MIR type relation"
            ),
            Self::MissingRuntimeStringDescriptor { producer } => write!(
                formatter,
                "Cone {producer} has no complete descriptor for the typed String declaration"
            ),

            Self::InvalidInitializationCallable(owner) => write!(
                formatter,
                "initialization service {owner:?} must be an ordinary function without a receiver"
            ),
            Self::Output(source) => source.fmt(formatter),
        }
    }
}

impl std::error::Error for StrongLirLoweringError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Capability(source) => Some(source),
            Self::DependencyLayout(source) => Some(source),
            Self::Diagnostic(source) => Some(source),
            Self::StorageReplay(source) => Some(source),
            Self::ExternalCallable(source) => Some(source),
            Self::CallableAbi(source) => Some(source),
            Self::Output(source) => Some(source),
            Self::InvalidInitializationCallable(_)
            | Self::DependencyLayoutConsumer { .. }
            | Self::DependencyLayoutTarget { .. }
            | Self::MissingDependencyValueLayout { .. }
            | Self::MissingDependencyLayoutSelection { .. }
            | Self::DependencyDescriptorBinding(_)
            | Self::ForeignExternalLirSelection { .. }
            | Self::ExternalCallableCountMismatch { .. }
            | Self::MissingExternalCallable { .. }
            | Self::ExternalCallableMismatch { .. }
            | Self::ExternalCallableGcEffectMismatch { .. }
            | Self::MissingExternalArgumentType { .. }
            | Self::MissingExternalResultType { .. }
            | Self::MissingRuntimeStringDescriptor { .. } => None,
        }
    }
}
