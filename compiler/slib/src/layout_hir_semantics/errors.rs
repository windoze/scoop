use std::fmt;

use scoop_hir::{TypeSectionPublicSupportError, TypeSectionSemanticValidationError};
use scoop_identity::ConeIdentity;

/// Complete old-public/new-type HIR closure validation failure.
#[derive(Debug)]
pub enum CrossConeLayoutHirSemanticClosureError<E> {
    Fields {
        provider: ConeIdentity,
        source: scoop_hir::NominalSourceFieldInventoryError,
    },
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
    AuthorityAllocation {
        provider: ConeIdentity,
        requested_slots: usize,
    },
    PublicAuthority {
        provider: ConeIdentity,
        source: E,
    },
    Public {
        provider: ConeIdentity,
        source: Box<TypeSectionPublicSupportError<E>>,
    },
    Intrinsics {
        provider: ConeIdentity,
        source: Box<crate::CrossConeIntrinsicDeclarationError>,
    },
    Types {
        provider: ConeIdentity,
        source: Box<TypeSectionSemanticValidationError<E>>,
    },
}

impl<E: fmt::Display> fmt::Display for CrossConeLayoutHirSemanticClosureError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Fields { provider, source } => {
                write!(formatter, "invalid nominal fields for {provider}: {source}")
            }
            Self::AuthorityCount { expected, actual } => write!(
                formatter,
                "layout HIR semantic authority count is {actual}, expected {expected}"
            ),
            Self::AuthorityProvider {
                position,
                expected,
                actual,
            } => write!(
                formatter,
                "layout HIR semantic authority {position} belongs to {actual}, expected {expected}"
            ),
            Self::Allocation { requested_slots } => write!(
                formatter,
                "cannot allocate {requested_slots} checked layout HIR provider slots"
            ),
            Self::AuthorityAllocation {
                provider,
                requested_slots,
            } => write!(
                formatter,
                "cannot allocate {requested_slots} layout HIR authority slots for {provider}"
            ),
            Self::PublicAuthority { provider, source } => write!(
                formatter,
                "cannot construct independent public HIR authority for {provider}: {source}"
            ),
            Self::Public { provider, source } => {
                write!(
                    formatter,
                    "invalid complete public HIR section for {provider}: {source}"
                )
            }
            Self::Types { provider, source } => {
                write!(
                    formatter,
                    "invalid complete type HIR section for {provider}: {source}"
                )
            }
            Self::Intrinsics { provider, source } => write!(
                formatter,
                "invalid intrinsic declarations for {provider}: {source}"
            ),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error
    for CrossConeLayoutHirSemanticClosureError<E>
{
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Fields { source, .. } => Some(source),
            Self::PublicAuthority { source, .. } => Some(source),
            Self::Public { source, .. } => Some(source.as_ref()),
            Self::Types { source, .. } => Some(source.as_ref()),
            Self::Intrinsics { source, .. } => Some(source.as_ref()),
            Self::AuthorityCount { .. }
            | Self::AuthorityProvider { .. }
            | Self::Allocation { .. }
            | Self::AuthorityAllocation { .. } => None,
        }
    }
}
