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
        .map(|(_, artifact)| artifact.file().map(scoop_slib::ArtifactInput::File))
        .collect::<Result<_, _>>()
        .map_err(|error| LinkError(error.to_string()))?;
    let closure = scoop_slib::read_program_link_closure(
        scoop_slib::ArtifactInput::File(root.file().map_err(|error| LinkError(error.to_string()))?),
        &dependencies,
        target.lir_target_selection(),
        target.c_bridge_toolchain().profile(),
    )
    .map_err(|error| LinkError(format!("built program Link input: {error}")))?;
    Ok(closure)
}
