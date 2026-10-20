use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DocumentationOutputDestination(PathBuf);

impl DocumentationOutputDestination {
    pub fn new(path: impl Into<PathBuf>) -> Result<Self, SingleConeBuildRequestError> {
        let path = path.into();
        if path.extension() != Some(std::ffi::OsStr::new("json")) {
            return Err(SingleConeBuildRequestError::InvalidDocumentationOutput { path });
        }
        Ok(Self(if path.is_absolute() {
            path
        } else {
            std::env::current_dir()
                .map_err(SingleConeBuildRequestError::CurrentDirectory)?
                .join(path)
        }))
    }

    pub fn as_path(&self) -> &Path {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SingleConeOutput {
    Artifact(SlibOutputDestination),
    Documentation(DocumentationOutputDestination),
    ArtifactWithDocumentation {
        artifact: SlibOutputDestination,
        documentation: DocumentationOutputDestination,
    },
}

impl From<SlibOutputDestination> for SingleConeOutput {
    fn from(output: SlibOutputDestination) -> Self {
        Self::Artifact(output)
    }
}

impl SingleConeOutput {
    pub fn artifact(&self) -> Option<&SlibOutputDestination> {
        match self {
            Self::Artifact(path) | Self::ArtifactWithDocumentation { artifact: path, .. } => {
                Some(path)
            }
            Self::Documentation(_) => None,
        }
    }

    pub fn documentation(&self) -> Option<&DocumentationOutputDestination> {
        match self {
            Self::Documentation(path)
            | Self::ArtifactWithDocumentation {
                documentation: path,
                ..
            } => Some(path),
            Self::Artifact(_) => None,
        }
    }

    pub(super) fn primary_path(&self) -> &Path {
        match self {
            Self::Artifact(path) | Self::ArtifactWithDocumentation { artifact: path, .. } => {
                path.as_path()
            }
            Self::Documentation(path) => path.as_path(),
        }
    }

    pub(super) fn validate(
        &self,
        current: &CurrentConeInput,
        purpose: CompilationPurpose,
        emit: StageDumpPolicy,
    ) -> Result<(), SingleConeBuildRequestError> {
        if self.documentation().is_some()
            && (matches!(current, CurrentConeInput::SingleFile { .. })
                || purpose != CompilationPurpose::Production)
        {
            return Err(SingleConeBuildRequestError::DocumentationRequiresProductionManifest);
        }
        if self.artifact().is_none()
            && let StageDumpPolicy::Stages(stages) = emit
            && (stages.contains(StageDumpKind::Mir) || stages.contains(StageDumpKind::Lir))
        {
            return Err(SingleConeBuildRequestError::DocumentationMachineDump);
        }
        Ok(())
    }
}

impl SingleConeBuildRequest {
    #[allow(clippy::too_many_arguments)]
    pub fn documentation(
        current: CurrentConeInput,
        dependencies: ExplicitDependencyInputs,
        trusted_core: TrustedCoreInput,
        target: scoop_lir::ValidatedLirTargetSelection,
        output: DocumentationOutputDestination,
        diagnostics: DiagnosticOutputPolicy,
        emit: StageDumpPolicy,
    ) -> Result<Self, SingleConeBuildRequestError> {
        Self::new_selected(
            current,
            dependencies,
            trusted_core,
            target::SelectedTarget::Frontend(target),
            SingleConeOutput::Documentation(output),
            diagnostics,
            emit,
        )
    }
}
