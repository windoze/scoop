//! Produce one library or linked executable through the existing graph pipeline.
use std::path::{Path, PathBuf};

use scoop_identity::{ConeIdentity, RequestedConeKind};
use scoop_linker::RuntimeObjectSet;
use scoop_toolchain::ResolvedTargetProfile;

use crate::materialize::OutputLocation;
use crate::{
    BuildFailurePhase, PreparedBuildGraph, RuntimeBuildRequest, SourceSnapshot, build_runtime,
    link_built_program,
};

mod error;
mod outcome;
mod request;
pub use error::*;
pub use outcome::*;
pub use request::*;

pub fn build(request: BuildRequest) -> BuildResult<BuildOutcome> {
    let target = request.graph.target().clone();
    let cache = request.graph.cache_root().as_path().to_owned();
    let stem = request
        .graph
        .root()
        .single_file_source()
        .map(|source| sanitized_stem(source.display_path()));
    let graph = request
        .graph
        .load_root()
        .map_err(BuildFailure::root)?
        .discover()
        .map_err(BuildFailure::discovery)?
        .resolve()
        .map_err(BuildFailure::classified)?;
    if request.keep_for_run && graph.root_kind() == RequestedConeKind::Library {
        return Err(BuildFailure::tool(
            "SCOOP_RUN_LIBRARY",
            BuildFailurePhase::Request,
            "run requires an executable Cone; the root is a library",
        ));
    }
    let mut prepared = graph.prepare().map_err(BuildFailure::preparation)?;
    let cache = std::fs::canonicalize(cache).map_err(output_error)?;
    let (sources, mut inputs) = source_context(&prepared);
    let output = output_path(
        &prepared,
        &request.cwd,
        request.output,
        request.target_dir,
        stem.as_deref(),
        &target,
        request.profile,
    )?;
    let output = OutputLocation::new(&output).map_err(output_error)?;
    if let Some(dumps) = request.dumps {
        prepared.observe_dumps(dumps).map_err(output_error)?;
    }
    let graph = prepared
        .execute()
        .map_err(|error| BuildFailure::execution(error, &sources))?
        .into_outcome();
    inputs.extend(
        graph
            .artifacts()
            .iter()
            .map(|node| node.artifact_locator().to_owned()),
    );
    let mut finish = || -> BuildResult<_> {
        if matches!(graph, crate::BuildGraphOutcome::Library { .. }) {
            output
                .publish_bytes(
                    graph.root().artifact().snapshot().as_bytes(),
                    &inputs,
                    Some(&cache),
                )
                .map_err(output_error)?;
            return Ok(None);
        }
        let (runtime, index, runtime_inputs) = runtime_input(&request.runtime, &target, &cache)?;
        inputs.extend(runtime_inputs);
        inputs.extend(runtime.input_paths().iter().cloned());
        let directory = tempfile::Builder::new()
            .prefix(".scoop-build-")
            .tempdir_in(output.parent())
            .map_err(output_error)?;
        let final_link = target.final_link().map_err(|error| {
            BuildFailure::tool("SCOOP_LINK_TOOLCHAIN", BuildFailurePhase::FinalLink, error)
        })?;
        let linked = link_built_program(
            graph.closure(),
            &runtime,
            &final_link,
            &request.library_paths,
            &directory.path().join("program"),
        )
        .map_err(|error| {
            BuildFailure::tool("SCOOP_LINK_FAILED", BuildFailurePhase::FinalLink, error)
        })?;
        inputs.extend(linked.input_paths);
        let execution_copy = output
            .publish_program(&linked.path, &inputs, Some(&cache), request.keep_for_run)
            .map_err(output_error)?;
        Ok(Some((index, linked.fingerprint, execution_copy)))
    };
    let executable = finish().map_err(|error| error.with_context(graph.warnings(), &sources))?;
    let build = BuiltArtifacts {
        output: output.path().to_owned(),
        profile: request.profile,
        graph,
        sources,
    };
    Ok(match executable {
        None => BuildOutcome::Library(build),
        Some((runtime_index, link_plan_fingerprint, execution_copy)) => BuildOutcome::Executable {
            build,
            runtime_index,
            link_plan_fingerprint,
            execution_copy,
        },
    })
}

