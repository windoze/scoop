//! Resolve an exact artifact closure without source discovery or compiler probes.
use std::path::PathBuf;

use scoop_linker::{ResolvedLinkPlanFingerprint, RuntimeObjectSet, link_program};
use scoop_slib::{ArtifactFile, ArtifactManifestSummaryV1};
use scoop_toolchain::ResolvedTargetProfile;

use crate::materialize::OutputLocation;
use crate::{BuildFailure, BuildFailurePhase, BuildResult};

mod resolve;

pub struct LinkRequest {
    pub cache_dir: PathBuf,
    pub root_slib: PathBuf,
    pub dependency_slibs: Vec<PathBuf>,
    pub cone_paths: Vec<PathBuf>,
    pub sysroot: PathBuf,
    pub target: String,
    pub c_toolchain: scoop_toolchain::CToolchainOptions,
    pub final_link: scoop_toolchain::FinalLinkOptions,
    pub runtime_index: Option<PathBuf>,
    pub enable_gc_stress: bool,
    pub enable_startup_stats: bool,
    pub library_paths: Vec<PathBuf>,
    pub output: PathBuf,
}

#[derive(Debug)]
pub struct LocatedLinkArtifact {
    pub path: PathBuf,
    pub file: ArtifactFile,
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
    let target = ResolvedTargetProfile::resolve_with_cached(
        &request.target,
        &request.c_toolchain,
        &request.cache_dir,
    )
    .map_err(failure)?;
    let selection = target.lir_target_selection();
    let (root, dependencies) = resolve::artifacts(&request, selection)?;
    let inputs = dependencies
        .iter()
        .map(|artifact| &artifact.file)
        .collect::<Vec<_>>();
    let closure = scoop_slib::read_program_link_closure(&root.file, &inputs, selection)
        .map_err(|error| read_failure(error, &root, &dependencies))?;
    let profile = scoop_linker::resolve_program_link_profile(
        &closure,
        &target,
        &request.final_link,
        Some(&request.cache_dir),
    )
    .map_err(failure)?;
    let runtime_index = request.runtime_index.clone().unwrap_or_else(|| {
        scoop_toolchain::default_runtime_index(
            &request.sysroot,
            target.lir_target(),
            request.enable_gc_stress,
            request.enable_startup_stats,
        )
    });
    let runtime = RuntimeObjectSet::read_index(
        &runtime_index,
        profile.target(),
        profile.startup_toolchain().profile(),
    )
    .map_err(failure)?;
    runtime
        .require_gc_stress(request.enable_gc_stress)
        .map_err(failure)?;
    runtime
        .require_startup_stats(request.enable_startup_stats)
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
        scoop_linker::ProgramLinkOptions {
            keep_symbols: request.final_link.keep_symbols,
            emit_symbol_map: request.final_link.emit_symbol_map,
        },
        &request.library_paths,
        &directory.path().join("program"),
        &request.cache_dir,
    )
    .map_err(failure)?;
    if let Some(symbol_map) = &linked.symbol_map {
        OutputLocation::new(&scoop_linker::symbol_map_path(output.path()))
            .map_err(failure)?
            .publish(symbol_map, None)
            .map_err(failure)?;
    }
    output.publish(&linked.path, None).map_err(failure)?;
    Ok(LinkOutcome {
        output: output.path().to_owned(),
        root,
        dependencies,
        runtime_index,
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
