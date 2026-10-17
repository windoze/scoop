//! Produce one library or linked executable through the existing graph pipeline.
use std::path::{Path, PathBuf};

use scoop_identity::{ConeIdentity, RequestedConeKind};
use scoop_linker::RuntimeObjectSet;
use scoop_toolchain::ResolvedTargetProfile;

use crate::materialize::OutputLocation;
use crate::{
    BuildFailurePhase, PreparedBuildGraph, RuntimeBuildRequest, SelectedSource, build_runtime,
    read_built_program,
};

mod error;
mod executable;
mod outcome;
mod request;
pub use error::*;
pub use outcome::*;
pub use request::*;

pub fn build(request: BuildRequest) -> BuildResult<BuildOutcome> {
    let target = request.graph.target().clone();
    let vendor_root = request.graph.sysroot().as_path().join("vendor");
    let cache = request.graph.cache_root().as_path().to_owned();
    let stem = request
        .graph
        .root()
        .single_file_source()
        .map(|source| sanitized_stem(source.display_path()));
    let graph = request
        .graph
        .with_optimization(request.profile.optimization())
        .load_root()
        .map_err(BuildFailure::root)?
        .discover()
        .map_err(BuildFailure::discovery)?
        .resolve()
        .map_err(BuildFailure::classified)?;
    if request.require_executable && graph.root_kind() == RequestedConeKind::Library {
        return Err(BuildFailure::tool(
            "SCOOP_RUN_LIBRARY",
            BuildFailurePhase::Request,
            "run requires an executable Cone; the root is a library",
        ));
    }
    let mut prepared = graph.prepare().map_err(BuildFailure::preparation)?;
    let cache = std::fs::canonicalize(cache).map_err(output_error)?;
    let sources = source_context(&prepared);
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
    let finish = || -> BuildResult<_> {
        if matches!(graph, crate::BuildGraphOutcome::Library { .. }) {
            output
                .publish(
                    graph.root().artifact_locator(),
                    Some(
                        graph
                            .root()
                            .artifact()
                            .summary()
                            .artifact_fingerprint()
                            .as_array(),
                    ),
                )
                .map_err(output_error)?;
            return Ok(None);
        }
        let (runtime, index) = runtime_input(
            &request.runtime,
            &target,
            &cache,
            &request.final_link,
            &vendor_root,
        )?;
        let linked = executable::get(executable::Request {
            graph: &graph,
            runtime: &runtime,
            target: &target,
            cache: &cache,
            profile: request.profile,
            options: &request.final_link,
            libraries: &request.library_paths,
            explicit_runtime: matches!(&request.runtime, RuntimeInput::ObjectIndex(_)),
        })?;
        output
            .publish(&linked.path, Some(&linked.result_key))
            .map_err(output_error)?;
        Ok(Some((index, linked.fingerprint, linked.path)))
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
        Some((runtime_index, link_plan_fingerprint, executable)) => BuildOutcome::Executable {
            build,
            executable,
            runtime_index,
            link_plan_fingerprint,
        },
    })
}

fn runtime_input(
    input: &RuntimeInput,
    target: &ResolvedTargetProfile,
    cache: &Path,
    final_link: &scoop_toolchain::FinalLinkOptions,
    vendor_root: &Path,
) -> BuildResult<(RuntimeObjectSet, PathBuf)> {
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
            Ok((objects, index.clone()))
        }
        RuntimeInput::SourceRoot(root) => {
            let unwind = if target.id() == scoop_target_data::DARWIN_AARCH64_ID {
                final_link.unwind_prefix.clone()
            } else {
                Some(scoop_toolchain::selected_unwind_prefix(
                    target.lir_target(),
                    final_link.sysroot.as_deref(),
                    final_link.unwind_prefix.as_deref(),
                ))
            };
            let runtime = build_runtime(RuntimeBuildRequest {
                target,
                runtime_root: root,
                vendor_root,
                cache_root: cache,
                optimization: Default::default(),
                unwind_prefix: unwind.as_deref(),
            })
            .map_err(|error| failure(error.to_string()))?;
            let index = runtime.index().to_owned();
            Ok((runtime.into_objects(), index))
        }
    }
}

fn source_context(prepared: &PreparedBuildGraph) -> Vec<SelectedSource> {
    let mut sources = Vec::new();
    for identity in prepared.dependency_first() {
        if let Some(manifest) = prepared.source_input(*identity) {
            sources.extend(manifest.sources().cloned());
        }
        if let Some(single) = prepared.single_file_input(*identity) {
            sources.push(single.source().clone());
        }
    }
    sources
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
        let manifest = graph.source_input(graph.root_identity()).ok_or_else(|| {
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
    let mut directory = target_dir
        .unwrap_or_else(|| root.join("target"))
        .join(target.canonical_triple())
        .join(profile.as_str());
    if graph.compilation_purpose(graph.root_identity()) == scoop_protocol::CompilationPurpose::Test
    {
        directory.push("tests");
    }
    Ok(directory.join(name))
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
