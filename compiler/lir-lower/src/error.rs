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
    StorageReplay(StorageLoweringError),
    MissingImportedCoreLirAuthority,
    CoreCannotImportCore,
    ImportedCoreLirCountMismatch {
        mir: usize,
        lir: usize,
    },
    MissingImportedCoreLirCallable {
        index: usize,
        kind: scoop_identity::CoreImportedCallableKind,
    },
    ImportedCoreLirCallableMismatch {
        index: usize,
        kind: scoop_identity::CoreImportedCallableKind,
    },
    MissingImportedCoreParameterType {
        index: usize,
        parameter: usize,
        exact: scoop_identity::PersistentExactTypeId,
    },
    MissingImportedCoreResultType {
        index: usize,
        exact: scoop_identity::PersistentExactTypeId,
    },
    ImportedCoreCallable(lir::ExternalCallableBuildError),
    ImportedCoreTypeDescriptor(lir::ExternalTypeDescriptorBuildError),
    ImportedCoreRuntimeStringMismatch {
        mir: scoop_identity::PersistentExactTypeId,
        lir: scoop_identity::PersistentExactTypeId,
    },
    MissingImportedDependencyLirAuthority,
    ForeignImportedDependencyLirSelection {
        expected: scoop_identity::ConeIdentity,
        actual: scoop_identity::ConeIdentity,
    },
    ImportedDependencyLirCountMismatch {
        mir: usize,
        lir: usize,
    },
    MissingImportedDependencyLirCallable {
        index: usize,
        provider: scoop_identity::ConeIdentity,
        declaration: scoop_identity::DependencyCallableDeclarationId,
    },
    ImportedDependencyLirCallableMismatch {
        index: usize,
        provider: scoop_identity::ConeIdentity,
        declaration: scoop_identity::DependencyCallableDeclarationId,
    },
    ImportedDependencyLirGcEffectMismatch {
        index: usize,
        provider: scoop_identity::ConeIdentity,
        declaration: scoop_identity::DependencyCallableDeclarationId,
        mir: mir::GcEffect,
        lir: scoop_identity::GcEffect,
    },
    MissingImportedDependencyArgumentType {
        index: usize,
        argument: usize,
        exact: scoop_identity::PersistentExactTypeId,
    },
    MissingImportedDependencyResultType {
        index: usize,
        exact: scoop_identity::PersistentExactTypeId,
    },
    ImportedDependencyCallable(lir::ExternalCallableBuildError),
    MissingRuntimeStringDescriptor {
        producer: scoop_identity::ConeIdentity,
    },
    RuntimeStringDescriptorOwnership {
        producer: scoop_identity::ConeIdentity,
    },
    MissingCoreCallableMaterialization(scoop_identity::CallableOwner),
    MissingCoreCallableSignature(scoop_identity::CallableOwner),
    UnsupportedCoreCallableEffect(scoop_identity::CallableOwner),
    UnsupportedCoreCallableReceiver(scoop_identity::CallableOwner),
    UnsupportedCoreCallableOwner(scoop_identity::CallableOwner),
    CoreLirBridge(lir::CoreLirBridgeBuildError),
    Output(lir::SingleConeStrongLirOutputError),
}

