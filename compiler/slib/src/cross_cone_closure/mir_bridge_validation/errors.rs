use std::fmt;

use scoop_hir::ExternalHirTargetV1;
use scoop_identity::{
    ConeIdentity, DependencyCallableDeclarationId, StrongCallableDefinitionOwner,
};

use crate::CrossConeMirFrontValidationError;

#[derive(Debug)]
pub enum CrossConeClosureMirBridgeError {
    Allocation {
        requested_slots: usize,
    },
    Artifact {
        identity: ConeIdentity,
        source: Box<CrossConeMirFrontValidationError>,
    },
    Relation {
        identity: ConeIdentity,
        source: Box<CrossConeMirClosureRelationError>,
    },
}

impl fmt::Display for CrossConeClosureMirBridgeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Allocation { requested_slots } => write!(
                formatter,
                "cannot allocate {requested_slots} cross-Cone MIR bridge validation slots"
            ),
            Self::Artifact { identity, source } => {
                write!(
                    formatter,
                    "invalid MIR bridge payload for {identity}: {source}"
                )
            }
            Self::Relation { identity, source } => {
                write!(
                    formatter,
                    "invalid MIR bridge closure for {identity}: {source}"
                )
            }
        }
    }
}

impl std::error::Error for CrossConeClosureMirBridgeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Artifact { source, .. } => Some(source.as_ref()),
            Self::Relation { source, .. } => Some(source.as_ref()),
            Self::Allocation { .. } => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CrossConeMirClosureRelationError {
    CallRoot {
        position: scoop_hir::concrete::ExecutableExpressionPosition,
    },
    CallSignature {
        position: Box<scoop_hir::concrete::ExecutableExpressionPosition>,
        provider: ConeIdentity,
        declaration: DependencyCallableDeclarationId,
    },
    NominalClassifier(scoop_hir::NominalExactLeafClassifierBuildError),
    NominalClassification(scoop_hir::NominalCallableClassificationError),
    Resource(scoop_wire::WireError),
    Allocation {
        requested_slots: usize,
    },
    StrongSignatureMismatch {
        declaration: DependencyCallableDeclarationId,
    },
    MissingMaximalExport {
        declaration: DependencyCallableDeclarationId,
    },
    UnexpectedExport {
        declaration: DependencyCallableDeclarationId,
    },
    ExportSignatureMismatch {
        declaration: DependencyCallableDeclarationId,
    },
    HirSelectionOriginMismatch {
        declaration: DependencyCallableDeclarationId,
        expected: ConeIdentity,
        actual: ConeIdentity,
    },
    UnsupportedHirSelection {
        target: ExternalHirTargetV1,
    },
    MissingMirSelection {
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
    SelectedImplementationMismatch {
        provider: ConeIdentity,
        declaration: DependencyCallableDeclarationId,
        implementations: Box<MirImplementationMismatch>,
    },
    SelectedSignatureMismatch {
        provider: ConeIdentity,
        declaration: DependencyCallableDeclarationId,
    },
}

impl fmt::Display for CrossConeMirClosureRelationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CallRoot { position } => write!(
                formatter,
                "HIR call {position:?} has no strong MIR executable root"
            ),
            Self::CallSignature {
                position,
                provider,
                declaration,
            } => write!(
                formatter,
                "HIR call {position:?} disagrees with the logical signature of {provider}:{declaration:?}"
            ),
            Self::NominalClassifier(source) => source.fmt(formatter),
            Self::NominalClassification(source) => source.fmt(formatter),
            Self::Resource(source) => source.fmt(formatter),
            Self::Allocation { requested_slots } => write!(
                formatter,
                "cannot allocate {requested_slots} MIR bridge relation slots"
            ),
            Self::StrongSignatureMismatch { declaration } => write!(
                formatter,
                "public callable {declaration:?} disagrees with its strong MIR signature"
            ),
            Self::MissingMaximalExport { declaration } => write!(
                formatter,
                "eligible public callable {declaration:?} is absent from the maximal MIR export set"
            ),
            Self::UnexpectedExport { declaration } => write!(
                formatter,
                "MIR export {declaration:?} is not an eligible public param-free callable"
            ),
            Self::ExportSignatureMismatch { declaration } => write!(
                formatter,
                "MIR export {declaration:?} disagrees with its HIR-derived exact signature"
            ),
            Self::HirSelectionOriginMismatch {
                declaration,
                expected,
                actual,
            } => write!(
                formatter,
                "MIR selection {declaration:?} names provider {expected}, but its HIR reference originates in {actual}"
            ),
            Self::UnsupportedHirSelection { target } => write!(
                formatter,
                "concrete HIR callable selection {target:?} has no M23-5 dependency-callable identity"
            ),
            Self::MissingMirSelection {
                provider,
                declaration,
            } => write!(
                formatter,
                "concrete HIR selection {provider}:{declaration:?} is absent from the MIR selected set"
            ),
            Self::MissingProvider {
                provider,
                declaration,
            } => write!(
                formatter,
                "MIR selection {declaration:?} names provider {provider}, which is absent from the closure"
            ),
            Self::UnreachableProvider {
                provider,
                declaration,
            } => write!(
                formatter,
                "MIR selection {declaration:?} names provider {provider}, which is not reachable from this artifact"
            ),
            Self::MissingProviderExport {
                provider,
                declaration,
            } => write!(
                formatter,
                "MIR selection {declaration:?} has no matching export in terminal provider {provider}"
            ),
            Self::SelectedImplementationMismatch {
                provider,
                declaration,
                implementations,
            } => write!(
                formatter,
                "MIR selection {provider}:{declaration:?} uses implementation {:?}, expected {:?}",
                implementations.actual, implementations.expected
            ),
            Self::SelectedSignatureMismatch {
                provider,
                declaration,
            } => write!(
                formatter,
                "MIR selection {provider}:{declaration:?} disagrees with the provider export signature"
            ),
        }
    }
}

impl std::error::Error for CrossConeMirClosureRelationError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MirImplementationMismatch {
    pub expected: StrongCallableDefinitionOwner,
    pub actual: StrongCallableDefinitionOwner,
}
