use std::fmt;
use std::path::{Path, PathBuf};

mod normalization;
mod output;
pub use normalization::{
    DirectBuildOptions, normalize_direct_build_request, normalize_protocol_build_request,
};
mod preflight;
mod report;
use output::validate_output_isolation;
pub use output::{OutputAliasRole, OutputIsolationErrorKind, SlibOutputDestination};
pub use preflight::*;
pub use report::{
    CurrentConeDiagnosticSet, CurrentConeDiagnosticSetError, DiagnosticMappingError,
    EmittedStageDump, SingleConeProductionSuccess,
};

use scoop_codegen::CodegenError;
pub use scoop_manifest::{
    CurrentConeInput, CurrentConeOperandError, CurrentConeOperandErrorKind,
    classify_current_cone_operand,
};
use scoop_manifest::{
    ManifestRootError, ManifestRootLocator, SingleFileInputError, SingleFileLocator,
};
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
    target: ResolvedTargetProfile,
    output: SlibOutputDestination,
    diagnostics: DiagnosticOutputPolicy,
    emit: StageDumpPolicy,
    optimization: scoop_lir::OptimizationMode,
    native_inputs: NativeInputOrigin,
}

#[derive(Debug, Default)]
enum NativeInputOrigin {
    #[default]
    Source,
    Preprocessed(Vec<PathBuf>),
}

impl SingleConeBuildRequest {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        current: CurrentConeInput,
        dependencies: ExplicitDependencyInputs,
        trusted_core: TrustedCoreInput,
        target: ResolvedTargetProfile,
        output: SlibOutputDestination,
        diagnostics: DiagnosticOutputPolicy,
        emit: StageDumpPolicy,
    ) -> Result<Self, SingleConeBuildRequestError> {
        validate_request_shape(&current, &dependencies, &trusted_core, &output)?;
        Ok(Self {
            current,
            dependencies,
            trusted_core,
            target,
            output,
            diagnostics,
            emit,
            optimization: scoop_lir::OptimizationMode::Debug,
            native_inputs: NativeInputOrigin::Source,
        })
    }

    pub fn with_optimization(mut self, mode: scoop_lir::OptimizationMode) -> Self {
        self.optimization = mode;
        self
    }

    fn with_native_inputs(mut self, inputs: Vec<PathBuf>) -> Self {
        self.native_inputs = NativeInputOrigin::Preprocessed(inputs);
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
    SingleFileHasDependencies,
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
            Self::SingleFileHasDependencies => {
                formatter.write_str("single-file input cannot carry direct or support artifacts")
            }
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
    CurrentOperand(CurrentConeOperandError),
    ManifestRoot(ManifestRootError),
    SingleFile(SingleFileInputError),
    HostPath(HostPathError),
    Target(ToolchainError),
    Backend(CodegenError),
    Request(SingleConeBuildRequestError),
}

impl fmt::Display for BuildRequestNormalizationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CurrentOperand(error) => error.fmt(formatter),
            Self::ManifestRoot(error) => error.fmt(formatter),
            Self::SingleFile(error) => error.fmt(formatter),
            Self::HostPath(error) => error.fmt(formatter),
            Self::Target(error) => error.fmt(formatter),
            Self::Backend(error) => error.fmt(formatter),
            Self::Request(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for BuildRequestNormalizationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::CurrentOperand(error) => Some(error),
            Self::ManifestRoot(error) => Some(error),
            Self::SingleFile(error) => Some(error),
            Self::HostPath(error) => Some(error),
            Self::Target(error) => Some(error),
            Self::Backend(error) => Some(error),
            Self::Request(error) => Some(error),
        }
    }
}

fn validate_request_shape(
    current: &CurrentConeInput,
    dependencies: &ExplicitDependencyInputs,
    trusted_core: &TrustedCoreInput,
    output: &SlibOutputDestination,
) -> Result<(), SingleConeBuildRequestError> {
    match (current, trusted_core) {
        (CurrentConeInput::Manifest { .. }, _) => Ok(()),
        (
            CurrentConeInput::SingleFile { .. },
            TrustedCoreInput::Artifact(_) | TrustedCoreInput::DependenciesOrDefault { .. },
        ) => {
            if dependencies.is_empty() {
                Ok(())
            } else {
                Err(SingleConeBuildRequestError::SingleFileHasDependencies)
            }
        }
        _ => Err(SingleConeBuildRequestError::InvalidCurrentCoreCombination),
    }?;
    validate_output_isolation(current, dependencies, trusted_core, output)
}

#[cfg(test)]
mod tests;
