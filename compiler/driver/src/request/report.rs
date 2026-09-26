use std::fmt;

use scoop_ast::{CurrentConeParsedSources, Diagnostic};
use scoop_slib::PublishedCrossConeArtifact;

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

#[derive(Debug)]
pub struct SingleConeProductionSuccess {
    artifact: PublishedCrossConeArtifact,
    warnings: CurrentConeDiagnosticSet,
    emitted_dump: Option<EmittedStageDump>,
}

impl SingleConeProductionSuccess {
    pub(super) fn new_cross_cone(
        artifact: PublishedCrossConeArtifact,
        warnings: CurrentConeDiagnosticSet,
        emitted_dump: Option<EmittedStageDump>,
    ) -> Self {
        Self {
            artifact,
            warnings,
            emitted_dump,
        }
    }

    pub const fn artifact(&self) -> &PublishedCrossConeArtifact {
        &self.artifact
    }

    pub const fn warnings(&self) -> &CurrentConeDiagnosticSet {
        &self.warnings
    }

    pub const fn emitted_dump(&self) -> Option<&EmittedStageDump> {
        self.emitted_dump.as_ref()
    }
}
