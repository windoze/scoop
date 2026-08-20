use std::fmt;
use std::ops::Range;
use std::path::{Path, PathBuf};

use scoop_manifest::{ManifestRootError, ManifestRootErrorKind};
use scoop_protocol::StructuredDiagnosticV1;
use scoop_slib::{ArtifactManifestSummaryError, SlibDiagnostic};

use crate::{
    BuildFailurePhase, BuildGraphDiscoveryError, BuildGraphExecutionFailure, ClassifyBuildFailure,
    DependencyLocatorError, LoadBuildRootError, PrepareBuildGraphError, SourceSnapshot,
};

#[derive(Debug)]
pub struct BuildFailure {
    pub message: String,
    pub code: String,
    pub phase: BuildFailurePhase,
    pub location: BuildFailureLocation,
    pub diagnostics: Vec<StructuredDiagnosticV1>,
    pub warnings: Vec<StructuredDiagnosticV1>,
    pub sources: Vec<SourceSnapshot>,
}

#[derive(Debug)]
pub enum BuildFailureLocation {
    None,
    Host {
        path: PathBuf,
        span: Option<Range<usize>>,
    },
    Artifact {
        path: PathBuf,
        member: String,
    },
}

impl BuildFailure {
    pub fn tool(code: &str, phase: BuildFailurePhase, message: impl fmt::Display) -> Box<Self> {
        Box::new(Self {
            message: message.to_string(),
            code: code.to_owned(),
            phase,
            location: BuildFailureLocation::None,
            diagnostics: Vec::new(),
            warnings: Vec::new(),
            sources: Vec::new(),
        })
    }

    pub fn at_artifact(mut self: Box<Self>, path: &Path, member: impl Into<String>) -> Box<Self> {
        self.location = BuildFailureLocation::Artifact {
            path: path.to_owned(),
            member: member.into(),
        };
        self
    }

    pub fn at_host(mut self: Box<Self>, path: &Path) -> Box<Self> {
        self.location = BuildFailureLocation::Host {
            path: path.to_owned(),
            span: None,
        };
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

    pub(super) fn root(error: LoadBuildRootError) -> Box<Self> {
        let LoadBuildRootError::RootManifest(manifest) = &error;
        let location = manifest_location(manifest);
        let mut failure = Self::classified(error);
        failure.location = location;
        failure
    }

    pub(super) fn discovery(error: BuildGraphDiscoveryError) -> Box<Self> {
        let location = match &error {
            BuildGraphDiscoveryError::Locator(error) => locator_location(error),
            _ => BuildFailureLocation::None,
        };
        let mut failure = Self::classified(error);
        failure.location = location;
        failure
    }

    pub(super) fn preparation(error: PrepareBuildGraphError) -> Box<Self> {
        let path: Option<&Path> = match &error {
            PrepareBuildGraphError::SourceDiscovery(error) => Some(error.path()),
            PrepareBuildGraphError::SingleFile(error) => Some(error.path()),
            PrepareBuildGraphError::ManifestChanged(path) => Some(path),
            _ => None,
        };
        let path = path.map(Path::to_owned);
        let failure = Self::classified(error);
        match path {
            Some(path) => failure.at_host(&path),
            None => failure,
        }
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

fn manifest_location(error: &ManifestRootError) -> BuildFailureLocation {
    BuildFailureLocation::Host {
        path: error.path().to_owned(),
        span: match error.kind() {
            ManifestRootErrorKind::Parse(error) => error.span().map(|span| span.range()),
            _ => None,
        },
    }
}

fn locator_location(error: &DependencyLocatorError) -> BuildFailureLocation {
    use DependencyLocatorError as Error;
    match error {
        Error::Manifest(error) => manifest_location(error),
        Error::Summary { path, source } => BuildFailureLocation::Artifact {
            path: path.clone(),
            member: match source {
                ArtifactManifestSummaryError::Envelope(error) => error.diagnostic().semantic_path(),
                ArtifactManifestSummaryError::Graph(error) => error.diagnostic().semantic_path(),
                ArtifactManifestSummaryError::LengthOverflow => "container:$".to_owned(),
            },
        },
        Error::InvalidArtifactSourceForm { path, .. } => BuildFailureLocation::Artifact {
            path: path.clone(),
            member: "manifest:cone.source_form".to_owned(),
        },
        Error::ReservedArtifact { path, .. } => BuildFailureLocation::Artifact {
            path: path.clone(),
            member: "manifest:cone.identity".to_owned(),
        },
        Error::Io { path, .. }
        | Error::ArtifactNotRegularFile(path)
        | Error::LengthOverflow(path)
        | Error::ArtifactChangedDuringRead(path)
        | Error::Allocation(path)
        | Error::CoordinateMismatch { path, .. }
        | Error::ExecutableDependency { path, .. }
        | Error::SelfSourceLocator { root: path, .. } => BuildFailureLocation::Host {
            path: path.clone(),
            span: None,
        },
        Error::UndeclaredDependency(_)
        | Error::MissingLocatorProjection(_)
        | Error::ArtifactNotFound { .. }
        | Error::AmbiguousArtifact { .. } => BuildFailureLocation::None,
    }
}

impl fmt::Display for BuildFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}
impl std::error::Error for BuildFailure {}

pub type BuildResult<T> = Result<T, Box<BuildFailure>>;