impl fmt::Display for StrongLirLoweringError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Capability(source) => source.fmt(formatter),
            Self::StorageReplay(source) => source.fmt(formatter),
            Self::ImportedCoreCallable(source) => source.fmt(formatter),
            Self::ImportedCoreTypeDescriptor(source) => source.fmt(formatter),
            Self::ImportedDependencyCallable(source) => source.fmt(formatter),
            Self::CoreLirBridge(source) => source.fmt(formatter),
            Self::MissingImportedCoreLirAuthority => formatter
                .write_str("imported-core MIR roots require the exact selected LIR authority"),
            Self::CoreCannotImportCore => {
                formatter.write_str("the core bootstrap Cone cannot import core callables")
            }
            Self::ImportedCoreLirCountMismatch { mir, lir } => write!(
                formatter,
                "imported-core selection count mismatch: MIR has {mir}, LIR authority has {lir}"
            ),
            Self::MissingImportedCoreLirCallable { index, kind } => write!(
                formatter,
                "imported-core MIR callable {index} kind {kind:?} has no LIR authority"
            ),
            Self::ImportedCoreLirCallableMismatch { index, kind } => write!(
                formatter,
                "imported-core MIR callable {index} kind {kind:?} disagrees with its LIR authority"
            ),
            Self::MissingImportedCoreParameterType {
                index,
                parameter,
                exact,
            } => write!(
                formatter,
                "imported-core MIR callable {index} parameter {parameter} exact type {exact} has no MIR type relation"
            ),
            Self::MissingImportedCoreResultType { index, exact } => write!(
                formatter,
                "imported-core MIR callable {index} result {exact} has no exact MIR type relation"
            ),
            Self::ImportedCoreRuntimeStringMismatch { mir, lir } => write!(
                formatter,
                "runtime String exact type mismatch: MIR requires {mir}, LIR authority provides {lir}"
            ),
            Self::MissingImportedDependencyLirAuthority => formatter.write_str(
                "ordinary dependency MIR roots require the exact selected LIR authority",
            ),
            Self::ForeignImportedDependencyLirSelection { expected, actual } => write!(
                formatter,
                "ordinary dependency LIR selection belongs to consumer {actual}, expected {expected}"
            ),
            Self::ImportedDependencyLirCountMismatch { mir, lir } => write!(
                formatter,
                "ordinary dependency selection count mismatch: MIR has {mir}, LIR authority has {lir}"
            ),
            Self::MissingImportedDependencyLirCallable {
                index,
                provider,
                declaration,
            } => write!(
                formatter,
                "ordinary dependency MIR callable {index} ({provider}, {declaration:?}) has no LIR authority"
            ),
            Self::ImportedDependencyLirCallableMismatch {
                index,
                provider,
                declaration,
            } => write!(
                formatter,
                "ordinary dependency MIR callable {index} ({provider}, {declaration:?}) disagrees with its LIR authority"
            ),
            Self::ImportedDependencyLirGcEffectMismatch {
                index,
                provider,
                declaration,
                mir,
                lir,
            } => write!(
                formatter,
                "ordinary dependency MIR callable {index} ({provider}, {declaration:?}) has GC effect {mir:?}, but its LIR authority requires {lir:?}"
            ),
            Self::MissingImportedDependencyArgumentType {
                index,
                argument,
                exact,
            } => write!(
                formatter,
                "ordinary dependency MIR callable {index} argument {argument} exact type {exact} has no MIR type relation"
            ),
            Self::MissingImportedDependencyResultType { index, exact } => write!(
                formatter,
                "ordinary dependency MIR callable {index} result {exact} has no exact MIR type relation"
            ),
            Self::MissingRuntimeStringDescriptor { producer } => write!(
                formatter,
                "Cone {producer} has no complete runtime String TypeDescriptor authority"
            ),
            Self::RuntimeStringDescriptorOwnership { producer } => write!(
                formatter,
                "Cone {producer} has an invalid local/external runtime String TypeDescriptor branch"
            ),
            Self::MissingCoreCallableMaterialization(owner) => {
                write!(
                    formatter,
                    "core callable {owner:?} has no strong materialization"
                )
            }
            Self::MissingCoreCallableSignature(owner) => {
                write!(formatter, "core callable {owner:?} has no exact signature")
            }
            Self::UnsupportedCoreCallableEffect(owner) => write!(
                formatter,
                "core callable {owner:?} has an unsupported suspend ABI"
            ),
            Self::UnsupportedCoreCallableReceiver(owner) => write!(
                formatter,
                "core callable {owner:?} has an unsupported receiver ABI"
            ),
            Self::UnsupportedCoreCallableOwner(owner) => write!(
                formatter,
                "core callable {owner:?} has no strong definition owner"
            ),
            Self::Output(source) => source.fmt(formatter),
        }
    }
}

impl std::error::Error for StrongLirLoweringError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Capability(source) => Some(source),
            Self::StorageReplay(source) => Some(source),
            Self::ImportedCoreCallable(source) => Some(source),
            Self::ImportedCoreTypeDescriptor(source) => Some(source),
            Self::ImportedDependencyCallable(source) => Some(source),
            Self::CoreLirBridge(source) => Some(source),
            Self::Output(source) => Some(source),
            Self::MissingCoreCallableMaterialization(_)
            | Self::MissingImportedCoreLirAuthority
            | Self::CoreCannotImportCore
            | Self::ImportedCoreLirCountMismatch { .. }
            | Self::MissingImportedCoreLirCallable { .. }
            | Self::ImportedCoreLirCallableMismatch { .. }
            | Self::MissingImportedCoreParameterType { .. }
            | Self::MissingImportedCoreResultType { .. }
            | Self::ImportedCoreRuntimeStringMismatch { .. }
            | Self::MissingImportedDependencyLirAuthority
            | Self::ForeignImportedDependencyLirSelection { .. }
            | Self::ImportedDependencyLirCountMismatch { .. }
            | Self::MissingImportedDependencyLirCallable { .. }
            | Self::ImportedDependencyLirCallableMismatch { .. }
            | Self::ImportedDependencyLirGcEffectMismatch { .. }
            | Self::MissingImportedDependencyArgumentType { .. }
            | Self::MissingImportedDependencyResultType { .. }
            | Self::MissingRuntimeStringDescriptor { .. }
            | Self::RuntimeStringDescriptorOwnership { .. }
            | Self::MissingCoreCallableSignature(_)
            | Self::UnsupportedCoreCallableEffect(_)
            | Self::UnsupportedCoreCallableReceiver(_)
            | Self::UnsupportedCoreCallableOwner(_) => None,
        }
    }
}
