use std::path::PathBuf;

use scoop_linker::ResolvedLinkPlanFingerprint;

use crate::{BuildGraphOutcome, BuildProfile, SourceSnapshot};

#[derive(Debug)]
pub struct BuiltArtifacts {
    pub output: PathBuf,
    pub profile: BuildProfile,
    pub graph: BuildGraphOutcome,
    pub sources: Vec<SourceSnapshot>,
}

#[derive(Debug)]
pub enum BuildOutcome {
    Library(BuiltArtifacts),
    Executable {
        build: BuiltArtifacts,
        runtime_index: PathBuf,
        link_plan_fingerprint: ResolvedLinkPlanFingerprint,
        execution_copy: Option<tempfile::TempPath>,
    },
}

impl BuildOutcome {
    pub fn artifacts(&self) -> &BuiltArtifacts {
        match self {
            Self::Library(build) | Self::Executable { build, .. } => build,
        }
    }
}