fn runtime_input(
    input: &RuntimeInput,
    target: &ResolvedTargetProfile,
    cache: &Path,
) -> BuildResult<(RuntimeObjectSet, PathBuf, Vec<PathBuf>)> {
    let failure = |error| {
        BuildFailure::tool(
            "SCOOP_RUNTIME_BUILD",
            BuildFailurePhase::RuntimeBuild,
            error,
        )
    };
    match input {
        RuntimeInput::ObjectIndex(index) => {
            let objects = RuntimeObjectSet::read_index(
                index,
                target.lir_target(),
                target.c_bridge_toolchain().profile(),
            )
            .map_err(|error| failure(error.to_string()))?;
            Ok((objects, index.clone(), Vec::new()))
        }
        RuntimeInput::SourceRoot(root) => {
            let runtime = build_runtime(RuntimeBuildRequest {
                target,
                runtime_root: root,
                cache_root: cache,
                optimization: Default::default(),
            })
            .map_err(|error| failure(error.to_string()))?;
            let index = runtime.index().to_owned();
            let inputs = runtime.input_paths().to_vec();
            Ok((runtime.into_objects(), index, inputs))
        }
    }
}

fn source_context(prepared: &PreparedBuildGraph) -> (Vec<SourceSnapshot>, Vec<PathBuf>) {
    let mut sources = Vec::new();
    let mut paths = Vec::new();
    for identity in prepared.dependency_first() {
        if let Some(manifest) = prepared.source_snapshot(*identity) {
            paths.push(manifest.manifest_locator().to_owned());
            sources.extend(manifest.sources().cloned());
        }
        if let Some(single) = prepared.single_file_snapshot(*identity) {
            sources.push(single.source().clone());
        }
        if let Some(candidates) = prepared.prebuilt_candidates(*identity) {
            paths.extend(candidates.map(|candidate| candidate.source_locator().to_owned()));
        }
    }
    paths.extend(
        sources
            .iter()
            .map(|source| source.display_locator().as_path().to_owned()),
    );
    (sources, paths)
}

fn output_path(
    graph: &PreparedBuildGraph,
    cwd: &Path,
    output: Option<PathBuf>,
    target_dir: Option<PathBuf>,
    stem: Option<&str>,
    target: &ResolvedTargetProfile,
    profile: BuildProfile,
) -> BuildResult<PathBuf> {
    if let Some(output) = output {
        return Ok(output);
    }
    let (root, name) = if graph.root_identity() == ConeIdentity::SINGLE_FILE {
        (cwd.to_owned(), stem.unwrap_or("program").to_owned())
    } else {
        let manifest = graph
            .source_snapshot(graph.root_identity())
            .ok_or_else(|| {
                BuildFailure::tool(
                    "SCOOP_OUTPUT_LAYOUT",
                    BuildFailurePhase::Publication,
                    "manifest root has no captured manifest",
                )
            })?;
        let root = manifest
            .manifest_locator()
            .parent()
            .ok_or_else(|| {
                BuildFailure::tool(
                    "SCOOP_OUTPUT_LAYOUT",
                    BuildFailurePhase::Publication,
                    "manifest has no parent directory",
                )
            })?
            .to_owned();
        let mut name = manifest.coordinate().name().to_owned();
        if graph.root_kind() == RequestedConeKind::Library {
            name.push_str(".slib");
        }
        (root, name)
    };
    Ok(target_dir
        .unwrap_or_else(|| root.join("target"))
        .join(target.canonical_triple())
        .join(profile.as_str())
        .join(name))
}

fn sanitized_stem(path: &Path) -> String {
    let bytes = path.file_stem().unwrap_or_default().as_encoded_bytes();
    let stem: String = bytes
        .iter()
        .map(|byte| {
            if byte.is_ascii_alphanumeric() || b"._-".contains(byte) {
                *byte as char
            } else {
                '_'
            }
        })
        .collect();
    if matches!(stem.as_str(), "" | "." | "..") {
        "program".to_owned()
    } else {
        stem
    }
}

fn output_error(error: impl std::fmt::Display) -> Box<BuildFailure> {
    BuildFailure::tool("SCOOP_OUTPUT_WRITE", BuildFailurePhase::Publication, error)
}
