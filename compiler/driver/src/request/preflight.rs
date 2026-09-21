use std::fmt;
use std::path::Path;
use std::rc::Rc;

use scoop_ast::{CurrentConeParsedSources, NonEmptyVec};
use scoop_identity::{ConeCoordinate, ConeIdentity, SemanticIdentitySession};
use scoop_manifest::{
    DiscoveredManifestSources, DiscoveredSource, LoadedConeManifest, ManifestRootError,
    SingleFileInputError, SingleFileInputErrorKind, SingleFileLocator, SourceDiscoveryError,
    SourceDiscoveryErrorKind, discover_manifest_sources, load_cone_manifest,
    load_single_file_source,
};
use scoop_parser::{CurrentConeSourceInput, ParseCurrentConeError, parse_current_cone};
use scoop_slib::SlibClosureResourceErrorV1;
#[cfg(test)]
use scoop_slib::{SlibClosureDecodeLimitsV1, SlibClosureDecodeMeterV1};
use scoop_wire::DecodeLimits;

use super::{
    CurrentConeDiagnosticSet, CurrentConeInput, DiagnosticOutputPolicy, EmittedStageDump,
    SingleConeBuildRequest, SingleConeProductionSuccess, SlibOutputDestination, StageDumpKind,
    StageDumpPolicy, TrustedCoreInput,
};
use crate::{CrossConeStrongIrProductionV1, ValidatedTrustedCoreArtifact};

#[cfg(test)]
mod bootstrap;
mod current_hir;
pub use current_hir::{CurrentConeHirStageError, CurrentConeStrongProfileError};
mod dependencies;
#[cfg(test)]
mod end_to_end_tests;
mod machine;
mod metering;
pub use machine::{CurrentConeLirStageError, CurrentConeMirStageError};
mod production;
mod validated;
#[cfg(test)]
use bootstrap::*;
use dependencies::LoadedExplicitDependencyInputs;
pub use dependencies::{
    ExplicitDependencyArtifactInput, ExplicitDependencyLoadError, ExplicitDependencyLoadOperation,
    ExplicitDependencyRole, ExplicitDependencyValidationError,
};
pub use production::{CurrentConeProductionError, CurrentConeProductionFailure};
pub use validated::{
    SingleConeDependencyValidationError, ValidatedCompilerProtocols, ValidatedCurrentConeInput,
    ValidatedSingleConeBuildRequest,
};

/// Proof that the manifest, explicit artifacts, and their recursive closure
/// were validated before current-source discovery begins.
pub struct ValidatedExplicitDependencyInputSet<'input> {
    closure: Rc<scoop_slib::ValidatedCrossConeArtifactClosure<'input>>,
    dependency_first: Vec<&'input [u8]>,
    direct_dependencies: Vec<scoop_slib::DependencyRecord>,
    _semantic_session: SemanticIdentitySession,
}

impl<'input> ValidatedExplicitDependencyInputSet<'input> {
    pub fn is_empty(&self) -> bool {
        self.closure.artifact_count() == 0
    }

    pub(crate) fn new(
        closure: Rc<scoop_slib::ValidatedCrossConeArtifactClosure<'input>>,
        dependency_first: Vec<&'input [u8]>,
        direct_dependencies: Vec<scoop_slib::DependencyRecord>,
        semantic_session: SemanticIdentitySession,
    ) -> Self {
        Self {
            closure,
            dependency_first,
            direct_dependencies,
            _semantic_session: semantic_session,
        }
    }

    pub(crate) fn semantic(&self) -> &scoop_slib::ValidatedCrossConeSemanticClosure<'_> {
        self.closure.semantic()
    }

    pub(crate) fn dependency_first(&self) -> &[&'input [u8]] {
        &self.dependency_first
    }

    pub(crate) fn direct_dependencies(&self) -> &[scoop_slib::DependencyRecord] {
        &self.direct_dependencies
    }
}

