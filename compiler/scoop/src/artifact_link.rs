//! Resolve an exact artifact closure without source discovery or compiler probes.
use std::path::PathBuf;
use std::sync::Arc;

use scoop_linker::{ResolvedLinkPlanFingerprint, RuntimeObjectSet, link_program};
use scoop_slib::{ArtifactManifestSummaryV1, ArtifactSnapshot};
use scoop_toolchain::ValidatedFinalLinkProfile;

use crate::materialize::OutputLocation;
use crate::{BuildFailure, BuildFailurePhase, BuildResult};

mod resolve;

pub struct LinkRequest {
    pub root_slib: PathBuf,
    pub dependency_slibs: Vec<PathBuf>,
    pub cone_paths: Vec<PathBuf>,
    pub sysroot: PathBuf,
    pub target: String,
    pub c_toolchain: scoop_toolchain::CToolchainOptions,
    pub final_link: scoop_toolchain::FinalLinkOptions,
    pub runtime_index: PathBuf,
    pub library_paths: Vec<PathBuf>,
    pub output: PathBuf,
}

#[derive(Debug)]
pub struct LocatedLinkArtifact {
    pub path: PathBuf,
    pub snapshot: Arc<ArtifactSnapshot>,
    pub summary: ArtifactManifestSummaryV1,
}

#[derive(Debug)]
pub struct LinkOutcome {
    pub output: PathBuf,
    pub root: LocatedLinkArtifact,
    pub dependencies: Vec<LocatedLinkArtifact>,
    pub runtime_index: PathBuf,
    pub link_plan_fingerprint: ResolvedLinkPlanFingerprint,
    pub link_plan: String,
}

pub fn link_artifacts(request: LinkRequest) -> BuildResult<LinkOutcome> {
    let profile = ValidatedFinalLinkProfile::resolve_with(
        &request.target,
        &request.c_toolchain,
        &request.final_link,
    )
    .map_err(failure)?;
    let selection = scoop_lir::ValidatedLirTargetSelection::from_id(profile.id());
    let (root, dependencies) = resolve::artifacts(&request, selection)?;
    let bytes = dependencies
        .iter()
        .map(|artifact| artifact.snapshot.as_bytes())
        .collect::<Vec<_>>();
    let closure = scoop_slib::read_program_link_closure(
        root.snapshot.as_bytes(),
        &bytes,
        selection,
        profile.startup_toolchain().profile(),
    )
    .map_err(|error| read_failure(error, &root, &dependencies))?;
    let runtime = RuntimeObjectSet::read_index(
        &request.runtime_index,
        profile.target(),
        profile.startup_toolchain().profile(),
    )
    .map_err(failure)?;
    let output = OutputLocation::new(&request.output).map_err(failure)?;
    let directory = tempfile::Builder::new()
        .prefix(".scoop-link-output-")
        .tempdir_in(output.parent())
        .map_err(failure)?;
    let linked = link_program(
        &closure,
        &runtime,
        &profile,
        &request.library_paths,
        &directory.path().join("program"),
    )
    .map_err(failure)?;
    let mut inputs = vec![root.path.clone()];
    inputs.extend(dependencies.iter().map(|artifact| artifact.path.clone()));
    inputs.extend(runtime.input_paths().iter().cloned());
    inputs.extend(linked.input_paths);
    output
        .publish_program(&linked.path, &inputs, None, false)
        .map_err(failure)?;
    Ok(LinkOutcome {
        output: output.path().to_owned(),
        root,
        dependencies,
        runtime_index: request.runtime_index,
        link_plan_fingerprint: linked.fingerprint,
        link_plan: linked.plan_dump,
    })
}

fn failure(error: impl std::fmt::Display) -> Box<BuildFailure> {
    BuildFailure::tool("SCOOP_LINK_FAILED", BuildFailurePhase::FinalLink, error)
}

fn read_failure(
    error: scoop_slib::ProgramLinkReadError,
    root: &LocatedLinkArtifact,
    dependencies: &[LocatedLinkArtifact],
) -> Box<BuildFailure> {
    let Some(identity) = error.artifact() else {
        return failure(error);
    };
    let Some(artifact) = std::iter::once(root)
        .chain(dependencies)
        .find(|artifact| artifact.summary.cone().identity() == identity)
    else {
        return BuildFailure::tool(
            "SCOOP_INTERNAL_ERROR",
            BuildFailurePhase::FinalLink,
            format!(
                "Link diagnostic references an artifact outside its inputs: {identity}: {error}"
            ),
        );
    };
    failure(&error).at_artifact(&artifact.path, error.semantic_path())
}
