use std::fmt;
use std::path::Path;

use scoop_ast::{CurrentConeParsedSources, NonEmptyVec};
use scoop_identity::{ConeCoordinate, ConeIdentity, TargetProfileId};
use scoop_manifest::{
    DiscoveredManifestSources, DiscoveredSource, LoadedConeManifest, SingleFileInputError,
    SingleFileLocator, SourceDiscoveryError, discover_manifest_sources, load_single_file_source,
};
use scoop_parser::{CurrentConeSourceInput, ParseCurrentConeError, parse_current_cone};

#[cfg(test)]
use super::SlibOutputDestination;
use super::{
    CompilationPurpose, CurrentConeDiagnosticSet, CurrentConeInput, DiagnosticOutputPolicy,
    EmittedStageDump, SingleConeBuildRequest, SingleConeCompilation, SingleConeProductionSuccess,
    StageDumpKind, StageDumpPolicy, StageObservation, TrustedCoreInput,
};

mod current_hir;
pub use current_hir::CurrentConeHirStageError;
mod dependencies;
#[cfg(test)]
mod end_to_end_tests;
mod loading;
mod machine;
mod observation;
mod packages;
#[cfg(test)]
mod single_file_tests;
pub use machine::{CurrentConeLirStageError, CurrentConeMirStageError};
mod production;
mod validated;
use dependencies::LoadedExplicitDependencyInputs;
pub use dependencies::{
    ExplicitDependencyArtifactInput, ExplicitDependencyLoadError, ExplicitDependencyLoadOperation,
    ExplicitDependencyRole, ExplicitDependencyValidationError,
};
pub use production::{
    CurrentConeProductionError, CurrentConeProductionFailure, DocumentationProductionError,
};
pub use validated::{
    SingleConeDependencyValidationError, ValidatedCompilerProtocols, ValidatedCurrentConeInput,
    ValidatedSingleConeBuildRequest,
};

/// Source declarations and file inputs for the complete visibility graph.
pub struct ValidatedExplicitDependencyInputSet<'input> {
    closure: scoop_slib::CompileDependencyClosure<'input>,
    direct_dependencies: Vec<scoop_slib::DependencyRecord>,
    inputs: std::collections::BTreeMap<ConeIdentity, ExplicitDependencyArtifactInput>,
    order: Vec<ConeIdentity>,
}

impl<'input> ValidatedExplicitDependencyInputSet<'input> {
    pub fn is_empty(&self) -> bool {
        self.closure.provider_count() == 0
    }

    pub(crate) fn new(
        closure: scoop_slib::CompileDependencyClosure<'input>,
        direct_dependencies: Vec<scoop_slib::DependencyRecord>,
        inputs: std::collections::BTreeMap<ConeIdentity, ExplicitDependencyArtifactInput>,
        order: Vec<ConeIdentity>,
    ) -> Self {
        Self {
            closure,
            direct_dependencies,
            inputs,
            order,
        }
    }

    pub(crate) fn semantic(&self) -> &scoop_slib::CompileDependencyClosure<'input> {
        &self.closure
    }

    pub(crate) fn direct_dependencies(&self) -> &[scoop_slib::DependencyRecord] {
        &self.direct_dependencies
    }
}

#[derive(Debug)]
pub enum LoadedCurrentConeInput {
    Manifest {
        manifest: Box<LoadedConeManifest>,
    },
    SingleFile {
        source: SingleFileLocator,
        input: Box<DiscoveredSource>,
        dependencies: Vec<scoop_manifest::SingleFileDependency>,
    },
}

#[derive(Debug)]
pub struct LoadedSingleConeBuildRequest {
    current: LoadedCurrentConeInput,
    dependencies: LoadedExplicitDependencyInputs,
    target: super::target::SelectedTarget,
    output: super::SingleConeOutput,
    diagnostics: DiagnosticOutputPolicy,
    emit: StageDumpPolicy,
    optimization: scoop_lir::OptimizationMode,
    debug_info: scoop_lir::DebugInfoLevel,
    purpose: CompilationPurpose,
    native_inputs: super::NativeInputOrigin,
    generators: scoop_toolchain::GeneratorPrograms,
}

