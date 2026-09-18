use std::fmt;

use scoop_ast::{CurrentConeParsedSources, Diagnostic};
use scoop_identity::ArtifactCapabilityProfileId;
use scoop_slib::{
    ArtifactFingerprint, CompileViewSummaryV1, ConeKind, ConeSourceForm, DependencyRecord,
    LinkViewSummaryV1, PublishableCrossConeArtifact, PublishableSingleConeArtifact,
    PublishedCrossConeArtifact, PublishedSingleConeArtifact,
};

use super::StageDumpKind;

#[derive(Debug)]
pub struct EmittedStageDump {
    kind: StageDumpKind,
    text: String,
}

impl EmittedStageDump {
    pub(super) fn new(kind: StageDumpKind, text: String) -> Self {
        Self { kind, text }
    }

    pub const fn kind(&self) -> StageDumpKind {
        self.kind
    }

    pub fn text(&self) -> &str {
        &self.text
    }
}

#[derive(Debug)]
struct CurrentConeDiagnosticSource {
    display_locator: String,
    source_text: String,
}

/// Diagnostics inseparably paired with the exact current-Cone sources whose
/// canonical indices they reference.
#[derive(Debug)]
pub struct CurrentConeDiagnosticSet {
    diagnostics: Vec<Diagnostic>,
    sources: Vec<CurrentConeDiagnosticSource>,
}

impl CurrentConeDiagnosticSet {
    pub(super) fn try_new(
        diagnostics: Vec<Diagnostic>,
        sources: &CurrentConeParsedSources,
    ) -> Result<Self, CurrentConeDiagnosticSetError> {
        let sources = sources
            .iter()
            .map(|source| CurrentConeDiagnosticSource {
                display_locator: source.diagnostic().display_locator().display().to_string(),
                source_text: source.text().text().to_owned(),
            })
            .collect::<Vec<_>>();
        for (diagnostic_index, diagnostic) in diagnostics.iter().enumerate() {
            if diagnostic.file >= sources.len() {
                return Err(CurrentConeDiagnosticSetError::PrimarySourceIndex {
                    diagnostic_index,
                    source_index: diagnostic.file,
                    source_count: sources.len(),
                });
            }
            for (note_index, note) in diagnostic.notes.iter().enumerate() {
                if note.file >= sources.len() {
                    return Err(CurrentConeDiagnosticSetError::NoteSourceIndex {
                        diagnostic_index,
                        note_index,
                        source_index: note.file,
                        source_count: sources.len(),
                    });
                }
            }
        }
        Ok(Self {
            diagnostics,
            sources,
        })
    }

    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    pub fn is_empty(&self) -> bool {
        self.diagnostics.is_empty()
    }

    pub fn render_human(&self) -> String {
        self.diagnostics
            .iter()
            .flat_map(|diagnostic| {
                let primary_source = &self.sources[diagnostic.file];
                let primary =
                    diagnostic.render(&primary_source.display_locator, &primary_source.source_text);
                std::iter::once(primary).chain(diagnostic.notes.iter().map(|note| {
                    let note_source = &self.sources[note.file];
                    note.render(&note_source.display_locator, &note_source.source_text)
                }))
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CurrentConeDiagnosticSetError {
    PrimarySourceIndex {
        diagnostic_index: usize,
        source_index: usize,
        source_count: usize,
    },
    NoteSourceIndex {
        diagnostic_index: usize,
        note_index: usize,
        source_index: usize,
        source_count: usize,
    },
}

impl fmt::Display for CurrentConeDiagnosticSetError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PrimarySourceIndex {
                diagnostic_index,
                source_index,
                source_count,
            } => write!(
                formatter,
                "diagnostic {diagnostic_index} references source {source_index}, but the current Cone has {source_count} sources"
            ),
            Self::NoteSourceIndex {
                diagnostic_index,
                note_index,
                source_index,
                source_count,
            } => write!(
                formatter,
                "diagnostic {diagnostic_index} note {note_index} references source {source_index}, but the current Cone has {source_count} sources"
            ),
        }
    }
}

impl std::error::Error for CurrentConeDiagnosticSetError {}

/// Published artifact during the profile migration. Both variants expose the
/// same immutable publication summary; new production must select CrossCone.
#[derive(Debug)]
pub enum PublishedStrongArtifact {
    LegacySingleCone(PublishedSingleConeArtifact),
    CrossCone(PublishedCrossConeArtifact),
}

impl PublishedStrongArtifact {
    pub fn path(&self) -> &std::path::Path {
        match self {
            Self::LegacySingleCone(artifact) => artifact.path(),
            Self::CrossCone(artifact) => artifact.path(),
        }
    }

