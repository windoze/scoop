use std::collections::BTreeMap;
use std::fmt;
use std::ops::Range;
use std::path::{Path, PathBuf};

use scoop_identity::{ConeCoordinate, ConeCoordinateError, RequestedConeKind};
use serde::Deserialize;
use toml::Spanned;

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
}

impl ConeManifestSemantic {
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
    fn new(kind: ManifestParseErrorKind, span: Option<Range<usize>>) -> Self {
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
}

impl fmt::Display for ManifestParseErrorKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
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

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawManifest {
    schema: Spanned<i64>,
    cone: Spanned<RawCone>,
    #[serde(default)]
    dependencies: BTreeMap<String, Spanned<RawDependency>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCone {
    group: Spanned<String>,
    name: Spanned<String>,
    version: Spanned<String>,
    kind: Spanned<String>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum RawDependency {
    Version(String),
    Detailed(RawDependencyDetails),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawDependencyDetails {
    version: String,
    path: Option<String>,
    artifact: Option<String>,
}

pub fn parse_cone_manifest(source: &str) -> Result<ParsedConeManifest, ManifestParseError> {
    let document = source.parse::<toml_edit::ImDocument<String>>().map_err(
        |error: toml_edit::TomlError| {
            ManifestParseError::new(
                ManifestParseErrorKind::InvalidToml(error.to_string()),
                error.span(),
            )
        },
    )?;
    let raw: RawManifest = toml::from_str(source).map_err(|error: toml::de::Error| {
        ManifestParseError::new(
            ManifestParseErrorKind::InvalidToml(error.to_string()),
            error.span(),
        )
    })?;

    let schema_span = raw.schema.span();
    let schema = raw.schema.into_inner();
    if schema != 1 {
        return Err(ManifestParseError::new(
            ManifestParseErrorKind::UnsupportedSchema(schema),
            Some(schema_span),
        ));
    }

    let cone_span = raw.cone.span();
    let raw_cone = raw.cone.into_inner();
    let group_span = raw_cone.group.span();
    let name_span = raw_cone.name.span();
    let version_span = raw_cone.version.span();
    let kind_span = raw_cone.kind.span();
    let group = raw_cone.group.into_inner();
    let name = raw_cone.name.into_inner();
    let version = raw_cone.version.into_inner();
    let kind = raw_cone.kind.into_inner();

    let coordinate = ConeCoordinate::new(&group, &name, &version).map_err(|error| {
        ManifestParseError::new(
            ManifestParseErrorKind::InvalidConeCoordinate(error),
            Some(group_span.start..version_span.end),
        )
    })?;
    if coordinate == ConeCoordinate::reserved_single_file() {
        return Err(ManifestParseError::new(
            ManifestParseErrorKind::ReservedConeCoordinate,
            Some(group_span.start..version_span.end),
        ));
    }

    let requested_kind = match kind.as_str() {
        "library" => RequestedConeKind::Library,
        "executable" => RequestedConeKind::Executable,
        _ => {
            return Err(ManifestParseError::new(
                ManifestParseErrorKind::InvalidConeKind(kind),
                Some(kind_span),
            ));
        }
    };
    if coordinate == ConeCoordinate::reserved_core() && requested_kind != RequestedConeKind::Library
    {
        return Err(ManifestParseError::new(
            ManifestParseErrorKind::TrustedCoreMustBeLibrary,
            Some(kind_span),
        ));
    }

    let mut dependencies = BTreeMap::new();
    let mut locators = BTreeMap::new();
    let mut dependency_spans = BTreeMap::new();
    for (raw_key, raw_dependency) in raw.dependencies {
        let declaration_span = raw_dependency.span();
        let dependency_item = document
            .get("dependencies")
            .and_then(toml_edit::Item::as_table)
            .and_then(|dependencies| dependencies.get(&raw_key));
        let (group, name) = split_dependency_key(&raw_key).ok_or_else(|| {
            ManifestParseError::new(
                ManifestParseErrorKind::InvalidDependencyKey(raw_key.clone()),
                Some(declaration_span.clone()),
            )
        })?;
        let dependency_key = DependencyCoordinateKey {
            group: group.to_owned(),
            name: name.to_owned(),
        };

        let (version, version_span, path, artifact) = match raw_dependency.into_inner() {
            RawDependency::Version(version) => (version, declaration_span.clone(), None, None),
            RawDependency::Detailed(details) => {
                let version_span = dependency_item
                    .and_then(toml_edit::Item::as_value)
                    .and_then(toml_edit::Value::as_inline_table)
                    .and_then(|fields| fields.get("version"))
                    .and_then(toml_edit::Value::span)
                    .unwrap_or_else(|| declaration_span.clone());
                (
                    details.version,
                    version_span,
                    details.path,
                    details.artifact,
                )
            }
        };

        let dependency_coordinate =
            ConeCoordinate::new(group, name, &version).map_err(|error| {
                ManifestParseError::new(
                    ManifestParseErrorKind::InvalidDependencyCoordinate {
                        key: raw_key.clone(),
                        source: error,
                    },
                    Some(declaration_span.clone()),
                )
            })?;

        let inline_fields = dependency_item
            .and_then(toml_edit::Item::as_value)
            .and_then(toml_edit::Value::as_inline_table);
        let path_span = path.as_ref().map(|_| {
            inline_fields
                .and_then(|fields| fields.get("path"))
                .and_then(toml_edit::Value::span)
                .unwrap_or_else(|| declaration_span.clone())
        });
        let artifact_span = artifact.as_ref().map(|_| {
            inline_fields
                .and_then(|fields| fields.get("artifact"))
                .and_then(toml_edit::Value::span)
                .unwrap_or_else(|| declaration_span.clone())
        });
        let locator = match (path, artifact) {
            (None, None) => DependencyLocator::SearchRoots,
            (Some(_), Some(_)) => {
                return Err(ManifestParseError::new(
                    ManifestParseErrorKind::ConflictingDependencyLocators,
                    Some(declaration_span),
                ));
            }
            (Some(path), None) => DependencyLocator::SourcePath(
                HostPathLocator::from_manifest_text(path).map_err(|()| {
                    ManifestParseError::new(
                        ManifestParseErrorKind::EmptyDependencyLocator,
                        path_span.clone(),
                    )
                })?,
            ),
            (None, Some(artifact)) => DependencyLocator::ArtifactPath(
                HostPathLocator::from_manifest_text(artifact).map_err(|()| {
                    ManifestParseError::new(
                        ManifestParseErrorKind::EmptyDependencyLocator,
                        artifact_span.clone(),
                    )
                })?,
            ),
        };

        dependencies.insert(dependency_key.clone(), dependency_coordinate);
        locators.insert(dependency_key.clone(), locator);
        dependency_spans.insert(
            dependency_key,
            DependencyManifestSpans {
                declaration: ManifestSpan::new(declaration_span),
                version: ManifestSpan::new(version_span),
                path: path_span.map(ManifestSpan::new),
                artifact: artifact_span.map(ManifestSpan::new),
            },
        );
    }

    Ok(ParsedConeManifest {
        semantic: ConeManifestSemantic {
            coordinate,
            requested_kind,
            dependencies,
        },
        locators: DependencyLocatorTable(locators),
        diagnostic_spans: ManifestDiagnosticSpans {
            schema: ManifestSpan::new(schema_span),
            cone: ManifestSpan::new(cone_span),
            cone_fields: ConeManifestSpans {
                group: ManifestSpan::new(group_span),
                name: ManifestSpan::new(name_span),
                version: ManifestSpan::new(version_span),
                kind: ManifestSpan::new(kind_span),
            },
            dependencies: dependency_spans,
        },
    })
}

fn split_dependency_key(key: &str) -> Option<(&str, &str)> {
    let (group, name) = key.split_once(':')?;
    if group.is_empty() || name.is_empty() || name.contains(':') {
        return None;
    }
    Some((group, name))
}

#[cfg(test)]
mod tests;
