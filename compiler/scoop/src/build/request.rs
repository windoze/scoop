use std::path::PathBuf;

use crate::{BuildDumpRequest, BuildGraphRequest};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, clap::ValueEnum)]
pub enum BuildProfile {
    #[default]
    Debug,
    Release,
}

impl BuildProfile {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Debug => "debug",
            Self::Release => "release",
        }
    }
}

#[derive(Debug)]
pub enum RuntimeInput {
    SourceRoot(PathBuf),
    ObjectIndex(PathBuf),
}

/// Final user outputs and observations are separate from compilation inputs.
pub struct BuildRequest {
    pub graph: BuildGraphRequest,
    pub profile: BuildProfile,
    pub cwd: PathBuf,
    pub output: Option<PathBuf>,
    pub target_dir: Option<PathBuf>,
    pub runtime: RuntimeInput,
    pub final_link: scoop_toolchain::FinalLinkOptions,
    pub library_paths: Vec<PathBuf>,
    pub dumps: Option<BuildDumpRequest>,
    pub keep_for_run: bool,
}
