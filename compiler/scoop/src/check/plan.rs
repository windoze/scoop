use std::collections::BTreeMap;

use super::*;
use crate::discovery::BuildContext;
use crate::{
    DiagnosticsPolicy, GraphTarget, LoadedBuildRoot, ResolvedBuildGraph, SourceCacheInputV1,
};

pub(super) struct Plan {
    pub graph: ResolvedBuildGraph,
    pub root: ConeCoordinate,
    pub key: String,
    pub sources: Vec<SelectedSource>,
}

impl Plan {
    pub fn new(request: &CheckRequest) -> BuildResult<Self> {
        if request.purpose == CompilationPurpose::Test && request.root.manifest_root().is_none() {
            return Err(error("--tests requires a manifest Cone"));
        }
        let selection =
            scoop_toolchain::select_target(request.target.canonical_triple()).map_err(error)?;
        let graph = LoadedBuildRoot::from_input(
            request.root.clone(),
            BuildContext {
                artifact_search_roots: request.search.clone(),
                cache_root: request.cache.clone(),
                sysroot: request.sysroot.clone(),
                target: GraphTarget::Frontend {
                    selection,
                    request: request.target.clone(),
                },
                compiler: request.compiler.clone(),
                diagnostics: DiagnosticsPolicy::Structured,
                optimization: request.profile.optimization(),
                purpose: request.purpose,
                jobs: request.jobs,
                generators: request.generators.clone(),
            },
        )
        .map_err(BuildFailure::root)?
        .discover()
        .map_err(BuildFailure::discovery)?
        .resolve()
        .map_err(BuildFailure::classified)?;
        let root = graph
            .node_coordinate(graph.root_identity())
            .expect("resolved root exists")
            .clone();
        let mut keys = BTreeMap::new();
        let mut sources = Vec::new();
        let mut machine_dependencies = false;
        for id in graph.dependency_first() {
            let purpose = if *id == graph.root_identity() {
                request.purpose
            } else {
                CompilationPurpose::Production
            };
            let dependencies = graph
                .edges()
                .filter(|edge| edge.dependent() == *id)
                .map(|edge| (edge.dependency().to_string(), &keys[&edge.dependency()]))
                .collect::<Vec<_>>();
            let key = if let Some(manifest) = graph.source_manifest(*id) {
                let selected = match purpose {
                    CompilationPurpose::Production => {
                        scoop_manifest::select_manifest_sources(manifest, selection.target().id())
                    }
                    CompilationPurpose::Test => scoop_manifest::select_manifest_sources_for_tests(
                        manifest,
                        selection.target().id(),
                    ),
                }
                .map_err(|cause| io_error(cause.path(), &cause))?;
                let selected = selected.iter().cloned().collect::<Vec<_>>();
                let semantic = manifest.parsed().semantic();
                let is_dependency = *id != graph.root_identity();
                machine_dependencies |= is_dependency;
                let key = input_key(&(
                    manifest.manifest_path().as_os_str().as_encoded_bytes(),
                    semantic.coordinate().to_string(),
                    semantic.requested_kind() == scoop_identity::RequestedConeKind::Library,
                    semantic.sources(),
                    (purpose == CompilationPurpose::Test).then(|| manifest.parsed().tests()),
                    is_dependency.then(|| semantic.native()),
                    source_inputs(&selected)?,
                    dependencies,
                ))?;
                sources.extend(selected);
                key
            } else if let Some(fingerprint) = graph.prebuilt_fingerprint(*id) {
                input_key(&(fingerprint.to_string(), dependencies))?
            } else {
                let source = scoop_manifest::select_single_file_source(
                    graph.single_file_source().expect("single-file graph node"),
                )
                .map_err(error)?;
                let key =
                    input_key(&(source_inputs(std::slice::from_ref(&source))?, dependencies))?;
                sources.push(source);
                key
            };
            keys.insert(*id, key);
        }
        let compiler = scoop_toolchain::NativeInputMetadata::read(request.compiler.as_path())
            .map_err(error)?;
        let native = machine_dependencies
            .then(|| scoop_toolchain::native_tool_configuration(&request.target))
            .transpose()
            .map_err(error)?;
        let capability = scoop_toolchain::paired_compiler_machine_capability().map_err(error)?;
        let key = input_key(&(
            &keys[&graph.root_identity()],
            selection.target().contract().canonical_triple(),
            scoop_wire::encode(&request.purpose).map_err(error)?,
            compiler,
            scoop_wire::encode(&capability).map_err(error)?,
            machine_dependencies.then_some(request.profile.as_str()),
            native.map(|key| key.to_string()),
        ))?;
        Ok(Self {
            graph,
            root,
            key,
            sources,
        })
    }
}

fn source_inputs(sources: &[SelectedSource]) -> BuildResult<Vec<Vec<u8>>> {
    sources
        .iter()
        .map(|source| scoop_wire::encode(&SourceCacheInputV1::new(source)).map_err(error))
        .collect()
}
