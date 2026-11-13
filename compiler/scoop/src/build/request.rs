use std::path::PathBuf;

use crate::{BuildDumpRequest, ResolvedBuildGraph};

#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Eq,
    PartialEq,
    clap::ValueEnum,
    serde::Serialize,
    serde::Deserialize,
)]
#[serde(rename_all = "kebab-case")]
pub enum BuildProfile {
    #[default]
    Debug,
    Release,
    Profiling,
}

impl BuildProfile {
    pub const fn optimization(self) -> scoop_lir::OptimizationMode {
        match self {
            Self::Debug => scoop_lir::OptimizationMode::Debug,
            Self::Release | Self::Profiling => scoop_lir::OptimizationMode::Release,
        }
    }

    pub const fn instrumentation(self) -> scoop_profile::Instrumentation {
        match self {
            Self::Profiling => scoop_profile::Instrumentation::Generate,
            Self::Debug | Self::Release => scoop_profile::Instrumentation::Off,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Debug => "debug",
            Self::Release => "release",
            Self::Profiling => "profiling",
        }
    }
}

#[derive(Clone, Debug)]
pub enum RuntimeInput {
    SourceRoot(PathBuf),
    ObjectIndex(PathBuf),
}

/// Final user outputs and observations are separate from compilation inputs.
pub struct BuildRequest {
    pub invocation: Option<crate::options::BuildInvocation>,
    pub graph: ResolvedBuildGraph,
    pub profile: BuildProfile,
    pub cwd: PathBuf,
    pub output: Option<PathBuf>,
    pub emit_slib: bool,
    pub target_dir: Option<PathBuf>,
    pub runtime: RuntimeInput,
    pub enable_gc_stress: bool,
    pub enable_startup_stats: bool,
    pub final_link: scoop_toolchain::FinalLinkOptions,
    pub library_paths: Vec<PathBuf>,
    pub dumps: Option<BuildDumpRequest>,
    pub require_executable: bool,
    pub profile_use_required: bool,
}
