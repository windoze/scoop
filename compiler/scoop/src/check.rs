//! Check one root through complete HIR, building only its source dependencies.

use std::path::PathBuf;

use scoop_identity::{ConeCoordinate, ConeIdentity};
use scoop_protocol::{CompilationPurpose, StructuredDiagnosticV1, TargetSelectionRequestV1};

use crate::{
    ArtifactCacheRoot, ArtifactSearchRoot, BuildFailure, BuildFailurePhase, BuildProfile,
    BuildResult, BuildRootInput, PairedScoopcLocator, SelectedSource, TrustedSysrootRoot,
};

mod cache;
mod compile;
mod plan;

#[derive(Debug)]
pub struct CheckRequest {
    pub root: BuildRootInput,
    pub target: TargetSelectionRequestV1,
    pub sysroot: TrustedSysrootRoot,
    pub cache: ArtifactCacheRoot,
    pub compiler: PairedScoopcLocator,
    pub search: Vec<ArtifactSearchRoot>,
    pub purpose: CompilationPurpose,
    pub profile: BuildProfile,
    pub jobs: std::num::NonZeroUsize,
    pub generators: scoop_toolchain::GeneratorPrograms,
}

#[derive(Debug)]
pub struct CheckOutcome {
    pub root: ConeCoordinate,
    pub target: String,
    pub purpose: CompilationPurpose,
    pub cache_hit: bool,
    pub warnings: Vec<StructuredDiagnosticV1>,
    pub sources: Vec<SelectedSource>,
    pub compiled_dependencies: Vec<ConeIdentity>,
}

pub fn check(request: CheckRequest) -> BuildResult<CheckOutcome> {
    let plan = plan::Plan::new(&request)?;
    let entry = cache::Entry::new(&request.cache, &plan.key);
    let _lock = entry.lock()?;
    let (warnings, cache_hit, compiled_dependencies) = match entry.current(&request)? {
        Some(result) => (result.warnings, true, Vec::new()),
        None => {
            let (record, compiled) = compile::run(plan.graph, &request, &plan.sources)?;
            let warnings = record.success()?.warnings;
            entry.save(&record)?;
            (warnings, false, compiled)
        }
    };
    Ok(CheckOutcome {
        root: plan.root,
        target: request.target.canonical_triple().into(),
        purpose: request.purpose,
        cache_hit,
        warnings,
        sources: plan.sources,
        compiled_dependencies,
    })
}

fn error(message: impl std::fmt::Display) -> Box<BuildFailure> {
    BuildFailure::tool("SCOOP_CHECK", BuildFailurePhase::ChildDiagnostic, message)
}

fn io_error(path: &std::path::Path, message: impl std::fmt::Display) -> Box<BuildFailure> {
    error(message).at_host(path)
}

fn input_key(value: &impl serde::Serialize) -> BuildResult<String> {
    serde_json::to_vec(value)
        .map(|bytes| scoop_wire::sha256(&bytes).to_string())
        .map_err(error)
}
