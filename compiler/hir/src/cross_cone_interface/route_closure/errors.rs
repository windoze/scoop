use std::fmt;

use scoop_identity::{
    BindableEntity, BindingNamespace, BindingRole, ConeIdentity, PersistentExportBindingId,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PublicExportBindingClosureValidationError {
    MissingCurrentBindingKey {
        binding: PersistentExportBindingId,
    },
    CurrentBindingExporterMismatch {
        binding: PersistentExportBindingId,
        expected: ConeIdentity,
        actual: ConeIdentity,
    },
    DeclaredTargetMismatch {
        exporter: ConeIdentity,
        binding: PersistentExportBindingId,
        expected: BindableEntity,
        actual: Box<BindableEntity>,
    },
    ImmediateProviderNotDirect {
        binding: PersistentExportBindingId,
        route: usize,
        provider: ConeIdentity,
    },
    RouteExceedsClosure {
        binding: PersistentExportBindingId,
        route: usize,
        hops: usize,
        closure_nodes: usize,
    },
    MissingHopBindingKey {
        binding: PersistentExportBindingId,
        route: usize,
        hop: usize,
        hop_binding: PersistentExportBindingId,
    },
    HopBindingExporterMismatch {
        binding: PersistentExportBindingId,
        route: usize,
        hop: usize,
        hop_binding: PersistentExportBindingId,
        expected: ConeIdentity,
        actual: Box<ConeIdentity>,
    },
    HopTargetMismatch {
        binding: PersistentExportBindingId,
        route: usize,
        hop: usize,
        hop_binding: PersistentExportBindingId,
        expected: BindableEntity,
        actual: Box<BindableEntity>,
    },
    HopNamespaceMismatch {
        binding: PersistentExportBindingId,
        route: usize,
        hop: usize,
        hop_binding: PersistentExportBindingId,
        expected: BindingNamespace,
        actual: BindingNamespace,
    },
    HopRoleMismatch {
        binding: PersistentExportBindingId,
        route: usize,
        hop: usize,
        hop_binding: PersistentExportBindingId,
        expected: BindingRole,
        actual: BindingRole,
    },
    MissingProviderSurface {
        binding: PersistentExportBindingId,
        route: usize,
        hop: usize,
        provider: ConeIdentity,
    },
    MissingProviderBinding {
        binding: PersistentExportBindingId,
        route: usize,
        hop: usize,
        provider: ConeIdentity,
        hop_binding: PersistentExportBindingId,
    },
    TerminalIsReexport {
        binding: PersistentExportBindingId,
        route: usize,
        hop: usize,
        hop_binding: PersistentExportBindingId,
    },
    IntermediateIsDeclared {
        binding: PersistentExportBindingId,
        route: usize,
        hop: usize,
        hop_binding: PersistentExportBindingId,
    },
    MissingRouteSuffix {
        binding: PersistentExportBindingId,
        route: usize,
        hop: usize,
        hop_binding: PersistentExportBindingId,
    },
}

impl fmt::Display for PublicExportBindingClosureValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingCurrentBindingKey { binding } => {
                write!(formatter, "public binding {binding} has no canonical key")
            }
            Self::CurrentBindingExporterMismatch {
                binding,
                expected,
                actual,
            } => write!(
                formatter,
                "public binding {binding} is exported by Cone {actual}, not current Cone {expected}"
            ),
            Self::DeclaredTargetMismatch {
                exporter,
                binding,
                expected,
                actual,
            } => write!(
                formatter,
                "declared binding {binding} from Cone {exporter} targets {expected:?}, but its source names {actual:?}"
            ),
            Self::ImmediateProviderNotDirect {
                binding,
                route,
                provider,
            } => write!(
                formatter,
                "route {route} of binding {binding} begins at non-direct provider Cone {provider}"
            ),
            Self::RouteExceedsClosure {
                binding,
                route,
                hops,
                closure_nodes,
            } => write!(
                formatter,
                "route {route} of binding {binding} has {hops} hops but the closure has only {closure_nodes} nodes"
            ),
            Self::MissingHopBindingKey {
                binding,
                route,
                hop,
                hop_binding,
            } => write!(
                formatter,
                "hop {hop} of route {route} for binding {binding} names binding {hop_binding} without a canonical key"
            ),
            Self::HopBindingExporterMismatch {
                binding,
                route,
                hop,
                hop_binding,
                expected,
                actual,
            } => write!(
                formatter,
                "hop {hop} of route {route} for binding {binding} assigns binding {hop_binding} to Cone {expected}, but its key is exported by Cone {actual}"
            ),
            Self::HopTargetMismatch {
                binding,
                route,
                hop,
                hop_binding,
                expected,
                actual,
            } => write!(
                formatter,
                "hop {hop} of route {route} for binding {binding} uses binding {hop_binding} with target {actual:?}, expected {expected:?}"
            ),
            Self::HopNamespaceMismatch {
                binding,
                route,
                hop,
                hop_binding,
                expected,
                actual,
            } => write!(
                formatter,
                "hop {hop} of route {route} for binding {binding} uses binding {hop_binding} in namespace {actual:?}, expected {expected:?}"
            ),
            Self::HopRoleMismatch {
                binding,
                route,
                hop,
                hop_binding,
                expected,
                actual,
            } => write!(
                formatter,
                "hop {hop} of route {route} for binding {binding} uses binding {hop_binding} with role {actual:?}, expected {expected:?}"
            ),
            Self::MissingProviderSurface {
                binding,
                route,
                hop,
                provider,
            } => write!(
                formatter,
                "hop {hop} of route {route} for binding {binding} has no public surface for Cone {provider}"
            ),
            Self::MissingProviderBinding {
                binding,
                route,
                hop,
                provider,
                hop_binding,
            } => write!(
                formatter,
                "hop {hop} of route {route} for binding {binding} names absent binding {hop_binding} in Cone {provider}"
            ),
            Self::TerminalIsReexport {
                binding,
                route,
                hop,
                hop_binding,
            } => write!(
                formatter,
                "terminal hop {hop} of route {route} for binding {binding} is re-export binding {hop_binding}"
            ),
            Self::IntermediateIsDeclared {
                binding,
                route,
                hop,
                hop_binding,
            } => write!(
                formatter,
                "intermediate hop {hop} of route {route} for binding {binding} is declared binding {hop_binding}"
            ),
            Self::MissingRouteSuffix {
                binding,
                route,
                hop,
                hop_binding,
            } => write!(
                formatter,
                "intermediate binding {hop_binding} at hop {hop} of route {route} for binding {binding} does not publish the exact remaining suffix"
            ),
        }
    }
}

impl std::error::Error for PublicExportBindingClosureValidationError {}
