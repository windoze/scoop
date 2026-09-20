use std::fmt;

use scoop_identity::ConeIdentity;
use scoop_mir::MirTypeBridgeSectionError;

use crate::{CrossConeLayoutHirSemanticClosureError, CrossConeLayoutMirFrontValidationError};

#[derive(Debug)]
pub enum CrossConeLayoutMirSemanticClosureError<HE, ME> {
    Hir(Box<CrossConeLayoutHirSemanticClosureError<HE>>),
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
    Front {
        provider: ConeIdentity,
        source: Box<CrossConeLayoutMirFrontValidationError>,
    },
    SourceAuthority {
        provider: ConeIdentity,
        source: Box<ME>,
    },
    TypeBridge {
        provider: ConeIdentity,
        source: Box<MirTypeBridgeSectionError<ME>>,
    },
}

impl<HE: fmt::Display, ME: fmt::Display + fmt::Debug> fmt::Display
    for CrossConeLayoutMirSemanticClosureError<HE, ME>
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Hir(source) => source.fmt(formatter),
            Self::AuthorityCount { expected, actual } => write!(
                formatter,
                "expected {expected} MIR source authorities, found {actual}"
            ),
            Self::AuthorityProvider {
                position,
                expected,
                actual,
            } => write!(
                formatter,
                "MIR source authority {position} belongs to {actual}, expected {expected}"
            ),
            Self::Allocation { requested_slots } => write!(
                formatter,
                "cannot allocate {requested_slots} layout-profile MIR closure slots"
            ),
            Self::DependencyAllocation {
                provider,
                requested_slots,
            } => write!(
                formatter,
                "cannot allocate {requested_slots} MIR dependency views for {provider}"
            ),
            Self::Front { provider, source } => {
                write!(formatter, "invalid MIR front for {provider}: {source}")
            }
            Self::SourceAuthority { provider, source } => {
                write!(
                    formatter,
                    "cannot build MIR source authority for {provider}: {source}"
                )
            }
            Self::TypeBridge { provider, source } => {
                write!(
                    formatter,
                    "invalid MIR type bridge for {provider}: {source}"
                )
            }
        }
    }
}

impl<HE, ME> std::error::Error for CrossConeLayoutMirSemanticClosureError<HE, ME>
where
    HE: std::error::Error + 'static,
    ME: std::error::Error + 'static,
{
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Hir(source) => Some(source.as_ref()),
            Self::Front { source, .. } => Some(source.as_ref()),
            Self::SourceAuthority { source, .. } => Some(source.as_ref()),
            Self::TypeBridge { source, .. } => Some(source.as_ref()),
            Self::AuthorityCount { .. }
            | Self::AuthorityProvider { .. }
            | Self::Allocation { .. }
            | Self::DependencyAllocation { .. } => None,
        }
    }
}