#[derive(Debug)]
pub enum LoadedCurrentConeInput {
    Manifest { manifest: Box<LoadedConeManifest> },
    SingleFile { source: SingleFileLocator },
}

#[derive(Debug)]
pub struct LoadedSingleConeBuildRequest {
    current: LoadedCurrentConeInput,
    dependencies: LoadedExplicitDependencyInputs,
    target: scoop_toolchain::ResolvedTargetProfile,
    output: SlibOutputDestination,
    diagnostics: DiagnosticOutputPolicy,
    emit: StageDumpPolicy,
}

impl SingleConeBuildRequest {
    /// Executes the only single-Cone production path and atomically publishes
    /// the resulting `.slib` artifact.
    pub fn build_and_publish(
        self,
        limits: DecodeLimits,
    ) -> Result<SingleConeProductionSuccess, SingleConeProductionError> {
        self.build_and_publish_with_closure_limits(limits, metering::production_closure_limits())
    }

    /// Loads the current manifest, every explicit dependency artifact, and
    /// trusted core bytes. Current source discovery and parsing remain
    /// deliberately unavailable at this stage.
    pub fn load_preflight(
        self,
        limits: DecodeLimits,
    ) -> Result<LoadedSingleConeBuildRequest, SingleConePreflightError> {
        self.load_preflight_inner(limits, None)
    }
}

fn capture_stage_dump(
    policy: StageDumpPolicy,
    kind: StageDumpKind,
    render: impl FnOnce() -> String,
) -> Option<EmittedStageDump> {
    if policy == StageDumpPolicy::Stage(kind) {
        Some(EmittedStageDump::new(kind, render()))
    } else {
        None
    }
}

#[derive(Debug)]
pub enum SingleConeProductionError {
    TemporaryWorkspace(std::io::Error),
    Preflight(SingleConePreflightError),
    Validation(SingleConeDependencyValidationError),
    Sources(CurrentConeSourceStageError),
    Production(Box<CurrentConeProductionError>),
}

impl fmt::Display for SingleConeProductionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TemporaryWorkspace(source) => {
                write!(
                    formatter,
                    "cannot create temporary build workspace: {source}"
                )
            }
            Self::Preflight(source) => source.fmt(formatter),
            Self::Validation(source) => source.fmt(formatter),
            Self::Sources(source) => source.fmt(formatter),
            Self::Production(source) => source.fmt(formatter),
        }
    }
}

impl std::error::Error for SingleConeProductionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::TemporaryWorkspace(source) => source,
            Self::Preflight(source) => source,
            Self::Validation(source) => source,
            Self::Sources(source) => source,
            Self::Production(source) => source,
        })
    }
}

fn load_current_input(
    current: CurrentConeInput,
) -> Result<LoadedCurrentConeInput, SingleConePreflightError> {
    match current {
        CurrentConeInput::Manifest { root } => load_cone_manifest(&root)
            .map(|manifest| LoadedCurrentConeInput::Manifest {
                manifest: Box::new(manifest),
            })
            .map_err(|source| SingleConePreflightError::Manifest(Box::new(source))),
        CurrentConeInput::SingleFile { source } => {
            Ok(LoadedCurrentConeInput::SingleFile { source })
        }
    }
}

pub struct ParsedSingleConeBuildRequest<'request, 'artifact> {
    request: &'request ValidatedSingleConeBuildRequest<'artifact>,
    sources: CurrentConeParsedSources,
}

impl<'request, 'artifact> ParsedSingleConeBuildRequest<'request, 'artifact> {
    pub const fn request(&self) -> &'request ValidatedSingleConeBuildRequest<'artifact> {
        self.request
    }
    pub const fn sources(&self) -> &CurrentConeParsedSources {
        &self.sources
    }
}

impl SingleConeProductionError {
    pub fn warnings(&self) -> Option<&CurrentConeDiagnosticSet> {
        match self {
            Self::Production(error) => error.warnings(),
            Self::TemporaryWorkspace(_)
            | Self::Preflight(_)
            | Self::Validation(_)
            | Self::Sources(_) => None,
        }
    }
}

