use super::*;

pub(super) struct ManifestDependencyPlan {
    pub(super) coordinate: ConeCoordinate,
    pub(super) locator: ManifestDependencyPlanLocator,
    pub(super) edge: DiscoveredDependencyEdge,
}

pub(super) enum ManifestDependencyPlanLocator {
    Explicit(DependencyCoordinateKey),
    SearchRoots,
}

pub(super) fn manifest_dependency_plans(
    manifest: &LoadedConeManifest,
    include_tests: bool,
) -> Result<Vec<ManifestDependencyPlan>, BuildGraphDiscoveryError> {
    let dependent = manifest.identity();
    let mut declarations = manifest
        .parsed()
        .semantic()
        .dependency_iter()
        .map(|(key, coordinate)| {
            let locator =
                manifest.parsed().locators().get(key).ok_or_else(|| {
                    BuildGraphDiscoveryError::MissingLocatorProjection(key.clone())
                })?;
            let span = manifest
                .parsed()
                .diagnostic_spans()
                .dependency(key)
                .ok_or_else(|| BuildGraphDiscoveryError::MissingDependencySpan(key.clone()))?
                .declaration()
                .range();
            Ok((key, coordinate, locator, span))
        })
        .collect::<Result<Vec<_>, BuildGraphDiscoveryError>>()?;
    if include_tests {
        declarations.extend(manifest.parsed().tests().dependency_iter().map(
            |(key, declaration)| {
                (
                    key,
                    declaration.coordinate(),
                    declaration.locator(),
                    declaration.spans().declaration().range(),
                )
            },
        ));
    }
    declarations
        .into_iter()
        .map(|(key, coordinate, locator, span)| {
            let dependency = coordinate
                .identity()
                .map_err(BuildGraphDiscoveryError::Identity)?;
            let (locator_kind, plan_locator) = match locator {
                DependencyLocator::SourcePath(_) => (
                    ManifestLocatorKind::SourcePath,
                    ManifestDependencyPlanLocator::Explicit(key.clone()),
                ),
                DependencyLocator::ArtifactPath(_) => (
                    ManifestLocatorKind::ArtifactPath,
                    ManifestDependencyPlanLocator::Explicit(key.clone()),
                ),
                DependencyLocator::SearchRoots => (
                    ManifestLocatorKind::SearchRoots,
                    ManifestDependencyPlanLocator::SearchRoots,
                ),
            };
            Ok(ManifestDependencyPlan {
                coordinate: coordinate.clone(),
                locator: plan_locator,
                edge: DiscoveredDependencyEdge::new(
                    dependent,
                    dependency,
                    coordinate.clone(),
                    EdgeOrigin::ManifestDeclaration {
                        manifest: manifest.manifest_path().to_path_buf(),
                        span,
                        locator_kind,
                    },
                    None,
                ),
            })
        })
        .collect()
}

pub(super) struct PrebuiltDependencyPlan {
    pub(super) coordinate: ConeCoordinate,
    pub(super) edge: DiscoveredDependencyEdge,
}

pub(super) fn prebuilt_dependency_plans(
    dependent: ConeIdentity,
    prebuilt: &PrebuiltArtifactProjection,
) -> Vec<PrebuiltDependencyPlan> {
    let artifact = prebuilt.first_candidate().resolved_path().to_path_buf();
    prebuilt
        .first_candidate()
        .summary()
        .direct_dependencies()
        .iter()
        .enumerate()
        .map(|(record_index, dependency)| PrebuiltDependencyPlan {
            coordinate: dependency.coordinate().clone(),
            edge: DiscoveredDependencyEdge::new(
                dependent,
                dependency.identity(),
                dependency.coordinate().clone(),
                EdgeOrigin::ArtifactDependencyRecord {
                    artifact: artifact.clone(),
                    record_index,
                },
                Some(dependency.clone()),
            ),
        })
        .collect()
}
