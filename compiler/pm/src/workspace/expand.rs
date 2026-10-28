use scoop_identity::{ConeCoordinateComponent, ConeCoordinateError};
use scoop_manifest::package::{DependencyDeclaration, InheritableValue};

use super::*;

pub(super) fn expand(
    loaded: &LoadedManifest,
    workspace: Option<&LoadedManifest>,
) -> PmResult<ExpandedCone> {
    let declaration =
        loaded.document.cone.as_ref().ok_or_else(|| {
            PmError::new(format!("{} does not define a Cone", loaded.path.display()))
        })?;
    let group = inherit("group", &declaration.group, loaded, workspace)?;
    let version = inherit("version", &declaration.version, loaded, workspace)?;
    let coordinate =
        ConeCoordinate::new(&group.0, &declaration.name.value, &version.0).map_err(|error| {
            let (origin, field) = match error {
                ConeCoordinateError::InvalidText {
                    component: ConeCoordinateComponent::Group,
                    ..
                } => (group.1, Some(&declaration.group)),
                ConeCoordinateError::InvalidText {
                    component: ConeCoordinateComponent::Name,
                    ..
                } => (loaded.origin(declaration.name.span.clone()), None),
                ConeCoordinateError::InvalidSemanticVersion
                | ConeCoordinateError::NonCanonicalSemanticVersion => {
                    (version.1, Some(&declaration.version))
                }
            };
            let mut error = PmError::new(error.to_string()).at(origin);
            if let Some(InheritableValue::Workspace { span }) = field {
                error = error.at(loaded.origin(span.clone()));
            }
            error
        })?;
    if coordinate == ConeCoordinate::reserved_single_file() {
        return Err(PmError::new(
            "user manifest cannot declare the reserved single-file coordinate",
        )
        .at(loaded.origin(declaration.name.span.clone())));
    }
    if coordinate == ConeCoordinate::reserved_core()
        && declaration.kind != RequestedConeKind::Library
    {
        return Err(
            PmError::new("core must be a library").at(loaded.origin(declaration.kind_span.clone()))
        );
    }
    let mut metadata = BTreeMap::new();
    for (key, value) in [
        ("license", &declaration.metadata.license),
        ("repository", &declaration.metadata.repository),
    ] {
        if let Some(value) = value {
            metadata.insert(key.to_owned(), inherit(key, value, loaded, workspace)?.0);
        }
    }
    for (key, value) in [
        ("description", &declaration.metadata.description),
        ("readme", &declaration.metadata.readme),
    ] {
        if let Some(value) = value {
            metadata.insert(key.to_owned(), value.value.clone());
        }
    }
    Ok(ExpandedCone {
        coordinate,
        kind: declaration.kind,
        metadata,
        dependencies: dependencies(&loaded.document.dependencies, loaded, workspace)?,
        test_dependencies: dependencies(&loaded.document.test_dependencies, loaded, workspace)?,
        sources: loaded.document.sources.clone(),
        test_sources: loaded.document.test_sources.clone(),
        native: loaded.document.native.clone(),
    })
}

fn inherit(
    field: &str,
    value: &InheritableValue,
    loaded: &LoadedManifest,
    workspace: Option<&LoadedManifest>,
) -> PmResult<(String, ManifestOrigin)> {
    match value {
        InheritableValue::Value(value) => {
            Ok((value.value.clone(), loaded.origin(value.span.clone())))
        }
        InheritableValue::Workspace { span } => {
            let (owner, definition) = workspace
                .and_then(|owner| owner.document.workspace.as_ref().map(|ws| (owner, ws)))
                .ok_or_else(|| {
                    PmError::new(format!(
                        "cannot inherit {field}: Cone is not a workspace member"
                    ))
                    .at(loaded.origin(span.clone()))
                })?;
            let value = definition.cone.get(field).ok_or_else(|| {
                PmError::new(format!(
                    "workspace {} does not define cone.{field}",
                    owner.path.display()
                ))
                .at(loaded.origin(span.clone()))
            })?;
            Ok((value.value.clone(), owner.origin(value.span.clone())))
        }
    }
}

fn dependencies(
    declarations: &BTreeMap<DependencyCoordinateKey, DependencyDeclaration>,
    loaded: &LoadedManifest,
    workspace: Option<&LoadedManifest>,
) -> PmResult<BTreeMap<DependencyCoordinateKey, ExpandedDependency>> {
    declarations
        .iter()
        .map(|(key, declaration)| {
            let (request, definition, inherited_at) = match declaration {
                DependencyDeclaration::Request(request) => (request, loaded, None),
                DependencyDeclaration::Workspace { span } => {
                    let (owner, ws) = workspace
                        .and_then(|owner| owner.document.workspace.as_ref().map(|ws| (owner, ws)))
                        .ok_or_else(|| {
                            PmError::new(format!(
                                "cannot inherit {key}: Cone is not a workspace member"
                            ))
                            .at(loaded.origin(span.clone()))
                        })?;
                    let request = ws.dependencies.get(key).ok_or_else(|| {
                        PmError::new(format!(
                            "workspace {} does not define dependency {key}",
                            owner.path.display()
                        ))
                        .at(loaded.origin(span.clone()))
                    })?;
                    (request, owner, Some(loaded.origin(span.clone())))
                }
            };
            let registry = definition
                .document
                .registry
                .clone()
                .or_else(|| workspace.and_then(|owner| owner.document.registry.clone()));
            Ok((
                key.clone(),
                ExpandedDependency {
                    request: request.clone(),
                    origin: definition.origin(request.span.clone()),
                    inherited_at,
                    base: definition.directory.clone(),
                    registry,
                },
            ))
        })
        .collect()
}
