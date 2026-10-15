use scoop_identity::{ConeCoordinate, ConeIdentity};

use crate::CrossConeClosureGraphError;

mod section;
pub(super) use section::section;

#[derive(Debug)]
pub struct ProgramLinkReadError {
    message: String,
    artifact: Option<ConeIdentity>,
    semantic_path: String,
}

impl ProgramLinkReadError {
    pub const fn artifact(&self) -> Option<ConeIdentity> {
        self.artifact
    }

    pub fn semantic_path(&self) -> &str {
        &self.semantic_path
    }

    pub(super) fn at(mut self, artifact: ConeIdentity, path: impl Into<String>) -> Self {
        self.artifact = Some(artifact);
        self.semantic_path = path.into();
        self
    }

    pub(super) fn context(mut self, artifact: ConeIdentity, coordinate: &ConeCoordinate) -> Self {
        self.artifact = Some(artifact);
        self.message = format!("{coordinate}: {}", self.message);
        self
    }
}

impl std::fmt::Display for ProgramLinkReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for ProgramLinkReadError {}

pub(super) fn error(value: impl std::fmt::Display) -> ProgramLinkReadError {
    ProgramLinkReadError {
        message: value.to_string(),
        artifact: None,
        semantic_path: "Link:$".to_owned(),
    }
}

pub(super) fn graph(
    source: CrossConeClosureGraphError,
    root: ConeIdentity,
) -> ProgramLinkReadError {
    use CrossConeClosureGraphError::*;
    let (cone, path) = match &source {
        InvalidProviderKind { identity, .. } => (*identity, "manifest:cone.kind"),
        TargetMismatch { identity, .. } => (*identity, "manifest:compatibility"),
        DuplicateArtifact { identity } | UnreachableSupport { identity } => {
            (*identity, "manifest:cone")
        }
        CurrentArtifactPresent { current } => (*current, "manifest:cone"),
        CurrentArtifactIdentityMismatch { actual, .. } => (*actual, "manifest:cone.identity"),
        MissingDependencyArtifact { dependent, .. }
        | InvalidDependencyFirstOrder { dependent, .. }
        | StaleDependency { dependent, .. } => (*dependent, "manifest:direct_dependencies"),
        NonCanonicalDirectProviders { .. }
        | CurrentDirectSetMismatch { .. }
        | MultipleVersions { .. }
        | MissingDirectArtifact { .. } => (root, "manifest:direct_dependencies"),
    };
    error(source).at(cone, path)
}
