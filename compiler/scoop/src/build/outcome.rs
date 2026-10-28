use std::path::PathBuf;

use scoop_linker::ResolvedLinkPlanFingerprint;

use crate::{BuildGraphOutcome, BuildProfile, SelectedSource};

#[derive(Debug)]
pub struct BuiltArtifacts {
    pub output: PathBuf,
    pub profile: BuildProfile,
    pub graph: BuildGraphOutcome,
    pub sources: Vec<SelectedSource>,
}

#[derive(Debug)]
pub enum BuildOutcome {
    Artifact(BuiltArtifacts),
    Library(BuiltArtifacts),
    Executable {
        build: BuiltArtifacts,
        executable: PathBuf,
        runtime_index: PathBuf,
        link_plan_fingerprint: ResolvedLinkPlanFingerprint,
    },
}

impl BuildOutcome {
    pub fn artifacts(&self) -> &BuiltArtifacts {
        match self {
            Self::Artifact(build) | Self::Library(build) | Self::Executable { build, .. } => build,
        }
    }
}
