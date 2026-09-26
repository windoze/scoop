//! Errors for closure-wide Compile, Link, and terminal-definition validation.

use std::fmt;

use scoop_identity::{ConeIdentity, ObjectDefinitionPlanId, PersistentSymbolRequest};
use scoop_wire::HashError;

use crate::{
    CrossConeHirFrontSectionDecodeError, CrossConePublishViewMismatchError,
    CrossConeSemanticClosureValidationError, GraphValidationError, LinkDefinitionOwnerV1,
    SlibReadError, StrongLinkArtifactValidationError,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CrossConeClosureArtifactSlotV1 {
    Dependency(usize),
    Current,
}

impl fmt::Display for CrossConeClosureArtifactSlotV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Dependency(index) => write!(formatter, "dependency artifact {index}"),
            Self::Current => formatter.write_str("current artifact"),
        }
    }
}

#[derive(Debug)]
pub enum CrossConeArtifactClosureValidationError {
    CompileEnvelope {
        slot: CrossConeClosureArtifactSlotV1,
        source: Box<SlibReadError>,
    },
    CompileGraph {
        slot: CrossConeClosureArtifactSlotV1,
        source: Box<GraphValidationError>,
    },
    CompileSections {
        slot: CrossConeClosureArtifactSlotV1,
        source: Box<CrossConeHirFrontSectionDecodeError>,
    },
    Semantic(Box<CrossConeSemanticClosureValidationError>),
    Link {
        slot: CrossConeClosureArtifactSlotV1,
        source: Box<StrongLinkArtifactValidationError>,
    },
    ViewMismatch {
        slot: CrossConeClosureArtifactSlotV1,
        source: Box<CrossConePublishViewMismatchError>,
    },
    Definition(Box<CrossConeDefinitionResolutionError>),
    MissingCompletedCurrentArtifact,
}

impl From<CrossConeDefinitionResolutionError> for CrossConeArtifactClosureValidationError {
    fn from(source: CrossConeDefinitionResolutionError) -> Self {
        Self::Definition(Box::new(source))
    }
}

impl fmt::Display for CrossConeArtifactClosureValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CompileEnvelope { slot, source } => {
                write!(formatter, "cannot open Compile view for {slot}: {source}")
            }
            Self::CompileGraph { slot, source } => {
                write!(
                    formatter,
                    "cannot validate Compile graph for {slot}: {source}"
                )
            }
            Self::CompileSections { slot, source } => {
                write!(
                    formatter,
                    "cannot decode Compile sections for {slot}: {source}"
                )
            }
            Self::Semantic(source) => source.fmt(formatter),

            Self::Link { slot, source } => {
                write!(formatter, "cannot validate Link view for {slot}: {source}")
            }
            Self::ViewMismatch { slot, source } => {
                write!(
                    formatter,
                    "cannot reconcile Compile/Link views for {slot}: {source}"
                )
            }
            Self::Definition(source) => source.fmt(formatter),
            Self::MissingCompletedCurrentArtifact => formatter.write_str(
                "completed cross-Cone closure validation did not retain the current artifact",
            ),
        }
    }
}

impl std::error::Error for CrossConeArtifactClosureValidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::CompileEnvelope { source, .. } => Some(source.as_ref()),
            Self::CompileGraph { source, .. } => Some(source.as_ref()),
            Self::CompileSections { source, .. } => Some(source.as_ref()),
            Self::Semantic(source) => Some(source.as_ref()),
            Self::Link { source, .. } => Some(source.as_ref()),
            Self::ViewMismatch { source, .. } => Some(source.as_ref()),
            Self::Definition(source) => Some(source.as_ref()),
            Self::MissingCompletedCurrentArtifact => None,
        }
    }
}

#[derive(Debug)]
pub enum CrossConeDefinitionResolutionError {
    MissingProvider {
        consumer: ConeIdentity,
        provider: ConeIdentity,
    },
    Identity(HashError),
    InvalidDefinitionOwner {
        consumer: ConeIdentity,
        provider: ConeIdentity,
        definition: ObjectDefinitionPlanId,
    },
    DefinitionIdentityMismatch(Box<CrossConeDefinitionIdentityMismatch>),
    MissingStrongDefinition(Box<CrossConeMissingStrongDefinition>),
    StrongDefinitionOwnerMismatch(Box<CrossConeStrongDefinitionOwnerMismatch>),
}

#[derive(Debug)]
pub struct CrossConeDefinitionIdentityMismatch {
    pub consumer: ConeIdentity,
    pub provider: ConeIdentity,
    pub expected: ObjectDefinitionPlanId,
    pub actual: ObjectDefinitionPlanId,
}

#[derive(Debug)]
pub struct CrossConeMissingStrongDefinition {
    pub consumer: ConeIdentity,
    pub provider: ConeIdentity,
    pub definition: ObjectDefinitionPlanId,
    pub symbol: PersistentSymbolRequest,
}

#[derive(Debug)]
pub struct CrossConeStrongDefinitionOwnerMismatch {
    pub consumer: ConeIdentity,
    pub provider: ConeIdentity,
    pub definition: ObjectDefinitionPlanId,
    pub symbol: PersistentSymbolRequest,
    pub actual: LinkDefinitionOwnerV1,
}

impl fmt::Display for CrossConeDefinitionResolutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "cross-Cone Link import does not resolve to its terminal Strong definition: {self:?}"
        )
    }
}

impl std::error::Error for CrossConeDefinitionResolutionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Identity(source) => Some(source),
            Self::MissingProvider { .. }
            | Self::InvalidDefinitionOwner { .. }
            | Self::DefinitionIdentityMismatch(_)
            | Self::MissingStrongDefinition(_)
            | Self::StrongDefinitionOwnerMismatch(_) => None,
        }
    }
}
