use std::fmt;

use scoop_identity::{ConeIdentity, DependencyCallableDeclarationId};
use scoop_lir::{
    CrossConeLirBridgeValidationError, LayoutAbiSectionError, StrongProductionLayoutJoinError,
    StrongProductionSectionValidationError,
};

use crate::{
    CrossConeLayoutMirSemanticClosureError, CrossConeLirClosureRelationError,
    NativeBoundaryCompileError,
};

pub type CrossConeLayoutLirSemanticClosureResult<T, HE, ME, LE> =
    Result<T, CrossConeLayoutLirSemanticClosureError<HE, ME, LE>>;

#[derive(Debug)]
pub enum CrossConeLayoutLirSemanticClosureError<HE, ME, LE> {
    Mir(Box<CrossConeLayoutMirSemanticClosureError<HE, ME>>),
    AuthorityCount {
        expected: usize,
        actual: usize,
    },
    AuthorityProvider {
        position: usize,
        expected: ConeIdentity,
        actual: ConeIdentity,
    },
    Allocation {
        requested_slots: usize,
    },
    DependencyAllocation {
        provider: ConeIdentity,
        requested_slots: usize,
    },
    OrdinaryBridge {
        provider: ConeIdentity,
        source: Box<CrossConeLirBridgeValidationError>,
    },
    Relation {
        provider: ConeIdentity,
        source: Box<CrossConeLirClosureRelationError>,
    },
    MissingTrustedCore,
    AbiReplay {
        provider: ConeIdentity,
        declaration: DependencyCallableDeclarationId,
        source: Box<NativeBoundaryCompileError>,
    },
    SourceAuthority {
        provider: ConeIdentity,
        source: Box<LE>,
    },
    SourceProvider {
        provider: ConeIdentity,
        actual: ConeIdentity,
    },
    StrongReplay {
        provider: ConeIdentity,
        source: Box<StrongProductionSectionValidationError>,
    },
    LayoutAbi {
        provider: ConeIdentity,
        source: Box<LayoutAbiSectionError<LE>>,
    },
    StrongLayoutJoin {
        provider: ConeIdentity,
        source: Box<StrongProductionLayoutJoinError>,
    },
}

impl<HE: fmt::Display, ME: fmt::Display + fmt::Debug, LE: fmt::Display + fmt::Debug> fmt::Display
    for CrossConeLayoutLirSemanticClosureError<HE, ME, LE>
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Mir(source) => source.fmt(formatter),
            Self::AuthorityCount { expected, actual } => write!(
                formatter,
                "expected {expected} LIR source authorities, found {actual}"
            ),
            Self::AuthorityProvider {
                position,
                expected,
                actual,
            } => write!(
                formatter,
                "LIR source authority {position} belongs to {actual}, expected {expected}"
            ),
            Self::Allocation { requested_slots } => write!(
                formatter,
                "cannot allocate {requested_slots} layout-profile LIR closure slots"
            ),
            Self::DependencyAllocation {
                provider,
                requested_slots,
            } => write!(
                formatter,
                "cannot allocate {requested_slots} LIR dependency views for {provider}"
            ),
            Self::OrdinaryBridge { provider, source } => {
                write!(
                    formatter,
                    "invalid ordinary LIR bridge for {provider}: {source}"
                )
            }
            Self::Relation { provider, source } => {
                write!(
                    formatter,
                    "invalid MIR/LIR relation for {provider}: {source}"
                )
            }
            Self::MissingTrustedCore => {
                formatter.write_str("layout-profile LIR closure has no trusted-core provider")
            }
            Self::AbiReplay {
                provider,
                declaration,
                source,
            } => write!(
                formatter,
                "cannot replay canonical ABI for {provider}:{declaration:?}: {source}"
            ),
            Self::SourceAuthority { provider, source } => {
                write!(
                    formatter,
                    "cannot build LIR source authority for {provider}: {source}"
                )
            }
            Self::SourceProvider { provider, actual } => write!(
                formatter,
                "LIR replay authority for {provider} belongs to {actual}"
            ),
            Self::StrongReplay { provider, source } => {
                write!(
                    formatter,
                    "invalid Strong V2 section for {provider}: {source}"
                )
            }
            Self::LayoutAbi { provider, source } => {
                write!(
                    formatter,
                    "invalid layout/ABI section for {provider}: {source}"
                )
            }
            Self::StrongLayoutJoin { provider, source } => write!(
                formatter,
                "invalid Strong V2 layout/ABI join for {provider}: {source}"
            ),
        }
    }
}

impl<HE, ME, LE> std::error::Error for CrossConeLayoutLirSemanticClosureError<HE, ME, LE>
where
    HE: std::error::Error + 'static,
    ME: std::error::Error + 'static,
    LE: std::error::Error + 'static,
{
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Mir(source) => Some(source.as_ref()),
            Self::OrdinaryBridge { source, .. } => Some(source.as_ref()),
            Self::Relation { source, .. } => Some(source.as_ref()),
            Self::AbiReplay { source, .. } => Some(source.as_ref()),
            Self::SourceAuthority { source, .. } => Some(source.as_ref()),
            Self::StrongReplay { source, .. } => Some(source.as_ref()),
            Self::LayoutAbi { source, .. } => Some(source.as_ref()),
            Self::StrongLayoutJoin { source, .. } => Some(source.as_ref()),
            Self::AuthorityCount { .. }
            | Self::AuthorityProvider { .. }
            | Self::Allocation { .. }
            | Self::DependencyAllocation { .. }
            | Self::MissingTrustedCore
            | Self::SourceProvider { .. } => None,
        }
    }
}
