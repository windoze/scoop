use std::fmt;
use std::path::{Path, PathBuf};

use scoop_identity::{ConeIdentity, SourceContentDigest, SourceIdentity};

use super::{AllParsedSources, IdentifiedParsedSource};
use crate::NonEmptyVec;

/// Owned UTF-8 text and content identity for one current-Cone source.
///
/// The semantic source identity is repeated so the enclosing checked table can
/// prove exact coverage without relying on an external index or source order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurrentSourceText {
    identity: SourceIdentity,
    text: String,
    content_digest: SourceContentDigest,
}

impl CurrentSourceText {
    pub fn new(identity: SourceIdentity, text: String) -> Self {
        let content_digest = SourceContentDigest::from_utf8(&text);
        Self {
            identity,
            text,
            content_digest,
        }
    }

    pub const fn identity(&self) -> &SourceIdentity {
        &self.identity
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub const fn content_digest(&self) -> SourceContentDigest {
        self.content_digest
    }
}

/// Request-local source label used only to decorate diagnostics.
///
/// Host paths deliberately remain separate from semantic source identities
/// and are never encoded into a persistent IR or artifact key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurrentSourceDiagnosticContext {
    identity: SourceIdentity,
    display_locator: PathBuf,
}

impl CurrentSourceDiagnosticContext {
    pub fn new(identity: SourceIdentity, display_locator: PathBuf) -> Self {
        Self {
            identity,
            display_locator,
        }
    }

    pub const fn identity(&self) -> &SourceIdentity {
        &self.identity
    }

    pub fn display_locator(&self) -> &Path {
        &self.display_locator
    }
}

/// Canonically ordered, complete text sidecar for current-Cone ASTs.
#[derive(Debug, Clone, PartialEq)]
pub struct CurrentSourceTextTable {
    entries: NonEmptyVec<CurrentSourceText>,
}

impl CurrentSourceTextTable {
    pub const fn entries(&self) -> &NonEmptyVec<CurrentSourceText> {
        &self.entries
    }

    pub fn get(&self, identity: &SourceIdentity) -> Option<&CurrentSourceText> {
        self.entries
            .as_slice()
            .binary_search_by(|entry| entry.identity().cmp(identity))
            .ok()
            .map(|index| &self.entries.as_slice()[index])
    }
}

/// Canonically ordered, complete diagnostic sidecar for current-Cone ASTs.
#[derive(Debug, Clone, PartialEq)]
pub struct CurrentDiagnosticContext {
    entries: NonEmptyVec<CurrentSourceDiagnosticContext>,
}

impl CurrentDiagnosticContext {
    pub const fn entries(&self) -> &NonEmptyVec<CurrentSourceDiagnosticContext> {
        &self.entries
    }

    pub fn display_locator(&self, identity: &SourceIdentity) -> Option<&Path> {
        self.entries
            .as_slice()
            .binary_search_by(|entry| entry.identity().cmp(identity))
            .ok()
            .map(|index| self.entries.as_slice()[index].display_locator())
    }
}

/// Atomic parser output for exactly one current Cone.
///
/// A value proves that every source parsed, all identities belong to `cone`,
/// source order is canonical, and both request-local sidecars cover exactly
/// the same identities. Fields are private so later stages cannot observe a
/// partially parsed or partially decorated source set.
#[derive(Debug, Clone, PartialEq)]
pub struct CurrentConeParsedSources {
    cone: ConeIdentity,
    sources: AllParsedSources,
    source_texts: CurrentSourceTextTable,
    diagnostic_context: CurrentDiagnosticContext,
}

impl CurrentConeParsedSources {
    pub fn try_new(
        sources: AllParsedSources,
        source_texts: NonEmptyVec<CurrentSourceText>,
        diagnostic_context: NonEmptyVec<CurrentSourceDiagnosticContext>,
    ) -> Result<Self, CurrentConeParsedSourcesError> {
        let cone = sources.sources().first().identity().cone();
        validate_current_source_order(cone, sources.sources())?;
        validate_sidecar_coverage(sources.sources(), &source_texts, &diagnostic_context)?;
        Ok(Self {
            cone,
            sources,
            source_texts: CurrentSourceTextTable {
                entries: source_texts,
            },
            diagnostic_context: CurrentDiagnosticContext {
                entries: diagnostic_context,
            },
        })
    }

    pub const fn cone(&self) -> ConeIdentity {
        self.cone
    }

    pub const fn sources(&self) -> &AllParsedSources {
        &self.sources
    }

    pub const fn source_texts(&self) -> &CurrentSourceTextTable {
        &self.source_texts
    }

    pub const fn diagnostic_context(&self) -> &CurrentDiagnosticContext {
        &self.diagnostic_context
    }

    /// Iterates the three checked tables as one structurally complete source
    /// view. Later stages never need to recover sidecar coverage through an
    /// optional identity lookup.
    pub fn iter(&self) -> impl ExactSizeIterator<Item = CurrentConeParsedSource<'_>> {
        self.sources
            .sources()
            .iter()
            .zip(self.source_texts.entries().iter())
            .zip(self.diagnostic_context.entries().iter())
            .map(|((source, text), diagnostic)| CurrentConeParsedSource {
                source,
                text,
                diagnostic,
            })
    }
}

