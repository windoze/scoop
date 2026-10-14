use super::*;

pub(super) fn parse_dependencies(
    raw: BTreeMap<String, Spanned<RawDependency>>,
    document: &toml_edit::ImDocument<String>,
    table: &str,
) -> Result<BTreeMap<DependencyCoordinateKey, ManifestDependency>, ManifestParseError> {
    let mut dependencies = BTreeMap::new();
    for (raw_key, raw_dependency) in raw {
        let declaration_span = raw_dependency.span();
        let dependency_item = document
            .get(table)
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

        dependencies.insert(
            dependency_key,
            ManifestDependency {
                coordinate: dependency_coordinate,
                locator,
                spans: DependencyManifestSpans {
                    declaration: ManifestSpan::new(declaration_span),
                    version: ManifestSpan::new(version_span),
                    path: path_span.map(ManifestSpan::new),
                    artifact: artifact_span.map(ManifestSpan::new),
                },
            },
        );
    }
    Ok(dependencies)
}
