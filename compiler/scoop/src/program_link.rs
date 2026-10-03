//! Link the immutable artifacts retained by dependency-graph execution.
use std::path::{Path, PathBuf};

use scoop_linker::{LinkError, ProgramLinkOutput, RuntimeObjectSet, link_program};
use scoop_lir::ValidatedLirTargetSelection;
use scoop_toolchain::ValidatedFinalLinkProfile;

use crate::ValidatedArtifactClosure;

pub fn link_built_program(
    artifacts: &ValidatedArtifactClosure,
    runtime: &RuntimeObjectSet,
    profile: &ValidatedFinalLinkProfile,
    library_paths: &[PathBuf],
    output: &Path,
) -> Result<ProgramLinkOutput, LinkError> {
    let root = artifacts.artifact(artifacts.root()).ok_or_else(|| {
        LinkError(format!(
            "build closure is missing root {}",
            artifacts.root()
        ))
    })?;
    let dependencies: Vec<_> = artifacts
        .artifacts()
        .filter(|(identity, _)| *identity != artifacts.root())
        .map(|(_, artifact)| artifact.snapshot().as_bytes())
        .collect();
    // Graph execution checked manifest edges, not Link machine data. Read that
    // data once from the retained snapshots, without reopening source paths.
    let closure = scoop_slib::read_program_link_closure(
        root.snapshot().as_bytes(),
        &dependencies,
        ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1,
        profile.startup_toolchain().profile(),
    )
    .map_err(|error| LinkError(format!("built program Link input: {error}")))?;
    link_program(&closure, runtime, profile, library_paths, output)
}
