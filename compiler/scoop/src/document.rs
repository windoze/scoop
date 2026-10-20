//! Source documentation uses independent language graphs and one site catalog.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use scoop_identity::ConeIdentity;
use scoop_protocol::{StructuredDiagnosticV1, TargetSelectionRequestV1};

use crate::{
    ArtifactCacheRoot, ArtifactSearchRoot, BuildFailure, BuildFailurePhase, BuildResult,
    BuildRootInput, PairedScoopcLocator, SelectedSource, TrustedSysrootRoot,
};

mod cache;
mod compile;
mod overview;
mod plan;
mod publication;

#[derive(Debug)]
pub struct DocumentRequest {
    pub roots: Vec<BuildRootInput>,
    pub target: TargetSelectionRequestV1,
    pub sysroot: TrustedSysrootRoot,
    pub cache: ArtifactCacheRoot,
    pub compiler: PairedScoopcLocator,
    pub search: Vec<ArtifactSearchRoot>,
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

pub fn document(request: DocumentRequest) -> BuildResult<DocumentOutcome> {
    let plan = plan::Plan::new(&request)?;
    let _lock = publication::lock(&request.output)?;
    let mut compiled = Vec::new();
    for graph in plan.graphs {
        compile::graph(
            graph,
            &request,
            &plan.sources,
            &plan.selected,
            &mut compiled,
        )?;
    }
    let mut warnings = Vec::new();
    let mut doc_warnings = Vec::new();
    let mut overviews = BTreeMap::new();
    for identity in &plan.selected {
        let source = &plan.sources[identity];
        let record = cache::Entry::new(&request.cache, &source.key).read()?;
        warnings.extend(record.ordinary);
        doc_warnings.extend(record.documentation);
        let overview = overview::select(source, &request.cache)?;
        doc_warnings.extend(overview.warnings.iter().cloned());
        overviews.insert(*identity, overview);
    }
    let sources = plan
        .sources
        .values()
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
    let (rendered, rendered_files) =
        publication::publish(&request, &plan.sources, &plan.selected, &overviews)
            .map_err(|failure| failure.with_context(&warnings, &sources))?;
    Ok(DocumentOutcome {
        output: request.output.join("index.html"),
        cones: plan
            .selected
            .iter()
            .map(|identity| {
                let key = scoop_doc::ConeKey::new(*identity).map_err(error)?;
                Ok(DocumentedCone {
                    identity: *identity,
                    output: request.output.join("c").join(key.0).join("index.html"),
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
