use std::fmt;

use scoop_identity::{ConeIdentity, DependencyCallableDeclarationId, GcEffect};

use crate::{CrossConeLirFrontValidationError, NativeBoundaryCompileError};

#[derive(Debug)]
pub enum CrossConeClosureLirBridgeError {
    Allocation {
        requested_slots: usize,
    },
    Artifact {
        identity: ConeIdentity,
        source: Box<CrossConeLirFrontValidationError>,
    },
    AbiReplay {
        identity: ConeIdentity,
        declaration: DependencyCallableDeclarationId,
        source: Box<NativeBoundaryCompileError>,
    },
    Relation {
        identity: ConeIdentity,
        source: Box<CrossConeLirClosureRelationError>,
    },
}

impl fmt::Display for CrossConeClosureLirBridgeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Allocation { requested_slots } => write!(
                formatter,
                "cannot allocate {requested_slots} cross-Cone LIR bridge validation slots"
            ),
            Self::Artifact { identity, source } => {
                write!(
                    formatter,
                    "invalid LIR bridge payload for {identity}: {source}"
                )
            }
            Self::AbiReplay {
                identity,
                declaration,
                source,
            } => write!(
                formatter,
                "cannot replay canonical ABI for {identity}:{declaration:?}: {source}"
            ),
            Self::Relation { identity, source } => {
                write!(
                    formatter,
                    "invalid LIR bridge closure for {identity}: {source}"
                )
            }
        }
    }
}

impl std::error::Error for CrossConeClosureLirBridgeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Artifact { source, .. } => Some(source.as_ref()),
            Self::AbiReplay { source, .. } => Some(source.as_ref()),
            Self::Relation { source, .. } => Some(source.as_ref()),
            Self::Allocation { .. } => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CrossConeLirClosureRelationError {
    Resource(scoop_wire::WireError),
    MissingLirExport {
        declaration: DependencyCallableDeclarationId,
    },
    UnexpectedLirExport {
        declaration: DependencyCallableDeclarationId,
    },
    ExportTargetMismatch {
        declaration: DependencyCallableDeclarationId,
    },
    ExportExactSignatureMismatch {
        declaration: DependencyCallableDeclarationId,
    },
    MissingExportCallableInterface {
        declaration: DependencyCallableDeclarationId,
    },
    ExportGcEffectMismatch {
        declaration: DependencyCallableDeclarationId,
        expected: GcEffect,
        actual: GcEffect,
    },
    ExportCallingConventionMismatch {
        declaration: DependencyCallableDeclarationId,
    },
    NonCanonicalExportAbi {
        declaration: DependencyCallableDeclarationId,
    },
    MissingLirSelection {
        provider: ConeIdentity,
        declaration: DependencyCallableDeclarationId,
    },
    UnexpectedLirSelection {
        provider: ConeIdentity,
        declaration: DependencyCallableDeclarationId,
    },
    SelectedTargetMismatch {
        provider: ConeIdentity,
        declaration: DependencyCallableDeclarationId,
    },
    SelectedExactSignatureMismatch {
        provider: ConeIdentity,
        declaration: DependencyCallableDeclarationId,
    },
    MissingProvider {
        provider: ConeIdentity,
        declaration: DependencyCallableDeclarationId,
    },
    UnreachableProvider {
        provider: ConeIdentity,
        declaration: DependencyCallableDeclarationId,
    },
    MissingProviderExport {
        provider: ConeIdentity,
        declaration: DependencyCallableDeclarationId,
    },
    SelectedProviderExportMismatch {
        provider: ConeIdentity,
        declaration: DependencyCallableDeclarationId,
    },
}

impl fmt::Display for CrossConeLirClosureRelationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid MIR/LIR dependency bridge relation: {self:?}"
        )
    }
}

impl std::error::Error for CrossConeLirClosureRelationError {}