impl SingleConeBuildRequest {
    /// Executes the only single-Cone production path and atomically publishes
    /// the resulting `.slib` artifact.
    pub fn build_and_publish(
        self,
    ) -> Result<SingleConeProductionSuccess, SingleConeProductionError> {
        if self.output.artifact().is_none() {
            return Err(SingleConeProductionError::ArtifactOutputRequired);
        }
        match self.compile(false)? {
            SingleConeCompilation::Published(result) => Ok(*result),
            SingleConeCompilation::Observed(_)
            | SingleConeCompilation::Documented(_)
            | SingleConeCompilation::Checked(_) => {
                unreachable!("production has no observation stop")
            }
        }
    }

    pub fn compile(
        self,
        observation_only: bool,
    ) -> Result<SingleConeCompilation, SingleConeProductionError> {
        if observation_only && self.output.documentation().is_some() {
            return Err(SingleConeProductionError::DocumentationObservation);
        }
        let stop_after = if observation_only {
            let StageDumpPolicy::Stages(stages) = self.emit else {
                return Err(SingleConeProductionError::ObservationRequiresStages);
            };
            stages.iter().last()
        } else {
            None
        };
        if stop_after == Some(StageDumpKind::Ast) {
            return self.observe_ast();
        }
        let temporary = self
            .output
            .primary_path()
            .map(|path| {
                tempfile::Builder::new()
                    .prefix(".scoopc-")
                    .tempdir_in(path.parent().expect("absolute output has a parent"))
            })
            .transpose()
            .map_err(SingleConeProductionError::TemporaryWorkspace)?;
        let loaded = self
            .load_preflight()
            .map_err(SingleConeProductionError::Preflight)?;
        let sources = loaded
            .parse_sources()
            .map_err(SingleConeProductionError::Sources)?;
        let validated = loaded
            .validate()
            .map_err(SingleConeProductionError::Validation)?;
        let parsed = ParsedSingleConeBuildRequest {
            request: &validated,
            sources,
        };
        parsed
            .compile(
                temporary.as_ref().map(|directory| directory.path()),
                stop_after,
            )
            .map_err(|source| SingleConeProductionError::Production(Box::new(source)))
    }

    /// Loads the current manifest, every explicit dependency artifact, and
    /// trusted core bytes. A single file is read once for its dependency header
    /// and retained for parsing; manifest source discovery remains deferred.
    pub fn load_preflight(self) -> Result<LoadedSingleConeBuildRequest, SingleConePreflightError> {
        self.load_preflight_inner()
    }
}

fn capture_stage_dump(
    policy: StageDumpPolicy,
    kind: StageDumpKind,
    render: impl FnOnce() -> String,
) -> Vec<EmittedStageDump> {
    if matches!(policy, StageDumpPolicy::Stages(stages) if stages.contains(kind)) {
        vec![EmittedStageDump::new(kind, render())]
    } else {
        Vec::new()
    }
}

#[derive(Debug)]
pub enum SingleConeProductionError {
    ArtifactOutputRequired,
    DocumentationObservation,
    ObservationRequiresStages,
    TemporaryWorkspace(std::io::Error),
    Preflight(SingleConePreflightError),
    Validation(SingleConeDependencyValidationError),
    Sources(CurrentConeSourceStageError),
    Production(Box<CurrentConeProductionError>),
}

