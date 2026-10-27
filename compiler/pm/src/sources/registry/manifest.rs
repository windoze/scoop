use scoop_manifest::package::{
    DependencyDeclaration, GitReference, InheritableValue, PackageSource, PublishConfig,
};

use super::*;

/// Read a standalone published manifest once and retain its declaration data.
pub fn published_manifest(
    text: &str,
) -> PmResult<(
    ConeCoordinate,
    RequestedConeKind,
    Vec<RegistryDependency>,
    PackageDocument,
)> {
    let document =
        scoop_manifest::package::parse_package_manifest(text, scoop_toolchain::SOURCE_TARGETS)
            .map_err(|error| PmError::new(format!("invalid packaged Cone.toml: {error}")))?;
    if document.workspace.is_some()
        || !document.overrides.is_empty()
        || document.publish != PublishConfig::default()
    {
        return Err(PmError::new(
            "published manifest must not contain workspace, overrides, or publish configuration",
        ));
    }
    let cone = document
        .cone
        .as_ref()
        .ok_or_else(|| PmError::new("published manifest must define a Cone"))?;
    let literal = |value: &InheritableValue| match value {
        InheritableValue::Value(value) => Ok(value.value.clone()),
        InheritableValue::Workspace { .. } => Err(PmError::new(
            "published manifest must expand workspace inheritance",
        )),
    };
    let coordinate = ConeCoordinate::new(
        &literal(&cone.group)?,
        &cone.name.value,
        &literal(&cone.version)?,
    )
    .map_err(|error| PmError::new(error.to_string()))?;
    if coordinate == ConeCoordinate::reserved_single_file()
        || (coordinate == ConeCoordinate::reserved_core()
            && cone.kind != RequestedConeKind::Library)
    {
        return Err(PmError::new(
            "invalid reserved package coordinate or core kind",
        ));
    }
    for field in [&cone.metadata.license, &cone.metadata.repository]
        .into_iter()
        .flatten()
    {
        literal(field)?;
    }
    let mut dependencies = document
        .dependencies
        .iter()
        .map(|(key, dependency)| published_dependency(key, dependency))
        .collect::<PmResult<Vec<_>>>()?;
    for (key, dependency) in &document.test_dependencies {
        published_dependency(key, dependency)?;
    }
    let core = ConeCoordinate::reserved_core();
    let core_key = DependencyCoordinateKey::from_coordinate(&core);
    if let Some(dependency) = dependencies
        .iter_mut()
        .find(|dependency| dependency.key == core_key)
    {
        if !dependency
            .version
            .matches(&semver::Version::parse(core.version()).expect("core version is valid"))
        {
            return Err(PmError::new(
                "explicit core requirement does not accept scoop:scoop.core:0.1.0",
            ));
        }
        dependency.version = "=0.1.0".parse().expect("fixed core requirement is valid");
    }
    if coordinate != core
        && !dependencies
            .iter()
            .any(|dependency| dependency.key == core_key)
    {
        dependencies.push(RegistryDependency {
            key: core_key,
            version: "=0.1.0".parse().expect("fixed core requirement is valid"),
            source: RegistrySource::Sysroot,
        });
    }
    dependencies.sort_by(|left, right| left.key.cmp(&right.key));
    Ok((coordinate, cone.kind, dependencies, document))
}

fn published_dependency(
    key: &DependencyCoordinateKey,
    declaration: &DependencyDeclaration,
) -> PmResult<RegistryDependency> {
    let DependencyDeclaration::Request(request) = declaration else {
        return Err(PmError::new(format!(
            "published dependency {key} must expand workspace inheritance"
        )));
    };
    let version = request
        .requirement
        .clone()
        .ok_or_else(|| PmError::new(format!("published dependency {key} requires a version")))?;
    let source = match &request.source {
        PackageSource::Registry(registry) => RegistrySource::Registry {
            registry: registry.clone(),
        },
        PackageSource::Default if key.group() == "scoop" => RegistrySource::Sysroot,
        PackageSource::Git(source) => {
            let GitReference::Revision(commit) = &source.reference else {
                return Err(PmError::new(format!(
                    "published Git dependency {key} must select a full commit"
                )));
            };
            if !matches!(commit.len(), 40 | 64)
                || !commit
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            {
                return Err(PmError::new(format!(
                    "published Git dependency {key} must select a full commit"
                )));
            }
            RegistrySource::Git {
                git: source.repository.clone(),
                commit: commit.clone(),
                subdir: source.subdir.as_str().into(),
            }
        }
        _ => {
            return Err(PmError::new(format!(
                "published dependency {key} needs an explicit registry, pinned Git, or sysroot source"
            )));
        }
    };
    Ok(RegistryDependency {
        key: key.clone(),
        version,
        source,
    })
}
