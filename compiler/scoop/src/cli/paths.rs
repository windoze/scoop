use std::path::{Path, PathBuf};

use scoop::{ArtifactCacheRoot, BuildProfile, BuildResult, BuildRootInput};
use scoop_manifest::{CurrentConeInput, classify_current_cone_operand};

use super::commands::{absolute, config};

pub(super) fn build_root(cwd: &Path, input: Option<PathBuf>) -> BuildResult<BuildRootInput> {
    let input = absolute(cwd, input.unwrap_or_else(|| PathBuf::from("Cone.toml")))?;
    match classify_current_cone_operand(input)
        .map_err(|error| config(&error).at_host(error.path()))?
    {
        CurrentConeInput::Manifest { root } => BuildRootInput::manifest(root).map_err(config),
        CurrentConeInput::SingleFile { source } => Ok(BuildRootInput::single_file(source)),
    }
}

pub(super) fn target_root(
    cwd: &Path,
    root: &BuildRootInput,
    explicit: Option<PathBuf>,
) -> BuildResult<PathBuf> {
    if let Some(path) = explicit {
        return absolute(cwd, path);
    }
    match root.manifest_root() {
        Some(root) => {
            let manifest = root
                .resolve()
                .map_err(|error| config(&error).at_host(error.path()))?;
            let directory = manifest
                .manifest_path()
                .parent()
                .ok_or_else(|| config("manifest has no parent directory"))?;
            Ok(directory.join("target"))
        }
        None => Ok(cwd.join("target")),
    }
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
