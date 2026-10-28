//! Source documentation keeps distinct dependency bindings in separate catalogs.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use scoop_identity::ConeIdentity;
use scoop_protocol::{StructuredDiagnosticV1, TargetSelectionRequestV1};

use crate::{
    ArtifactCacheRoot, BuildFailure, BuildFailurePhase, BuildResult, PairedScoopcLocator,
    ResolvedBuildGraph, SelectedSource,
};

mod cache;
mod compile;
mod overview;
mod plan;
mod publication;

#[derive(Debug)]
pub struct DocumentRequest {
    pub target: TargetSelectionRequestV1,
    pub cache: ArtifactCacheRoot,
    pub compiler: PairedScoopcLocator,
    pub output: PathBuf,
    pub no_deps: bool,
    pub private: bool,
    pub deny_warnings: bool,
}

#[derive(Debug)]
pub struct DocumentOutcome {
    pub output: PathBuf,
    pub cones: Vec<DocumentedCone>,
    pub warnings: Vec<StructuredDiagnosticV1>,
    pub sources: Vec<SelectedSource>,
    pub compiled: Vec<ConeIdentity>,
    pub rendered: bool,
    pub rendered_files: Vec<PathBuf>,
}

#[derive(Debug)]
pub struct DocumentedCone {
    pub identity: ConeIdentity,
    pub output: PathBuf,
}

pub fn document(
    graphs: Vec<ResolvedBuildGraph>,
    request: DocumentRequest,
) -> BuildResult<DocumentOutcome> {
    let mut plan = plan::Plan::new(graphs, &request)?;
    let _lock = publication::lock(&request.output)?;
    let mut compiled = Vec::new();
    for site in &mut plan.sites {
        for graph in std::mem::take(&mut site.graphs) {
            compile::graph(
                graph,
                &request,
                &site.sources,
                &site.selected,
                &mut compiled,
            )?;
        }
    }
    let mut warnings = Vec::new();
    let mut doc_warnings = Vec::new();
    let mut overviews = Vec::new();
    for site in &plan.sites {
        let mut selected = BTreeMap::new();
        for identity in &site.selected {
            let source = &site.sources[identity];
            let record = cache::Entry::new(&request.cache, &source.key).read()?;
            warnings.extend(record.ordinary);
            doc_warnings.extend(record.documentation);
            let overview = overview::select(source, &request.cache)?;
            doc_warnings.extend(overview.warnings.iter().cloned());
            selected.insert(*identity, overview);
        }
        overviews.push(selected);
    }
    let sources = plan
        .sites
        .iter()
        .flat_map(|site| site.sources.values())
        .flat_map(|source| source.sources.iter().cloned())
        .collect::<Vec<_>>();
    warnings.extend(doc_warnings.iter().cloned());
    let mut unique = Vec::new();
    warnings.retain(|warning| {
        if unique.contains(warning) {
            false
        } else {
            unique.push(warning.clone());
            true
        }
    });
    if request.deny_warnings && !doc_warnings.is_empty() {
        return Err(error("documentation warnings are denied").with_context(&warnings, &sources));
    }
    let (rendered, rendered_files) = publication::publish(&request, &plan.sites, &overviews)
        .map_err(|failure| failure.with_context(&warnings, &sources))?;
    Ok(DocumentOutcome {
        output: request.output.join("index.html"),
        cones: plan
            .sites
            .iter()
            .flat_map(|site| site.selected.iter().map(move |id| (site, id)))
            .map(|(site, identity)| {
                let key = scoop_doc::ConeKey::new(*identity).map_err(error)?;
                Ok(DocumentedCone {
                    identity: *identity,
                    output: request
                        .output
                        .join(&site.prefix)
                        .join("c")
                        .join(key.0)
                        .join("index.html"),
                })
            })
            .collect::<BuildResult<_>>()?,
        warnings,
        sources,
        compiled,
        rendered,
        rendered_files,
    })
}

fn error(message: impl std::fmt::Display) -> Box<BuildFailure> {
    BuildFailure::tool(
        "SCOOP_DOCUMENTATION",
        BuildFailurePhase::Publication,
        message,
    )
}

fn io_error(path: &Path, message: impl std::fmt::Display) -> Box<BuildFailure> {
    error(message).at_host(path)
}

fn input_key(value: &impl serde::Serialize) -> BuildResult<String> {
    serde_json::to_vec(value)
        .map(|bytes| scoop_wire::sha256(&bytes).to_string())
        .map_err(error)
}
