//! Read the built artifacts when final linking needs their objects.
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
        .map(|(_, artifact)| artifact.snapshot().map(|snapshot| snapshot.as_bytes()))
        .collect::<Result<_, _>>()
        .map_err(|error| LinkError(error.to_string()))?;
    // This is the first byte read for source-built results; graph completion
    // only retained their paths and producer summaries.
    let closure = scoop_slib::read_program_link_closure(
        root.snapshot()
            .map_err(|error| LinkError(error.to_string()))?
            .as_bytes(),
        &dependencies,
        target.lir_target_selection(),
        target.c_bridge_toolchain().profile(),
    )
    .map_err(|error| LinkError(format!("built program Link input: {error}")))?;
    Ok(closure)
}
