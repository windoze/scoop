use scoop_identity::SourceIdentity;
use std::fmt;

mod protocol;
pub use protocol::DiagnosticMappingError;

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
    identity: SourceIdentity,
    display_locator: String,
    source_text: Option<String>,
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
        mut diagnostics: Vec<Diagnostic>,
        sources: &CurrentConeParsedSources,
    ) -> Result<Self, CurrentConeDiagnosticSetError> {
        let mut sources = sources
            .iter()
            .map(|source| CurrentConeDiagnosticSource {
                identity: source.text().identity().clone(),
                display_locator: source.diagnostic().display_locator().display().to_string(),
                source_text: Some(source.text().text().to_owned()),
            })
            .collect::<Vec<_>>();
        for diagnostic in &mut diagnostics {
            if let Some(identity) = &diagnostic.source {
                diagnostic.file = source_index(&mut sources, identity);
            }
            for note in &mut diagnostic.notes {
                if let Some(identity) = &note.source {
                    note.file = source_index(&mut sources, identity);
                }
            }
        }
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
                let primary = match &primary_source.source_text {
                    Some(text) => diagnostic.render(&primary_source.display_locator, text),
                    None => {
                        let severity = match diagnostic.severity {
                            scoop_ast::DiagnosticSeverity::Error => "error",
                            scoop_ast::DiagnosticSeverity::Warning => "warning",
                        };
                        render_without_text(
                            primary_source,
                            diagnostic.span,
                            severity,
                            &diagnostic.message,
                        )
                    }
                };
                std::iter::once(primary).chain(diagnostic.notes.iter().map(|note| {
                    let note_source = &self.sources[note.file];
                    match &note_source.source_text {
                        Some(text) => note.render(&note_source.display_locator, text),
                        None => {
                            render_without_text(note_source, Some(note.span), "note", &note.message)
                        }
                    }
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
    emitted_dumps: Vec<EmittedStageDump>,
}

impl SingleConeProductionSuccess {
    pub(super) fn new_cross_cone(
        artifact: PublishedCrossConeArtifact,
        warnings: CurrentConeDiagnosticSet,
        emitted_dumps: Vec<EmittedStageDump>,
    ) -> Self {
        Self {
            artifact,
            warnings,
            emitted_dumps,
        }
    }

    pub const fn artifact(&self) -> &PublishedCrossConeArtifact {
        &self.artifact
    }

    pub const fn warnings(&self) -> &CurrentConeDiagnosticSet {
        &self.warnings
    }

    pub fn emitted_dumps(&self) -> &[EmittedStageDump] {
        &self.emitted_dumps
    }
}

fn source_index(
    sources: &mut Vec<CurrentConeDiagnosticSource>,
    identity: &SourceIdentity,
) -> usize {
    if let Some(index) = sources
        .iter()
        .position(|source| &source.identity == identity)
    {
        return index;
    }
    let index = sources.len();
    sources.push(CurrentConeDiagnosticSource {
        identity: identity.clone(),
        display_locator: format!("{}/{}", identity.cone(), identity.logical_path()),
        source_text: None,
    });
    index
}

fn render_without_text(
    source: &CurrentConeDiagnosticSource,
    span: Option<scoop_ast::Span>,
    severity: &str,
    message: &str,
) -> String {
    let location = match span {
        Some(span) => format!(
            "{}:bytes {}..{}",
            source.display_locator, span.start, span.end
        ),
        None => source.display_locator.clone(),
    };
    format!("{location}: {severity}: {message}")
}
