use std::fmt;
use std::path::{Path, PathBuf};

mod current;
mod documentation;
#[cfg(test)]
mod test_support;
pub use current::CurrentConeInput;
mod normalization;
mod output;
mod target;
pub use documentation::{DocumentationOutputDestination, SingleConeOutput};
pub use normalization::{
    DirectBuildOptions, DirectCheckOptions, normalize_direct_build_request,
    normalize_direct_check_request, normalize_protocol_build_request,
};
mod preflight;
mod report;
use output::validate_output_isolation;
pub use output::{OutputAliasRole, OutputIsolationErrorKind, SlibOutputDestination};
pub use preflight::*;
pub use report::{
    CurrentConeDiagnosticSet, CurrentConeDiagnosticSetError, DiagnosticMappingError,
    EmittedStageDump, PublishedDocumentation, SingleConeCheckSuccess, SingleConeCompilation,
    SingleConeDocumentationSuccess, SingleConeProductionSuccess, StageObservation,
};

use scoop_codegen::CodegenError;
use scoop_manifest::{ManifestRootError, SingleFileInputError, SingleFileLocator};
pub use scoop_protocol::CompilationPurpose;
use scoop_protocol::HostPathError;
use scoop_toolchain::{ResolvedTargetProfile, ToolchainError};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostArtifactLocator(PathBuf);

impl HostArtifactLocator {
    pub fn new(path: impl Into<PathBuf>) -> Result<Self, SingleConeBuildRequestError> {
        let path = path.into();
        if path.as_os_str().is_empty() {
            return Err(SingleConeBuildRequestError::EmptyArtifactLocator);
        }
        Ok(Self(path))
    }

