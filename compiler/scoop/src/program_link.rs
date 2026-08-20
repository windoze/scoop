//! Link the immutable artifacts retained by dependency-graph execution.
use scoop_linker::LinkError;
use scoop_toolchain::ResolvedTargetProfile;

use crate::ValidatedArtifactClosure;

pub fn read_built_program(
    artifacts: &ValidatedArtifactClosure,
    target: &ResolvedTargetProfile,
) -> Result<scoop_slib::ProgramLinkClosure, LinkError> {
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
        target.lir_target_selection(),
        target.c_bridge_toolchain().profile(),
    )
    .map_err(|error| LinkError(format!("built program Link input: {error}")))?;
    Ok(closure)
}