impl fmt::Display for SingleConeProductionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ArtifactOutputRequired => formatter.write_str(
                "artifact production requires an artifact output; use compile for documentation",
            ),
            Self::DocumentationObservation => {
                formatter.write_str("documentation cannot be combined with observation-only")
            }
            Self::ObservationRequiresStages => {
                formatter.write_str("stage observation requires at least one dump")
            }
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
            Self::ArtifactOutputRequired
            | Self::DocumentationObservation
            | Self::ObservationRequiresStages => return None,
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
        CurrentConeInput::ExpandedManifest { manifest } => {
            Ok(LoadedCurrentConeInput::Manifest { manifest })
        }
        CurrentConeInput::SingleFile { source } => {
            let input = load_single_file_source(&source)
                .map_err(|error| SingleConePreflightError::SingleFile(Box::new(error)))?;
            let dependencies = scoop_manifest::parse_single_file_dependencies(input.source_text())
                .map_err(|error| {
                    SingleConePreflightError::SingleFile(Box::new(SingleFileInputError::header(
                        source.display_path().to_owned(),
                        error,
                    )))
                })?;
            Ok(LoadedCurrentConeInput::SingleFile {
                source,
                input: Box::new(input),
                dependencies,
            })
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
            Self::ArtifactOutputRequired
            | Self::DocumentationObservation
            | Self::ObservationRequiresStages
            | Self::TemporaryWorkspace(_)
            | Self::Preflight(_)
            | Self::Validation(_)
            | Self::Sources(_) => None,
        }
    }
}

fn parse_manifest_current(
    manifest: &LoadedConeManifest,
    target: TargetProfileId,
    purpose: CompilationPurpose,
) -> Result<CurrentConeParsedSources, CurrentConeSourceStageError> {
    let sources = match purpose {
        CompilationPurpose::Production => discover_manifest_sources(manifest, target),
        CompilationPurpose::Test => {
            scoop_manifest::discover_manifest_sources_for_tests(manifest, target)
        }
    }
    .map_err(|source| CurrentConeSourceStageError::Discovery(Box::new(source)))?;
    let parsed = parse_discovered_sources(&sources)?;
    packages::validate(manifest, &parsed)?;
    Ok(parsed)
}

#[cfg(test)]
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
    .with_role(source.role())
}

#[derive(Debug)]
pub enum CurrentConeSourceStageError {
    Discovery(Box<SourceDiscoveryError>),
    SingleFile(Box<SingleFileInputError>),

    Parser(Box<ParseCurrentConeError>),
    Package(Box<CurrentConeDiagnosticSet>),
}

impl fmt::Display for CurrentConeSourceStageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Discovery(source) => source.fmt(formatter),
            Self::SingleFile(source) => source.fmt(formatter),

            Self::Parser(source) => source.fmt(formatter),
            Self::Package(diagnostics) => formatter.write_str(&diagnostics.render_human()),
        }
    }
}

impl std::error::Error for CurrentConeSourceStageError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Discovery(source) => Some(source.as_ref()),
            Self::SingleFile(source) => Some(source.as_ref()),
            Self::Parser(source) => Some(source.as_ref()),
            Self::Package(_) => None,
        }
    }
}

#[derive(Debug)]
pub enum SingleConePreflightError {
    SingleFile(Box<SingleFileInputError>),
    ExplicitDependencyLoad(Box<ExplicitDependencyLoadError>),
    Dependencies(Box<ExplicitDependencyValidationError>),
    DefaultCoreSlot(Box<crate::TrustedCoreSlotError>),
    Request(Box<super::SingleConeBuildRequestError>),
}

impl fmt::Display for SingleConePreflightError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SingleFile(source) => source.fmt(formatter),
            Self::ExplicitDependencyLoad(source) => source.fmt(formatter),
            Self::Dependencies(source) => source.fmt(formatter),
            Self::DefaultCoreSlot(source) => source.fmt(formatter),
            Self::Request(source) => source.fmt(formatter),
        }
    }
}

impl std::error::Error for SingleConePreflightError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::SingleFile(source) => Some(source.as_ref()),
            Self::ExplicitDependencyLoad(source) => Some(source.as_ref()),
            Self::Dependencies(source) => Some(source.as_ref()),
            Self::DefaultCoreSlot(source) => Some(source.as_ref()),
            Self::Request(source) => Some(source.as_ref()),
        }
    }
}

#[cfg(test)]
mod tests;
