//! Artifact summaries shared by the production writer and consumer.

use crate::{
    ArtifactDistributionClassV1, SemanticFingerprintRecord, SingleConeProductionOutputV1,
    SlibMemberId,
};

mod cross_cone;
pub use cross_cone::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompileViewSummaryV1 {
    semantic_fingerprints: SemanticFingerprintRecord,
}

impl CompileViewSummaryV1 {
    pub(crate) const fn new(semantic_fingerprints: SemanticFingerprintRecord) -> Self {
        Self {
            semantic_fingerprints,
        }
    }

    pub const fn semantic_fingerprints(self) -> SemanticFingerprintRecord {
        self.semantic_fingerprints
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LinkViewSummaryV1 {
    distribution: ArtifactDistributionClassV1,
    output: SingleConeProductionOutputV1,
    image_owner_member: SlibMemberId,
    link_object_count: usize,
    semantic_fingerprints: SemanticFingerprintRecord,
}

impl LinkViewSummaryV1 {
    pub const fn distribution(&self) -> ArtifactDistributionClassV1 {
        self.distribution
    }

    pub const fn output(&self) -> &SingleConeProductionOutputV1 {
        &self.output
    }

    pub const fn image_owner_member(&self) -> SlibMemberId {
        self.image_owner_member
    }

    pub const fn link_object_count(&self) -> usize {
        self.link_object_count
    }

    pub const fn semantic_fingerprints(&self) -> SemanticFingerprintRecord {
        self.semantic_fingerprints
    }
}
