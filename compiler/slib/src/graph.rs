use std::fmt;

use scoop_identity::{ConeCoordinate, ConeIdentity};
use scoop_lir::ValidatedLirTargetSelection;
use scoop_wire::DecodeUsage;

use crate::{
    ArtifactFingerprint, CompatibilityRecord, ConeKind, ConeSourceForm, DecodedSlibEnvelope,
    DependencyRecord,
};

/// A single artifact whose immutable envelope is valid as a graph node.
///
/// This proof does not assert that dependency artifacts exist, form a closed
/// acyclic graph, or satisfy any Compile or Link capability.
#[derive(Debug, Eq, PartialEq)]
pub struct ValidatedGraphArtifact<'input> {
    pub(crate) envelope: DecodedSlibEnvelope<'input>,
}

impl<'input> DecodedSlibEnvelope<'input> {
    pub fn validate_graph(self) -> Result<ValidatedGraphArtifact<'input>, GraphValidationError> {
        let cone = self.manifest().cone().identity();
        if self
            .manifest()
            .direct_dependencies()
            .iter()
            .any(|dependency| dependency.identity() == cone)
        {
            return Err(GraphValidationError::SelfDependency { cone });
        }
        if self
            .manifest()
            .direct_dependencies()
            .iter()
            .any(|dependency| dependency.identity() == ConeIdentity::SINGLE_FILE)
        {
            return Err(GraphValidationError::SingleFileDependency);
        }
        Ok(ValidatedGraphArtifact { envelope: self })
    }
}

impl ValidatedGraphArtifact<'_> {
    pub const fn coordinate(&self) -> &ConeCoordinate {
        self.envelope.manifest().cone().coordinate()
    }

    pub const fn identity(&self) -> ConeIdentity {
        self.envelope.manifest().cone().identity()
    }

    pub const fn kind(&self) -> ConeKind {
        self.envelope.manifest().cone().kind()
    }

    pub const fn source_form(&self) -> ConeSourceForm {
        self.envelope.manifest().cone().source_form()
    }

    pub fn direct_dependencies(&self) -> &[DependencyRecord] {
        self.envelope.manifest().direct_dependencies()
    }

    pub const fn compatibility(&self) -> &CompatibilityRecord {
        self.envelope.manifest().compatibility()
    }

    pub const fn target_selection(&self) -> ValidatedLirTargetSelection {
        self.envelope.target_selection()
    }

    pub const fn artifact_fingerprint(&self) -> ArtifactFingerprint {
        self.envelope.manifest().artifact_fingerprint()
    }

    pub const fn decode_usage(&self) -> DecodeUsage {
        self.envelope.decode_usage()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GraphValidationError {
    SelfDependency { cone: ConeIdentity },
    SingleFileDependency,
}

impl fmt::Display for GraphValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SelfDependency { cone } => write!(formatter, "Cone {cone} depends on itself"),
            Self::SingleFileDependency => {
                formatter.write_str("the reserved single-file Cone cannot be a dependency")
            }
        }
    }
}

impl std::error::Error for GraphValidationError {}

#[cfg(test)]
mod tests;