#[derive(Clone, Copy)]
pub struct CurrentConeParsedSource<'a> {
    source: &'a IdentifiedParsedSource,
    text: &'a CurrentSourceText,
    diagnostic: &'a CurrentSourceDiagnosticContext,
}

impl<'a> CurrentConeParsedSource<'a> {
    pub const fn source(self) -> &'a IdentifiedParsedSource {
        self.source
    }

    pub const fn text(self) -> &'a CurrentSourceText {
        self.text
    }

    pub const fn diagnostic(self) -> &'a CurrentSourceDiagnosticContext {
        self.diagnostic
    }
}

fn validate_current_source_order(
    cone: ConeIdentity,
    sources: &NonEmptyVec<IdentifiedParsedSource>,
) -> Result<(), CurrentConeParsedSourcesError> {
    for (index, source) in sources.iter().enumerate() {
        if source.identity().cone() != cone {
            return Err(CurrentConeParsedSourcesError::MixedCone {
                index,
                expected: cone,
                actual: source.identity().cone(),
            });
        }
    }
    for (index, pair) in sources.as_slice().windows(2).enumerate() {
        if pair[0].identity() >= pair[1].identity() {
            return Err(CurrentConeParsedSourcesError::NonIncreasingSourceIdentity {
                first_index: index,
                second_index: index + 1,
                first: Box::new(pair[0].identity().clone()),
                second: Box::new(pair[1].identity().clone()),
            });
        }
    }
    Ok(())
}

fn validate_sidecar_coverage(
    sources: &NonEmptyVec<IdentifiedParsedSource>,
    source_texts: &NonEmptyVec<CurrentSourceText>,
    diagnostic_context: &NonEmptyVec<CurrentSourceDiagnosticContext>,
) -> Result<(), CurrentConeParsedSourcesError> {
    if source_texts.len() != sources.len() {
        return Err(CurrentConeParsedSourcesError::SourceTextCount {
            expected: sources.len(),
            actual: source_texts.len(),
        });
    }
    if diagnostic_context.len() != sources.len() {
        return Err(CurrentConeParsedSourcesError::DiagnosticContextCount {
            expected: sources.len(),
            actual: diagnostic_context.len(),
        });
    }
    for (index, ((source, text), diagnostic)) in sources
        .iter()
        .zip(source_texts.iter())
        .zip(diagnostic_context.iter())
        .enumerate()
    {
        if source.identity() != text.identity() {
            return Err(CurrentConeParsedSourcesError::SourceTextIdentity {
                index,
                expected: Box::new(source.identity().clone()),
                actual: Box::new(text.identity().clone()),
            });
        }
        if source.identity() != diagnostic.identity() {
            return Err(CurrentConeParsedSourcesError::DiagnosticContextIdentity {
                index,
                expected: Box::new(source.identity().clone()),
                actual: Box::new(diagnostic.identity().clone()),
            });
        }
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CurrentConeParsedSourcesError {
    MixedCone {
        index: usize,
        expected: ConeIdentity,
        actual: ConeIdentity,
    },
    NonIncreasingSourceIdentity {
        first_index: usize,
        second_index: usize,
        first: Box<SourceIdentity>,
        second: Box<SourceIdentity>,
    },
    SourceTextCount {
        expected: usize,
        actual: usize,
    },
    DiagnosticContextCount {
        expected: usize,
        actual: usize,
    },
    SourceTextIdentity {
        index: usize,
        expected: Box<SourceIdentity>,
        actual: Box<SourceIdentity>,
    },
    DiagnosticContextIdentity {
        index: usize,
        expected: Box<SourceIdentity>,
        actual: Box<SourceIdentity>,
    },
}

impl fmt::Display for CurrentConeParsedSourcesError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MixedCone {
                index,
                expected,
                actual,
            } => write!(
                formatter,
                "source {index} belongs to Cone {actual}, expected current Cone {expected}"
            ),
            Self::NonIncreasingSourceIdentity {
                first_index,
                second_index,
                first,
                second,
            } => write!(
                formatter,
                "source identities are not strictly increasing at indexes {first_index} and {second_index}: {}/{} then {}/{}",
                first.cone(),
                first.logical_path(),
                second.cone(),
                second.logical_path(),
            ),
            Self::SourceTextCount { expected, actual } => write!(
                formatter,
                "source text table must contain {expected} entries, found {actual}"
            ),
            Self::DiagnosticContextCount { expected, actual } => write!(
                formatter,
                "diagnostic context must contain {expected} entries, found {actual}"
            ),
            Self::SourceTextIdentity {
                index,
                expected,
                actual,
            } => write!(
                formatter,
                "source text {index} has identity {}/{}, expected {}/{}",
                actual.cone(),
                actual.logical_path(),
                expected.cone(),
                expected.logical_path(),
            ),
            Self::DiagnosticContextIdentity {
                index,
                expected,
                actual,
            } => write!(
                formatter,
                "diagnostic context {index} has identity {}/{}, expected {}/{}",
                actual.cone(),
                actual.logical_path(),
                expected.cone(),
                expected.logical_path(),
            ),
        }
    }
}

impl std::error::Error for CurrentConeParsedSourcesError {}
