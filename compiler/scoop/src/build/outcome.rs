use std::path::PathBuf;

use scoop_linker::ResolvedLinkPlanFingerprint;

use crate::{BuildGraphOutcome, BuildProfile, SelectedSource};

#[derive(Debug)]
pub struct BuiltArtifacts {
    pub invocation: Option<crate::options::BuildInvocation>,
    pub output: PathBuf,
    pub profile: BuildProfile,
    pub graph: BuildGraphOutcome,
    pub sources: Vec<SelectedSource>,
    pub profiling: Option<ProfilingBuildOutput>,
    pub profile_use: Option<ProfileUseBuildOutput>,
}

#[derive(Debug, Default)]
pub struct ProfilingBuildOutput {
    pub maps: Vec<PathBuf>,
    pub missing_maps: Vec<PathBuf>,
    pub uninstrumented_cones: Vec<String>,
}

#[derive(Debug, Default, serde::Serialize)]
pub struct ProfileUseBuildOutput {
    pub source_cones: Vec<ProfileUseCone>,
    pub precompiled_cones: Vec<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct ProfileUseCone {
    pub cone: String,
    pub summary: scoop_profile::UseSummary,
}

impl ProfileUseBuildOutput {
    pub fn used(&self) -> bool {
        self.source_cones.iter().any(|cone| cone.summary.used())
    }
}

#[derive(Debug)]
pub enum BuildOutcome {
    Artifact(BuiltArtifacts),
    Library(BuiltArtifacts),
    NativeApp {
        build: BuiltArtifacts,
        library: PathBuf,
        runtime_index: PathBuf,
        link_plan_fingerprint: ResolvedLinkPlanFingerprint,
    },
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
            Self::Artifact(build)
            | Self::Library(build)
            | Self::Executable { build, .. }
            | Self::NativeApp { build, .. } => build,
        }
    }
}
