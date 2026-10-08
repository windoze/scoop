use std::collections::BTreeMap;
use std::fmt;
use std::ops::Range;
use std::path::{Path, PathBuf};

use crate::selection::SourceSelection;
use scoop_identity::{ConeCoordinate, ConeCoordinateError, RequestedConeKind};

mod parse;
pub use parse::parse_cone_manifest;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct DependencyCoordinateKey {
    group: String,
    name: String,
}

impl DependencyCoordinateKey {
    pub fn group(&self) -> &str {
        &self.group
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

impl fmt::Display for DependencyCoordinateKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}:{}", self.group, self.name)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConeManifestSemantic {
    coordinate: ConeCoordinate,
    requested_kind: RequestedConeKind,
    dependencies: BTreeMap<DependencyCoordinateKey, ConeCoordinate>,
    sources: SourceSelection,
}

impl ConeManifestSemantic {
    pub fn sources(&self) -> &SourceSelection {
        &self.sources
    }

    pub fn coordinate(&self) -> &ConeCoordinate {
        &self.coordinate
    }

    pub const fn requested_kind(&self) -> RequestedConeKind {
        self.requested_kind
    }

    /// Iterates exact dependency coordinates in canonical key order without
    /// exposing a mutable or representation-specific map API.
    pub fn dependency_iter(
        &self,
    ) -> impl ExactSizeIterator<Item = (&DependencyCoordinateKey, &ConeCoordinate)> {
        self.dependencies.iter()
    }

    pub fn dependency_count(&self) -> usize {
        self.dependencies.len()
    }

