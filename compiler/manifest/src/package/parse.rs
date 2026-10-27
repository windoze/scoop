use toml::Spanned;

use super::*;
use raw::*;

pub fn parse_package_manifest(
    text: &str,
    catalog: &[crate::SourceTarget<'_>],
) -> Result<PackageDocument, ManifestParseError> {
    let raw: RawPackage = toml::from_str(text).map_err(|error: toml::de::Error| {
        ManifestParseError::new(
            ManifestParseErrorKind::InvalidToml(error.to_string()),
            error.span(),
        )
    })?;
    if *raw.schema.get_ref() != 1 {
        return Err(ManifestParseError::new(
            ManifestParseErrorKind::UnsupportedSchema(*raw.schema.get_ref()),
            Some(raw.schema.span()),
        ));
    }
    if raw.cone.is_none() && raw.workspace.is_none() {
        return Err(invalid(
            "manifest requires cone or workspace",
            raw.schema.span(),
        ));
    }
    if raw.cone.is_none()
        && (!raw.dependencies.is_empty()
            || !raw.test_dependencies.is_empty()
            || raw.sources.is_some()
            || raw.test.sources.is_some())
    {
        return Err(invalid(
            "a virtual workspace cannot declare Cone dependencies or sources",
            raw.schema.span(),
        ));
    }
    let dependencies = dependency::declarations(raw.dependencies)?;
    let test_dependencies = dependency::declarations(raw.test_dependencies)?;
    for (key, declaration) in &test_dependencies {
        if dependencies.contains_key(key) {
            let span = match declaration {
                DependencyDeclaration::Workspace { span } => span,
                DependencyDeclaration::Request(request) => &request.span,
            };
            return Err(ManifestParseError::new(
                ManifestParseErrorKind::DuplicateTestDependency(key.to_string()),
                Some(span.clone()),
            ));
        }
    }
    let native = raw.native.parse(catalog)?;
    let publish = publish(raw.publish)?;
    if raw.cone.is_none()
        && (native != NativeConfig::default() || publish != PublishConfig::default())
    {
        return Err(invalid(
            "a virtual workspace cannot declare native or publish inputs",
            raw.schema.span(),
        ));
    }
    Ok(PackageDocument {
        cone: raw.cone.map(cone).transpose()?,
        workspace: raw.workspace.map(workspace).transpose()?,
        registry: raw
            .registry
            .default
            .map(|value| {
                value
                    .get_ref()
                    .parse()
                    .map_err(|message| invalid(message, value.span()))
            })
            .transpose()?,
        dependencies,
        test_dependencies,
        overrides: dependency::requests(raw.overrides, true)?,
        sources: crate::selection::parse_sources(raw.sources, catalog)?,
        test_sources: crate::selection::parse_sources(raw.test.sources, catalog)?,
        native,
        publish,
    })
}

fn cone(raw: RawCone) -> Result<ConeDeclaration, ManifestParseError> {
    let kind = match raw.kind.get_ref().as_str() {
        "library" => RequestedConeKind::Library,
        "executable" => RequestedConeKind::Executable,
        value => {
            return Err(ManifestParseError::new(
                ManifestParseErrorKind::InvalidConeKind(value.to_owned()),
                Some(raw.kind.span()),
            ));
        }
    };
    if let Some(value) = &raw.readme {
        if path(value.get_ref())
            .map_err(|message| invalid(message, value.span()))?
            .is_absolute()
        {
            return Err(invalid("readme must be a relative path", value.span()));
        }
    }
    Ok(ConeDeclaration {
        group: inheritable(raw.group)?,
        name: field(raw.name),
        version: inheritable(raw.version)?,
        kind,
        kind_span: raw.kind.span(),
        metadata: ConeMetadata {
            license: raw.license.map(inheritable).transpose()?,
            repository: raw.repository.map(inheritable).transpose()?,
            description: raw.description.map(field),
            readme: raw.readme.map(field),
        },
    })
}

fn workspace(raw: RawWorkspace) -> Result<WorkspaceDefinition, ManifestParseError> {
    let mut cone = BTreeMap::new();
    for (key, value) in raw.cone {
        if !matches!(key.as_str(), "group" | "version" | "license" | "repository") {
            return Err(invalid(
                format!("workspace.cone cannot define {key:?}"),
                value.span(),
            ));
        }
        cone.insert(key, field(value));
    }
    Ok(WorkspaceDefinition {
        members: raw
            .members
            .into_iter()
            .map(relative)
            .collect::<Result<_, _>>()?,
        default_members: raw
            .default_members
            .map(|values| values.into_iter().map(relative).collect::<Result<_, _>>())
            .transpose()?,
        cone,
        dependencies: dependency::requests(raw.dependencies, false)?,
    })
}

fn inheritable(value: Spanned<toml::Value>) -> Result<InheritableValue, ManifestParseError> {
    let span = value.span();
    match value.into_inner() {
        toml::Value::String(value) => Ok(InheritableValue::Value(ManifestValue { value, span })),
        toml::Value::Table(table)
            if table.len() == 1
                && table.get("workspace").and_then(toml::Value::as_bool) == Some(true) =>
        {
            Ok(InheritableValue::Workspace { span })
        }
        _ => Err(invalid("expected a string or { workspace = true }", span)),
    }
}

fn field<T>(value: Spanned<T>) -> ManifestValue<T> {
    ManifestValue {
        span: value.span(),
        value: value.into_inner(),
    }
}

fn relative(value: Spanned<String>) -> Result<ManifestValue<ConeRelativePath>, ManifestParseError> {
    Ok(ManifestValue {
        value: ConeRelativePath::new(value.get_ref())
            .map_err(|message| invalid(message, value.span()))?,
        span: value.span(),
    })
}

fn publish(raw: RawPublish) -> Result<PublishConfig, ManifestParseError> {
    let paths = |values: Vec<Spanned<String>>| {
        values
            .into_iter()
            .map(|value| relative(value).map(|value| value.value))
            .collect::<Result<Vec<_>, _>>()
    };
    Ok(PublishConfig {
        include: raw.include.map(paths).transpose()?,
        exclude: paths(raw.exclude)?,
        files: raw
            .files
            .into_iter()
            .map(|value| {
                let from = path(value.from.get_ref())
                    .map_err(|message| invalid(message, value.from.span()))?;
                if from.is_absolute() {
                    return Err(invalid(
                        "publish.files.from must be a relative path",
                        value.from.span(),
                    ));
                }
                Ok(PublishFile {
                    from,
                    to: relative(value.to)?.value,
                })
            })
            .collect::<Result<_, ManifestParseError>>()?,
    })
}
