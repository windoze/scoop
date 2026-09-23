//! Complete validated request with independent source and protocol inputs.

use super::*;
use scoop_slib::SlibClosureDecodeMeterV1;

impl LoadedSingleConeBuildRequest {
    /// Validates the complete dependency closure before current-source loading.
    pub fn validate(
        &self,
    ) -> Result<ValidatedSingleConeBuildRequest<'_>, SingleConeDependencyValidationError> {
        self.validate_inner(None)
    }
}

#[derive(Debug)]
pub enum SingleConeDependencyValidationError {
    ExplicitDependencies(Box<ExplicitDependencyValidationError>),
}

impl fmt::Display for SingleConeDependencyValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ExplicitDependencies(source) => source.fmt(formatter),
        }
    }
}

impl std::error::Error for SingleConeDependencyValidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ExplicitDependencies(source) => Some(source.as_ref()),
        }
    }
}

/// Source form is independent of the current library's protocol role.
pub enum ValidatedCurrentConeInput<'input> {
    Manifest {
        manifest: &'input LoadedConeManifest,
    },
    SingleFile {
        source: &'input SingleFileLocator,
    },
}

/// Frontend roles imported once from the shared dependency metadata.
pub enum ValidatedCompilerProtocols {
    CurrentDeclarations,
    Imported(Box<scoop_hir::ImportedCoreInputs>),
}

pub struct ValidatedSingleConeBuildRequest<'input> {
    pub(super) request: &'input LoadedSingleConeBuildRequest,
    pub(super) current: ValidatedCurrentConeInput<'input>,
    pub(super) dependencies: ValidatedExplicitDependencyInputSet<'input>,
    pub(super) protocols: ValidatedCompilerProtocols,
}

impl<'input> ValidatedSingleConeBuildRequest<'input> {
    pub const fn current(&self) -> &ValidatedCurrentConeInput<'input> {
        &self.current
    }

    pub const fn protocols(&self) -> &ValidatedCompilerProtocols {
        &self.protocols
    }

    pub const fn dependencies(&self) -> &ValidatedExplicitDependencyInputSet<'input> {
        &self.dependencies
    }

    pub const fn target(&self) -> &scoop_toolchain::ResolvedTargetProfile {
        &self.request.target
    }

    pub const fn output(&self) -> &SlibOutputDestination {
        &self.request.output
    }

    pub const fn diagnostics(&self) -> DiagnosticOutputPolicy {
        self.request.diagnostics
    }

    pub const fn emit(&self) -> StageDumpPolicy {
        self.request.emit
    }

    pub fn parse_current_sources<'request>(
        &'request self,
    ) -> Result<ParsedSingleConeBuildRequest<'request, 'input>, CurrentConeSourceStageError> {
        let sources = match &self.current {
            ValidatedCurrentConeInput::Manifest { manifest } => parse_manifest_current(manifest)?,
            ValidatedCurrentConeInput::SingleFile { source } => parse_single_file_current(source)?,
        };
        Ok(ParsedSingleConeBuildRequest {
            request: self,
            sources,
        })
    }
}

impl LoadedSingleConeBuildRequest {
    pub(super) fn validate_inner(
        &self,
        meter: Option<&mut SlibClosureDecodeMeterV1>,
    ) -> Result<ValidatedSingleConeBuildRequest<'_>, SingleConeDependencyValidationError> {
        let (manifest, current_identity) = match &self.current {
            LoadedCurrentConeInput::Manifest { manifest } => {
                (Some(manifest.as_ref()), manifest.identity())
            }
            LoadedCurrentConeInput::SingleFile { .. } => (None, ConeIdentity::SINGLE_FILE),
        };
        let dependencies = self
            .dependencies
            .validate_inner(manifest, current_identity, &self.target, meter)
            .map_err(SingleConeDependencyValidationError::ExplicitDependencies)?;
        let protocols = if current_identity == ConeIdentity::CORE {
            ValidatedCompilerProtocols::CurrentDeclarations
        } else {
            let imported = dependencies
                .semantic()
                .import_compiler_protocols(ConeIdentity::CORE)
                .map_err(|source| {
                    SingleConeDependencyValidationError::ExplicitDependencies(Box::new(
                        ExplicitDependencyValidationError::CompilerProtocols(source),
                    ))
                })?;
            ValidatedCompilerProtocols::Imported(Box::new(imported))
        };
        let current = match &self.current {
            LoadedCurrentConeInput::Manifest { manifest } => {
                ValidatedCurrentConeInput::Manifest { manifest }
            }
            LoadedCurrentConeInput::SingleFile { source } => {
                ValidatedCurrentConeInput::SingleFile { source }
            }
        };
        Ok(ValidatedSingleConeBuildRequest {
            request: self,
            current,
            dependencies,
            protocols,
        })
    }
}