    pub fn is_dependency_free(&self) -> bool {
        self.dependencies.is_empty()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostPathLocator(PathBuf);

impl HostPathLocator {
    fn from_manifest_text(value: String) -> Result<Self, ()> {
        if value.is_empty() {
            return Err(());
        }
        Ok(Self(PathBuf::from(value)))
    }

    pub fn as_path(&self) -> &Path {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DependencyLocator {
    SearchRoots,
    SourcePath(HostPathLocator),
    ArtifactPath(HostPathLocator),
}

impl DependencyLocator {
    pub const fn uses_search_roots(&self) -> bool {
        matches!(self, Self::SearchRoots)
    }

    pub const fn source_path(&self) -> Option<&HostPathLocator> {
        match self {
            Self::SourcePath(path) => Some(path),
            Self::SearchRoots | Self::ArtifactPath(_) => None,
        }
    }

    pub const fn artifact_path(&self) -> Option<&HostPathLocator> {
        match self {
            Self::ArtifactPath(path) => Some(path),
            Self::SearchRoots | Self::SourcePath(_) => None,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DependencyLocatorTable(BTreeMap<DependencyCoordinateKey, DependencyLocator>);

impl DependencyLocatorTable {
    pub fn get(&self, key: &DependencyCoordinateKey) -> Option<&DependencyLocator> {
        self.0.get(key)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&DependencyCoordinateKey, &DependencyLocator)> {
        self.0.iter()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManifestSpan(Range<usize>);

impl ManifestSpan {
    fn new(span: Range<usize>) -> Self {
        Self(span)
    }

    pub fn range(&self) -> Range<usize> {
        self.0.clone()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConeManifestSpans {
    group: ManifestSpan,
    name: ManifestSpan,
    version: ManifestSpan,
    kind: ManifestSpan,
}

impl ConeManifestSpans {
    pub fn group(&self) -> &ManifestSpan {
        &self.group
    }

    pub fn name(&self) -> &ManifestSpan {
        &self.name
    }

    pub fn version(&self) -> &ManifestSpan {
        &self.version
    }

    pub fn kind(&self) -> &ManifestSpan {
        &self.kind
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DependencyManifestSpans {
    declaration: ManifestSpan,
    version: ManifestSpan,
    path: Option<ManifestSpan>,
    artifact: Option<ManifestSpan>,
}

impl DependencyManifestSpans {
    pub fn declaration(&self) -> &ManifestSpan {
        &self.declaration
    }

    pub fn version(&self) -> &ManifestSpan {
        &self.version
    }

    pub fn path(&self) -> Option<&ManifestSpan> {
        self.path.as_ref()
    }

    pub fn artifact(&self) -> Option<&ManifestSpan> {
        self.artifact.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManifestDiagnosticSpans {
    schema: ManifestSpan,
    cone: ManifestSpan,
    cone_fields: ConeManifestSpans,
    dependencies: BTreeMap<DependencyCoordinateKey, DependencyManifestSpans>,
}

impl ManifestDiagnosticSpans {
    pub fn schema(&self) -> &ManifestSpan {
        &self.schema
    }

    pub fn cone(&self) -> &ManifestSpan {
        &self.cone
    }

    pub fn cone_fields(&self) -> &ConeManifestSpans {
        &self.cone_fields
    }

    pub fn dependency(&self, key: &DependencyCoordinateKey) -> Option<&DependencyManifestSpans> {
        self.dependencies.get(key)
    }

    pub fn dependency_iter(
        &self,
    ) -> impl ExactSizeIterator<Item = (&DependencyCoordinateKey, &DependencyManifestSpans)> {
        self.dependencies.iter()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParsedConeManifest {
    semantic: ConeManifestSemantic,
    locators: DependencyLocatorTable,
    diagnostic_spans: ManifestDiagnosticSpans,
}

impl ParsedConeManifest {
    pub fn semantic(&self) -> &ConeManifestSemantic {
        &self.semantic
    }

    pub fn locators(&self) -> &DependencyLocatorTable {
        &self.locators
    }

    pub fn diagnostic_spans(&self) -> &ManifestDiagnosticSpans {
        &self.diagnostic_spans
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManifestParseError {
    kind: ManifestParseErrorKind,
    span: Option<ManifestSpan>,
}

impl ManifestParseError {
    pub(crate) fn new(kind: ManifestParseErrorKind, span: Option<Range<usize>>) -> Self {
        Self {
            kind,
            span: span.map(ManifestSpan::new),
        }
    }

    pub fn kind(&self) -> &ManifestParseErrorKind {
        &self.kind
    }

    pub fn span(&self) -> Option<&ManifestSpan> {
        self.span.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ManifestParseErrorKind {
    InvalidToml(String),
    UnsupportedSchema(i64),
    InvalidConeCoordinate(ConeCoordinateError),
    ReservedConeCoordinate,
    TrustedCoreMustBeLibrary,
    InvalidConeKind(String),
    InvalidDependencyKey(String),
    InvalidDependencyCoordinate {
        key: String,
        source: ConeCoordinateError,
    },
    ConflictingDependencyLocators,
    EmptyDependencyLocator,
    InvalidSelection(String),
}

impl fmt::Display for ManifestParseErrorKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSelection(message) => formatter.write_str(message),
            Self::InvalidToml(message) => write!(formatter, "invalid Cone.toml: {message}"),
            Self::UnsupportedSchema(schema) => {
                write!(
                    formatter,
                    "unsupported Cone.toml schema {schema}; expected 1"
                )
            }
            Self::InvalidConeCoordinate(error) => error.fmt(formatter),
            Self::ReservedConeCoordinate => {
                formatter.write_str("user Cone.toml must not declare a reserved Cone coordinate")
            }
            Self::TrustedCoreMustBeLibrary => {
                formatter.write_str("trusted core Cone.toml must declare kind = \"library\"")
            }
            Self::InvalidConeKind(kind) => write!(
                formatter,
                "invalid Cone kind {kind:?}; expected \"library\" or \"executable\""
            ),
            Self::InvalidDependencyKey(key) => write!(
                formatter,
                "invalid dependency key {key:?}; expected canonical group:name"
            ),
            Self::InvalidDependencyCoordinate { key, source } => {
                write!(formatter, "invalid dependency {key:?}: {source}")
            }
            Self::ConflictingDependencyLocators => {
                formatter.write_str("a dependency cannot specify both path and artifact")
            }
            Self::EmptyDependencyLocator => {
                formatter.write_str("a dependency locator must not be empty")
            }
        }
    }
}

impl fmt::Display for ManifestParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.kind.fmt(formatter)
    }
}

impl std::error::Error for ManifestParseError {}

#[cfg(test)]
mod tests;
