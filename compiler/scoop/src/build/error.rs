use std::fmt;
use std::path::{Path, PathBuf};

use scoop_protocol::StructuredDiagnosticV1;

use crate::{BuildFailurePhase, BuildGraphExecutionFailure, ClassifyBuildFailure, SourceSnapshot};

#[derive(Debug)]
pub struct BuildFailure {
    pub message: String,
    pub code: String,
    pub phase: BuildFailurePhase,
    pub artifact: Option<ArtifactFailureLocation>,
    pub diagnostics: Vec<StructuredDiagnosticV1>,
    pub warnings: Vec<StructuredDiagnosticV1>,
    pub sources: Vec<SourceSnapshot>,
}

#[derive(Debug)]
pub struct ArtifactFailureLocation {
    pub path: PathBuf,
    pub member: String,
}

impl BuildFailure {
    pub fn tool(code: &str, phase: BuildFailurePhase, message: impl fmt::Display) -> Box<Self> {
        Box::new(Self {
            message: message.to_string(),
            code: code.to_owned(),
            phase,
            artifact: None,
            diagnostics: Vec::new(),
            warnings: Vec::new(),
            sources: Vec::new(),
        })
    }

    pub fn at_artifact(mut self: Box<Self>, path: &Path, member: impl Into<String>) -> Box<Self> {
        self.artifact = Some(ArtifactFailureLocation {
            path: path.to_owned(),
            member: member.into(),
        });
        self
    }

    pub fn classified(error: impl fmt::Display + ClassifyBuildFailure) -> Box<Self> {
        let classification = error.classification();
        Self::tool(
            classification
                .code()
                .map_or("SCOOP_BUILD_FAILED", |code| code.as_str()),
            classification.phase(),
            error,
        )
    }

    pub(super) fn execution(
        error: BuildGraphExecutionFailure,
        sources: &[SourceSnapshot],
    ) -> Box<Self> {
        let diagnostics = error.child_diagnostics().unwrap_or_default().to_vec();
        let warnings = error.warnings().to_vec();
        let mut failure = Self::classified(error);
        failure.diagnostics = diagnostics;
        failure.warnings = warnings;
        failure.sources = sources.to_vec();
        failure
    }

    pub fn with_context(
        mut self: Box<Self>,
        warnings: &[StructuredDiagnosticV1],
        sources: &[SourceSnapshot],
    ) -> Box<Self> {
        self.warnings = warnings.to_vec();
        self.sources = sources.to_vec();
        self
    }
}

impl fmt::Display for BuildFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}
impl std::error::Error for BuildFailure {}

pub type BuildResult<T> = Result<T, Box<BuildFailure>>;
