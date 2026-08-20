//! Errors shared by the closure-wide declaration-surface states.

use std::fmt;

use scoop_identity::ConeIdentity;

use crate::{
    CrossConeHirCallableSurfaceError, CrossConeHirInternalClosureError,
    CrossConeHirNominalSurfaceError, CrossConeHirPropertySurfaceError,
    CrossConeHirTypeAliasSurfaceError,
};

#[derive(Debug)]
pub enum CrossConeClosureInternalHirError {
    Allocation {
        requested_slots: usize,
    },
    Artifact {
        identity: ConeIdentity,
        source: Box<CrossConeHirInternalClosureError>,
    },
}

impl fmt::Display for CrossConeClosureInternalHirError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Allocation { requested_slots } => write!(
                formatter,
                "cannot allocate {requested_slots} internally closed cross-Cone HIR slots"
            ),
            Self::Artifact { identity, source } => write!(
                formatter,
                "invalid internal HIR closure for {identity}: {source}"
            ),
        }
    }
}

impl std::error::Error for CrossConeClosureInternalHirError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Artifact { source, .. } => Some(source),
            Self::Allocation { .. } => None,
        }
    }
}

macro_rules! define_surface_error {
    ($name:ident, $source:ty, $label:literal) => {
        #[derive(Debug)]
        pub enum $name {
            Allocation {
                requested_slots: usize,
            },
            AuthorityAllocation {
                identity: ConeIdentity,
                requested_slots: usize,
            },
            Artifact {
                identity: ConeIdentity,
                source: Box<$source>,
            },
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                match self {
                    Self::Allocation { requested_slots } => write!(
                        formatter,
                        "cannot allocate {requested_slots} {}-validated cross-Cone HIR slots",
                        $label
                    ),
                    Self::AuthorityAllocation {
                        identity,
                        requested_slots,
                    } => write!(
                        formatter,
                        "cannot allocate {requested_slots} {} dependency authority slots for {identity}",
                        $label
                    ),
                    Self::Artifact { identity, source } => write!(
                        formatter,
                        "invalid {} HIR surface for {identity}: {source}",
                        $label
                    ),
                }
            }
        }

        impl std::error::Error for $name {
            fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
                match self {
                    Self::Artifact { source, .. } => Some(source),
                    Self::Allocation { .. } | Self::AuthorityAllocation { .. } => None,
                }
            }
        }
    };
}

define_surface_error!(
    CrossConeClosureNominalSurfaceError,
    CrossConeHirNominalSurfaceError,
    "nominal"
);
define_surface_error!(
    CrossConeClosurePropertySurfaceError,
    CrossConeHirPropertySurfaceError,
    "property"
);
define_surface_error!(
    CrossConeClosureCallableSurfaceError,
    CrossConeHirCallableSurfaceError,
    "callable"
);
define_surface_error!(
    CrossConeClosureTypeAliasSurfaceError,
    CrossConeHirTypeAliasSurfaceError,
    "type-alias"
);
