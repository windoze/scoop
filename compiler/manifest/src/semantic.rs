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
    TrustedCoreHasDependencies,
    InvalidConeKind(String),
    InvalidDependencyKey(String),
    InvalidDependencyCoordinate {
        key: String,
        source: ConeCoordinateError,
    },
    ReservedCoreDependency,
    ConflictingDependencyLocators,
    EmptyDependencyLocator,
}

impl fmt::Display for ManifestParseErrorKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidToml(message) => write!(formatter, "invalid Cone.toml: {message}"),
            Self::UnsupportedSchema(schema) => {
                write!(formatter, "unsupported Cone.toml schema {schema}; expected 1")
            }
            Self::InvalidConeCoordinate(error) => error.fmt(formatter),
            Self::ReservedConeCoordinate => {
                formatter.write_str("user Cone.toml must not declare a reserved Cone coordinate")
            }
            Self::TrustedCoreMustBeLibrary => {
                formatter.write_str("trusted core Cone.toml must declare kind = \"library\"")
            }
            Self::TrustedCoreHasDependencies => {
                formatter.write_str("trusted core Cone.toml must not declare dependencies")
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
            Self::ReservedCoreDependency => formatter.write_str(
                "Cone.toml must not declare scoop:scoop.core:0.1.0; core is injected by the typed request",
            ),
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

    if coordinate == ConeCoordinate::reserved_core() {
        if let Some(dependency) = raw.dependencies.values().next() {
            return Err(ManifestParseError::new(
                ManifestParseErrorKind::TrustedCoreHasDependencies,
                Some(dependency.span()),
            ));
        }
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
        if dependency_coordinate == ConeCoordinate::reserved_core() {
            return Err(ManifestParseError::new(
                ManifestParseErrorKind::ReservedCoreDependency,
                Some(declaration_span),
            ));
        }

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
mod tests {
    use super::*;

    const MINIMAL: &str = r#"
schema = 1

[cone]
group = "dev.example"
name = "sample"
version = "0.1.0"
kind = "library"
"#;

    #[test]
    fn parses_minimal_manifest_and_preserves_field_spans() {
        let parsed = parse_cone_manifest(MINIMAL).unwrap();

        assert_eq!(
            parsed.semantic().coordinate(),
            &ConeCoordinate::new("dev.example", "sample", "0.1.0").unwrap()
        );
        assert_eq!(
            parsed.semantic().requested_kind(),
            RequestedConeKind::Library
        );
        assert!(parsed.semantic().is_dependency_free());
        assert!(parsed.locators().is_empty());
        assert_eq!(
            &MINIMAL[parsed.diagnostic_spans().cone_fields().name().range()],
            "\"sample\""
        );
    }

    #[test]
    fn separates_dependency_semantics_from_locators() {
        let source = format!(
            "{MINIMAL}\n[dependencies]\n\
             \"org.foo:bar\" = {{ version = \"1.2.3\", path = \"../bar\" }}\n\
             \"org.acme:util\" = {{ version = \"2.0.0\", artifact = \"../util.slib\" }}\n\
             \"org.other:log\" = \"3.1.0\"\n"
        );
        let parsed = parse_cone_manifest(&source).unwrap();
        let keys = parsed
            .semantic()
            .dependency_iter()
            .map(|(key, _)| key.to_string())
            .collect::<Vec<_>>();

        assert_eq!(keys, ["org.acme:util", "org.foo:bar", "org.other:log"]);
        assert_eq!(parsed.semantic().dependency_count(), 3);
        assert_eq!(
            parsed
                .semantic()
                .dependency_iter()
                .map(|(key, _)| key.to_string())
                .collect::<Vec<_>>(),
            keys
        );
        for (key, _) in parsed.semantic().dependency_iter() {
            match key.to_string().as_str() {
                "org.foo:bar" => {
                    let locator = parsed.locators().get(key).unwrap();
                    assert_eq!(
                        locator.source_path().unwrap().as_path(),
                        Path::new("../bar")
                    );
                    assert!(locator.artifact_path().is_none());
                    let spans = parsed.diagnostic_spans().dependency(key).unwrap();
                    assert_eq!(&source[spans.path().unwrap().range()], "\"../bar\"");
                }
                "org.acme:util" => assert!(matches!(
                    parsed.locators().get(key).unwrap().artifact_path(),
                    Some(path) if path.as_path() == Path::new("../util.slib")
                )),
                "org.other:log" => {
                    assert!(parsed.locators().get(key).unwrap().uses_search_roots());
                }
                other => panic!("unexpected dependency {other}"),
            }
            let spans = parsed.diagnostic_spans().dependency(key).unwrap();
            assert_eq!(
                &source[spans.version().range()],
                match key.to_string().as_str() {
                    "org.acme:util" => "\"2.0.0\"",
                    "org.foo:bar" => "\"1.2.3\"",
                    "org.other:log" => "\"3.1.0\"",
                    _ => unreachable!("dependency keys were checked above"),
                }
            );
        }
        assert_eq!(parsed.diagnostic_spans().dependency_iter().count(), 3);
    }

    #[test]
    fn rejects_noncanonical_and_reserved_semantics() {
        let cases = [
            (
                MINIMAL.replace("0.1.0", "0.1.00"),
                ManifestParseErrorKind::InvalidConeCoordinate(
                    ConeCoordinateError::InvalidSemanticVersion,
                ),
            ),
            (
                MINIMAL
                    .replace("dev.example", "scoop")
                    .replace("sample", "single-file")
                    .replace("0.1.0", "0.0.0"),
                ManifestParseErrorKind::ReservedConeCoordinate,
            ),
        ];

        for (source, expected) in cases {
            assert_eq!(*parse_cone_manifest(&source).unwrap_err().kind(), expected);
        }
    }

    #[test]
    fn rejects_unknown_duplicate_and_wrong_typed_fields() {
        for source in [
            MINIMAL.replace("schema = 1", "schema = 1\nunknown = true"),
            MINIMAL.replace("name = \"sample\"", "name = \"sample\"\nname = \"again\""),
            MINIMAL.replace("schema = 1", "schema = \"1\""),
        ] {
            assert!(matches!(
                parse_cone_manifest(&source).unwrap_err().kind(),
                ManifestParseErrorKind::InvalidToml(_)
            ));
        }
    }

    #[test]
    fn rejects_invalid_dependency_shapes() {
        let cases = [
            (
                "\"bad:key:extra\" = \"1.0.0\"",
                ManifestParseErrorKind::InvalidDependencyKey("bad:key:extra".to_owned()),
            ),
            (
                "\"scoop:scoop.core\" = \"0.1.0\"",
                ManifestParseErrorKind::ReservedCoreDependency,
            ),
            (
                "\"org.foo:bar\" = { version = \"1.0.0\", path = \"a\", artifact = \"b\" }",
                ManifestParseErrorKind::ConflictingDependencyLocators,
            ),
            (
                "\"org.foo:bar\" = { version = \"1.0.0\", path = \"\" }",
                ManifestParseErrorKind::EmptyDependencyLocator,
            ),
        ];

        for (dependency, expected) in cases {
            let source = format!("{MINIMAL}\n[dependencies]\n{dependency}\n");
            assert_eq!(*parse_cone_manifest(&source).unwrap_err().kind(), expected);
        }
    }

    #[test]
    fn rejects_unknown_dependency_field() {
        let source = format!(
            "{MINIMAL}\n[dependencies]\n\
             \"org.foo:bar\" = {{ version = \"1.0.0\", branch = \"main\" }}\n"
        );
        assert!(matches!(
            parse_cone_manifest(&source).unwrap_err().kind(),
            ManifestParseErrorKind::InvalidToml(_)
        ));
    }

    #[test]
    fn core_uses_the_common_manifest_parser() {
        let core = MINIMAL
            .replace("dev.example", "scoop")
            .replace("sample", "scoop.core");
        let parsed = parse_cone_manifest(&core).unwrap();
        assert_eq!(
            parsed.semantic().coordinate(),
            &ConeCoordinate::reserved_core()
        );
        assert_eq!(
            parsed.semantic().requested_kind(),
            RequestedConeKind::Library
        );
        assert!(parsed.semantic().is_dependency_free());
        let executable = core.replace("kind = \"library\"", "kind = \"executable\"");
        assert_eq!(
            *parse_cone_manifest(&executable).unwrap_err().kind(),
            ManifestParseErrorKind::TrustedCoreMustBeLibrary
        );

        let dependency = format!("{core}\n[dependencies]\n\"dev.example:dep\" = \"1.0.0\"\n");
        assert_eq!(
            *parse_cone_manifest(&dependency).unwrap_err().kind(),
            ManifestParseErrorKind::TrustedCoreHasDependencies
        );
    }
}
