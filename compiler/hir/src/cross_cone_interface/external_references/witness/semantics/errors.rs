use std::fmt;

use scoop_identity::{
    BindableEntity, BindingNamespace, BindingRole, ConeIdentity, PersistentExportBindingId,
};
use scoop_wire::WireError;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DependencyBindingWitnessSemanticValidationError {
    Resource(WireError),
    RouteExceedsClosure {
        hops: usize,
        closure_nodes: usize,
    },
    MissingHopBindingKey {
        hop: usize,
        binding: PersistentExportBindingId,
    },
    HopBindingExporterMismatch {
        hop: usize,
        binding: PersistentExportBindingId,
        expected: ConeIdentity,
        actual: ConeIdentity,
    },
    HopTargetMismatch {
        hop: usize,
        binding: PersistentExportBindingId,
        expected: BindableEntity,
        actual: Box<BindableEntity>,
    },
    HopNamespaceMismatch {
        hop: usize,
        binding: PersistentExportBindingId,
        expected: BindingNamespace,
        actual: BindingNamespace,
    },
    HopRoleMismatch {
        hop: usize,
        binding: PersistentExportBindingId,
        expected: BindingRole,
        actual: BindingRole,
    },
    MissingProviderSurface {
        hop: usize,
        provider: ConeIdentity,
    },
    MissingProviderBinding {
        hop: usize,
        provider: ConeIdentity,
        binding: PersistentExportBindingId,
    },
    TerminalIsReexport {
        hop: usize,
        binding: PersistentExportBindingId,
    },
    IntermediateIsDeclared {
        hop: usize,
        binding: PersistentExportBindingId,
    },
    MissingRouteSuffix {
        hop: usize,
        binding: PersistentExportBindingId,
    },
    DeclaredTargetMismatch {
        hop: usize,
        binding: PersistentExportBindingId,
        expected: BindableEntity,
        actual: Box<BindableEntity>,
    },
}

impl fmt::Display for DependencyBindingWitnessSemanticValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(formatter),
            Self::RouteExceedsClosure {
                hops,
                closure_nodes,
            } => write!(
                formatter,
                "dependency binding witness has {hops} hops but the closure has only {closure_nodes} nodes"
            ),
            Self::MissingHopBindingKey { hop, binding } => write!(
                formatter,
                "dependency binding witness hop {hop} names binding {binding} without a canonical key"
            ),
            Self::HopBindingExporterMismatch {
                hop,
                binding,
                expected,
                actual,
            } => write!(
                formatter,
                "dependency binding witness hop {hop} assigns binding {binding} to Cone {expected}, but its key is exported by Cone {actual}"
            ),
            Self::HopTargetMismatch {
                hop,
                binding,
                expected,
                actual,
            } => write!(
                formatter,
                "dependency binding witness hop {hop} uses binding {binding} with target {actual:?}, expected {expected:?}"
            ),
            Self::HopNamespaceMismatch {
                hop,
                binding,
                expected,
                actual,
            } => write!(
                formatter,
                "dependency binding witness hop {hop} uses binding {binding} in namespace {actual:?}, expected {expected:?}"
            ),
            Self::HopRoleMismatch {
                hop,
                binding,
                expected,
                actual,
            } => write!(
                formatter,
                "dependency binding witness hop {hop} uses binding {binding} with role {actual:?}, expected {expected:?}"
            ),
            Self::MissingProviderSurface { hop, provider } => write!(
                formatter,
                "dependency binding witness hop {hop} has no public surface for Cone {provider}"
            ),
            Self::MissingProviderBinding {
                hop,
                provider,
                binding,
            } => write!(
                formatter,
                "dependency binding witness hop {hop} names absent binding {binding} in Cone {provider}"
            ),
            Self::TerminalIsReexport { hop, binding } => write!(
                formatter,
                "dependency binding witness terminal hop {hop} is re-export binding {binding}"
            ),
            Self::IntermediateIsDeclared { hop, binding } => write!(
                formatter,
                "dependency binding witness intermediate hop {hop} is declared binding {binding}"
            ),
            Self::MissingRouteSuffix { hop, binding } => write!(
                formatter,
                "dependency binding witness intermediate binding {binding} at hop {hop} does not publish the exact remaining suffix"
            ),
            Self::DeclaredTargetMismatch {
                hop,
                binding,
                expected,
                actual,
            } => write!(
                formatter,
                "dependency binding witness terminal hop {hop} binding {binding} declares {actual:?}, expected {expected:?}"
            ),
        }
    }
}

impl From<WireError> for DependencyBindingWitnessSemanticValidationError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}

impl std::error::Error for DependencyBindingWitnessSemanticValidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            _ => None,
        }
    }
}