    pub fn as_path(&self) -> &Path {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExplicitDependencyInputs {
    direct: Vec<HostArtifactLocator>,
    support: Vec<HostArtifactLocator>,
}

impl ExplicitDependencyInputs {
    pub fn new(direct: Vec<HostArtifactLocator>, support: Vec<HostArtifactLocator>) -> Self {
        Self { direct, support }
    }

    pub fn direct(&self) -> &[HostArtifactLocator] {
        &self.direct
    }

    pub fn support(&self) -> &[HostArtifactLocator] {
        &self.support
    }

    pub fn is_empty(&self) -> bool {
        self.direct.is_empty() && self.support.is_empty()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TrustedCoreInput {
    Artifact(HostArtifactLocator),
    BootstrapSelf,
    DependenciesOrDefault { sysroot: PathBuf },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticOutputPolicy {
    Human,
    Structured,
}

pub use scoop_protocol::{StageDumpKindV1 as StageDumpKind, StageDumpSet};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StageDumpPolicy {
    None,
    Stages(StageDumpSet),
}

#[derive(Debug)]
pub struct SingleConeBuildRequest {
    current: CurrentConeInput,
    dependencies: ExplicitDependencyInputs,
    trusted_core: TrustedCoreInput,
    target: target::SelectedTarget,
    output: SingleConeOutput,
    diagnostics: DiagnosticOutputPolicy,
    emit: StageDumpPolicy,
    optimization: scoop_lir::OptimizationMode,
    debug_info: scoop_lir::DebugInfoLevel,
    purpose: CompilationPurpose,
    native_inputs: NativeInputOrigin,
    generators: scoop_toolchain::GeneratorPrograms,
    object_cache_root: Option<PathBuf>,
    jobs: std::num::NonZeroUsize,
}

#[derive(Debug)]
enum NativeInputOrigin {
    Source { sysroot: Option<PathBuf> },
    Objects(Vec<PathBuf>),
}

impl SingleConeBuildRequest {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        current: CurrentConeInput,
        dependencies: ExplicitDependencyInputs,
        trusted_core: TrustedCoreInput,
        target: ResolvedTargetProfile,
        output: impl Into<SingleConeOutput>,
        diagnostics: DiagnosticOutputPolicy,
        emit: StageDumpPolicy,
    ) -> Result<Self, SingleConeBuildRequestError> {
        Self::new_selected(
            current,
            dependencies,
            trusted_core,
            target::SelectedTarget::Production(Box::new(target)),
            output.into(),
            diagnostics,
            emit,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn new_selected(
        current: CurrentConeInput,
        dependencies: ExplicitDependencyInputs,
        trusted_core: TrustedCoreInput,
        target: target::SelectedTarget,
        output: SingleConeOutput,
        diagnostics: DiagnosticOutputPolicy,
        emit: StageDumpPolicy,
    ) -> Result<Self, SingleConeBuildRequestError> {
        validate_request_shape(&current, &dependencies, &trusted_core, &output)?;
        output.validate(&current, CompilationPurpose::Production, emit)?;
        Ok(Self {
            current,
            dependencies,
            trusted_core,
            target,
            output,
            diagnostics,
            emit,
            optimization: scoop_lir::OptimizationMode::Debug,
            debug_info: scoop_lir::DebugInfoLevel::None,
            purpose: CompilationPurpose::Production,
            native_inputs: NativeInputOrigin::Source { sysroot: None },
            generators: scoop_toolchain::GeneratorPrograms::default(),
            object_cache_root: None,
            jobs: std::num::NonZeroUsize::MIN,
        })
    }

    pub fn with_optimization(mut self, mode: scoop_lir::OptimizationMode) -> Self {
        self.optimization = mode;
        self
    }

    pub fn with_jobs(mut self, jobs: std::num::NonZeroUsize) -> Self {
        self.jobs = jobs;
        self
    }

    pub fn with_object_cache_root(mut self, root: Option<PathBuf>) -> Self {
        self.object_cache_root = root;
        self
    }

    pub fn with_debug_info(mut self, level: scoop_lir::DebugInfoLevel) -> Self {
        self.debug_info = level;
        self
    }

    pub fn with_generators(mut self, programs: scoop_toolchain::GeneratorPrograms) -> Self {
        self.generators = programs;
        self
    }

    pub fn with_purpose(
        mut self,
        purpose: CompilationPurpose,
    ) -> Result<Self, SingleConeBuildRequestError> {
        if purpose == CompilationPurpose::Test
            && matches!(self.current, CurrentConeInput::SingleFile { .. })
        {
            return Err(SingleConeBuildRequestError::TestRequiresManifest);
        }
        self.purpose = purpose;
        self.output.validate(&self.current, purpose, self.emit)?;
        Ok(self)
    }

    fn with_native_inputs(mut self, inputs: Vec<PathBuf>) -> Self {
        self.native_inputs = NativeInputOrigin::Objects(inputs);
        self
    }

    fn with_native_sysroot(mut self, sysroot: PathBuf) -> Self {
        self.native_inputs = NativeInputOrigin::Source {
            sysroot: Some(sysroot),
        };
        self
    }
}

#[derive(Debug)]
pub enum SingleConeBuildRequestError {
    EmptyArtifactLocator,
    EmptyOutputDestination,
    InvalidOutputExtension {
        path: PathBuf,
    },
    CurrentDirectory(std::io::Error),
    InvalidCurrentCoreCombination,
    TestRequiresManifest,
    DocumentationRequiresProductionManifest,
    DocumentationMachineDump,
    CheckRequiresNoMachineInputs,
    InvalidDocumentationOutput {
        path: PathBuf,
    },
    OutputIsolation {
        path: PathBuf,
        kind: OutputIsolationErrorKind,
    },
}

impl fmt::Display for SingleConeBuildRequestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyArtifactLocator => formatter.write_str("artifact locator must not be empty"),
            Self::EmptyOutputDestination => {
                formatter.write_str(".slib output destination must not be empty")
            }
            Self::InvalidOutputExtension { path } => write!(
                formatter,
                ".slib output destination must use the exact .slib extension: {}",
                path.display()
            ),
            Self::CurrentDirectory(error) => {
                write!(formatter, "cannot resolve current directory: {error}")
            }
            Self::InvalidCurrentCoreCombination => formatter
                .write_str("current Cone input and trusted core input are not a permitted pair"),

            Self::TestRequiresManifest => {
                formatter.write_str("test compilation requires a manifest Cone")
            }
            Self::DocumentationRequiresProductionManifest => {
                formatter.write_str("documentation requires a Production manifest Cone")
            }
            Self::CheckRequiresNoMachineInputs => {
                formatter.write_str("check requests cannot include dumps or native objects")
            }
            Self::DocumentationMachineDump => formatter
                .write_str("documentation-only output cannot emit MIR, optimizations or LIR"),
            Self::InvalidDocumentationOutput { path } => write!(
                formatter,
                "documentation input output must use the exact .json extension: {}",
                path.display()
            ),
            Self::OutputIsolation { path, kind } => {
                write!(formatter, "invalid .slib output {}: {kind}", path.display())
            }
        }
    }
}

impl std::error::Error for SingleConeBuildRequestError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::CurrentDirectory(error) => Some(error),
            Self::OutputIsolation { kind, .. } => kind.source(),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub enum BuildRequestNormalizationError {
    ManifestRequiresScoop,
    ManifestRoot(ManifestRootError),
    SingleFile(SingleFileInputError),
    HostPath(HostPathError),
    Target(ToolchainError),
    Generators(ToolchainError),
    Backend(CodegenError),
    Request(SingleConeBuildRequestError),
}

impl fmt::Display for BuildRequestNormalizationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ManifestRequiresScoop => formatter.write_str(
                "scoopc does not read Cone.toml; use scoop build/check for Cone projects",
            ),
            Self::ManifestRoot(error) => error.fmt(formatter),
            Self::SingleFile(error) => error.fmt(formatter),
            Self::HostPath(error) => error.fmt(formatter),
            Self::Target(error) => error.fmt(formatter),
            Self::Generators(error) => error.fmt(formatter),
            Self::Backend(error) => error.fmt(formatter),
            Self::Request(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for BuildRequestNormalizationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ManifestRequiresScoop => None,
            Self::ManifestRoot(error) => Some(error),
            Self::SingleFile(error) => Some(error),
            Self::HostPath(error) => Some(error),
            Self::Target(error) => Some(error),
            Self::Generators(error) => Some(error),
            Self::Backend(error) => Some(error),
            Self::Request(error) => Some(error),
        }
    }
}

fn validate_request_shape(
    current: &CurrentConeInput,
    dependencies: &ExplicitDependencyInputs,
    trusted_core: &TrustedCoreInput,
    output: &SingleConeOutput,
) -> Result<(), SingleConeBuildRequestError> {
    match (current, trusted_core) {
        (CurrentConeInput::ExpandedManifest { .. }, _) => Ok(()),
        (
            CurrentConeInput::SingleFile { .. },
            TrustedCoreInput::Artifact(_) | TrustedCoreInput::DependenciesOrDefault { .. },
        ) => Ok(()),
        _ => Err(SingleConeBuildRequestError::InvalidCurrentCoreCombination),
    }?;
    if let Some(output) = output.artifact() {
        validate_output_isolation(current, dependencies, trusted_core, output)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
