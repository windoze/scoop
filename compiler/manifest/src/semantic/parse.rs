use super::*;
use serde::Deserialize;
use toml::Spanned;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawManifest {
    schema: Spanned<i64>,
    cone: Spanned<RawCone>,
    #[serde(default)]
    dependencies: BTreeMap<String, Spanned<RawDependency>>,
    sources: Option<Vec<crate::selection::RawConditionalPath>>,
    #[serde(default)]
    native: crate::native::RawNativeConfig,
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
            sources: crate::selection::parse_sources(raw.sources)?,
            native: raw.native.parse()?,
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
