use super::*;
use serde::Deserialize;
use toml::Spanned;

mod dependencies;
use dependencies::parse_dependencies;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawManifest {
    schema: Spanned<i64>,
    cone: Spanned<RawCone>,
    #[serde(default)]
    dependencies: BTreeMap<String, Spanned<RawDependency>>,
    sources: Option<Vec<crate::selection::RawConditionalPath>>,
    #[serde(default)]
    test: RawTestConfig,
    #[serde(default, rename = "test-dependencies")]
    test_dependencies: BTreeMap<String, Spanned<RawDependency>>,
    #[serde(default)]
    native: crate::native::RawNativeConfig,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTestConfig {
    sources: Option<Vec<crate::selection::RawConditionalPath>>,
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

pub fn parse_cone_manifest(
    source: &str,
    catalog: &[crate::SourceTarget<'_>],
) -> Result<ParsedConeManifest, ManifestParseError> {
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

    let declared_dependencies = parse_dependencies(raw.dependencies, &document, "dependencies")?;
    let test_dependencies =
        parse_dependencies(raw.test_dependencies, &document, "test-dependencies")?;
    for (key, dependency) in &test_dependencies {
        if declared_dependencies.contains_key(key) {
            return Err(ManifestParseError::new(
                ManifestParseErrorKind::DuplicateTestDependency(key.to_string()),
                Some(dependency.spans.declaration().range()),
            ));
        }
    }
    let mut dependencies = BTreeMap::new();
    let mut locators = BTreeMap::new();
    let mut dependency_spans = BTreeMap::new();
    for (key, dependency) in declared_dependencies {
        dependencies.insert(key.clone(), dependency.coordinate);
        locators.insert(key.clone(), dependency.locator);
        dependency_spans.insert(key, dependency.spans);
    }

    Ok(ParsedConeManifest {
        tests: TestManifestConfig {
            sources: crate::selection::parse_sources(raw.test.sources, catalog)?,
            dependencies: test_dependencies,
        },
        semantic: ConeManifestSemantic {
            coordinate,
            requested_kind,
            dependencies,
            sources: crate::selection::parse_sources(raw.sources, catalog)?,
            native: raw.native.parse(catalog)?,
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
