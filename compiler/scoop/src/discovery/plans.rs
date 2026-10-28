use super::*;

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