fn parse_manifest_current(
    manifest: &LoadedConeManifest,
) -> Result<CurrentConeParsedSources, CurrentConeSourceStageError> {
    let sources = discover_manifest_sources(manifest)
        .map_err(|source| CurrentConeSourceStageError::Discovery(Box::new(source)))?;
    parse_discovered_sources(&sources)
}

fn parse_single_file_current(
    source: &SingleFileLocator,
) -> Result<CurrentConeParsedSources, CurrentConeSourceStageError> {
    let source = load_single_file_source(source)
        .map_err(|source| CurrentConeSourceStageError::SingleFile(Box::new(source)))?;
    parse_single_discovered_source(&source)
}

fn parse_discovered_sources(
    sources: &DiscoveredManifestSources,
) -> Result<CurrentConeParsedSources, CurrentConeSourceStageError> {
    let first = parser_source_input(sources.first());
    let rest = sources.iter().skip(1).map(parser_source_input).collect();
    parse_current_cone(NonEmptyVec::new(first, rest))
        .map_err(|source| CurrentConeSourceStageError::Parser(Box::new(source)))
}

fn parse_single_discovered_source(
    source: &DiscoveredSource,
) -> Result<CurrentConeParsedSources, CurrentConeSourceStageError> {
    parse_current_cone(NonEmptyVec::new(parser_source_input(source), Vec::new()))
        .map_err(|source| CurrentConeSourceStageError::Parser(Box::new(source)))
}

fn parser_source_input(source: &DiscoveredSource) -> CurrentConeSourceInput<'_> {
    CurrentConeSourceInput::new(
        source.identity(),
        source.source_text(),
        source.display_locator().as_path(),
    )
}

#[derive(Debug)]
pub enum CurrentConeSourceStageError {
    Resource(SlibClosureResourceErrorV1),
    Discovery(Box<SourceDiscoveryError>),
    SingleFile(Box<SingleFileInputError>),
    SourceLengthOverflow,
    Parser(Box<ParseCurrentConeError>),
}

impl CurrentConeSourceStageError {
    pub fn is_resource_limit(&self) -> bool {
        match self {
            Self::Resource(_) => true,
            Self::Discovery(source) => matches!(
                source.kind(),
                SourceDiscoveryErrorKind::FileLimitExceeded { .. }
                    | SourceDiscoveryErrorKind::ByteLimitExceeded { .. }
            ),
            Self::SingleFile(source) => matches!(
                source.kind(),
                SingleFileInputErrorKind::ByteLimitExceeded { .. }
            ),
            Self::SourceLengthOverflow | Self::Parser(_) => false,
        }
    }
}

impl fmt::Display for CurrentConeSourceStageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(source) => source.fmt(formatter),
            Self::Discovery(source) => source.fmt(formatter),
            Self::SingleFile(source) => source.fmt(formatter),
            Self::SourceLengthOverflow => {
                formatter.write_str("current source length does not fit u64")
            }
            Self::Parser(source) => source.fmt(formatter),
        }
    }
}

impl std::error::Error for CurrentConeSourceStageError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(source) => Some(source),
            Self::Discovery(source) => Some(source.as_ref()),
            Self::SingleFile(source) => Some(source.as_ref()),
            Self::Parser(source) => Some(source.as_ref()),
            Self::SourceLengthOverflow => None,
        }
    }
}

#[derive(Debug)]
pub enum SingleConePreflightError {
    Manifest(Box<ManifestRootError>),
    ExplicitDependencyLoad(Box<ExplicitDependencyLoadError>),
}

impl fmt::Display for SingleConePreflightError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Manifest(source) => source.fmt(formatter),
            Self::ExplicitDependencyLoad(source) => source.fmt(formatter),
        }
    }
}

impl std::error::Error for SingleConePreflightError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Manifest(source) => Some(source.as_ref()),
            Self::ExplicitDependencyLoad(source) => Some(source.as_ref()),
        }
    }
}

#[cfg(test)]
mod tests;