    pub const fn validation(&self) -> PublishedStrongArtifactValidation<'_> {
        match self {
            Self::LegacySingleCone(artifact) => {
                PublishedStrongArtifactValidation::LegacySingleCone(artifact.validation())
            }
            Self::CrossCone(artifact) => {
                PublishedStrongArtifactValidation::CrossCone(artifact.validation())
            }
        }
    }
}

/// Borrowed common view over legacy and cross-Cone publication proofs.
#[derive(Clone, Copy, Debug)]
pub enum PublishedStrongArtifactValidation<'a> {
    LegacySingleCone(&'a PublishableSingleConeArtifact),
    CrossCone(&'a PublishableCrossConeArtifact),
}

impl<'a> PublishedStrongArtifactValidation<'a> {
    pub const fn artifact_fingerprint(self) -> ArtifactFingerprint {
        match self {
            Self::LegacySingleCone(artifact) => artifact.artifact_fingerprint(),
            Self::CrossCone(artifact) => artifact.artifact_fingerprint(),
        }
    }

    pub const fn coordinate(self) -> &'a scoop_identity::ConeCoordinate {
        match self {
            Self::LegacySingleCone(artifact) => artifact.coordinate(),
            Self::CrossCone(artifact) => artifact.coordinate(),
        }
    }

    pub const fn identity(self) -> scoop_identity::ConeIdentity {
        match self {
            Self::LegacySingleCone(artifact) => artifact.identity(),
            Self::CrossCone(artifact) => artifact.identity(),
        }
    }

    pub const fn kind(self) -> ConeKind {
        match self {
            Self::LegacySingleCone(artifact) => artifact.kind(),
            Self::CrossCone(artifact) => artifact.kind(),
        }
    }

    pub const fn source_form(self) -> ConeSourceForm {
        match self {
            Self::LegacySingleCone(artifact) => artifact.source_form(),
            Self::CrossCone(artifact) => artifact.source_form(),
        }
    }

    pub const fn profile(self) -> &'a ArtifactCapabilityProfileId {
        match self {
            Self::LegacySingleCone(artifact) => artifact.profile(),
            Self::CrossCone(artifact) => artifact.profile(),
        }
    }

    pub const fn compile_summary(self) -> CompileViewSummaryV1 {
        match self {
            Self::LegacySingleCone(artifact) => artifact.compile_summary(),
            Self::CrossCone(artifact) => artifact.compile_summary(),
        }
    }

    pub const fn link_summary(self) -> &'a LinkViewSummaryV1 {
        match self {
            Self::LegacySingleCone(artifact) => artifact.link_summary(),
            Self::CrossCone(artifact) => artifact.link_summary(),
        }
    }

    pub fn direct_dependencies(self) -> &'a [DependencyRecord] {
        match self {
            Self::LegacySingleCone(artifact) => artifact.direct_dependencies(),
            Self::CrossCone(artifact) => artifact.direct_dependencies(),
        }
    }

    pub fn dependency_record(self) -> DependencyRecord {
        match self {
            Self::LegacySingleCone(artifact) => artifact.dependency_record(),
            Self::CrossCone(artifact) => artifact.dependency_record(),
        }
    }
}

#[derive(Debug)]
pub struct SingleConeProductionSuccess {
    artifact: PublishedStrongArtifact,
    warnings: CurrentConeDiagnosticSet,
    emitted_dump: Option<EmittedStageDump>,
}

impl SingleConeProductionSuccess {
    pub(super) fn new(
        artifact: PublishedSingleConeArtifact,
        warnings: CurrentConeDiagnosticSet,
        emitted_dump: Option<EmittedStageDump>,
    ) -> Self {
        Self {
            artifact: PublishedStrongArtifact::LegacySingleCone(artifact),
            warnings,
            emitted_dump,
        }
    }

    pub(super) fn new_cross_cone(
        artifact: PublishedCrossConeArtifact,
        warnings: CurrentConeDiagnosticSet,
        emitted_dump: Option<EmittedStageDump>,
    ) -> Self {
        Self {
            artifact: PublishedStrongArtifact::CrossCone(artifact),
            warnings,
            emitted_dump,
        }
    }

    pub const fn artifact(&self) -> &PublishedStrongArtifact {
        &self.artifact
    }

    pub const fn warnings(&self) -> &CurrentConeDiagnosticSet {
        &self.warnings
    }

    pub const fn emitted_dump(&self) -> Option<&EmittedStageDump> {
        self.emitted_dump.as_ref()
    }
}
