use std::path::{Path, PathBuf};

use scoop::{ArtifactCacheRoot, BuildProfile, BuildResult, BuildRootInput};
use scoop_manifest::SingleFileLocator;

use super::commands::{absolute, config};

pub(super) fn single_file_root(cwd: &Path, input: PathBuf) -> BuildResult<BuildRootInput> {
    let input = absolute(cwd, input)?;
    SingleFileLocator::from_path(input)
        .map(BuildRootInput::single_file)
        .map_err(|error| config(&error).at_host(error.path()))
}

pub(super) fn build_cache(
    cwd: &Path,
    target_root: &Path,
    canonical_triple: &str,
    profile: BuildProfile,
    explicit: Option<PathBuf>,
) -> BuildResult<ArtifactCacheRoot> {
    let path = explicit
        .or_else(|| std::env::var_os("SCOOP_CACHE_DIR").map(PathBuf::from))
        .unwrap_or_else(|| {
            target_root
                .join(canonical_triple)
                .join("build")
                .join(profile.as_str())
        });
    ArtifactCacheRoot::new(absolute(cwd, path)?).map_err(config)
}
