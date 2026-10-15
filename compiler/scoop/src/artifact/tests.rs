use super::*;

fn coordinate(name: &str) -> ConeCoordinate {
    ConeCoordinate::new("test", name, "1.0.0").unwrap()
}

fn node(coordinate: ConeCoordinate) -> PlannedArtifactNode {
    PlannedArtifactNode::new(
        coordinate,
        ConeKind::Library,
        ConeSourceForm::Manifest,
        None,
    )
}

#[test]
fn closure_order_is_a_stable_projection_of_the_graph_order() {
    let core_coordinate = ConeCoordinate::reserved_core();
    let core = core_coordinate.identity().unwrap();
    let left_coordinate = coordinate("left");
    let left = left_coordinate.identity().unwrap();
    let right_coordinate = coordinate("right");
    let right = right_coordinate.identity().unwrap();
    let root_coordinate = coordinate("root");
    let root = root_coordinate.identity().unwrap();
    let nodes = BTreeMap::from([
        (core, node(core_coordinate)),
        (left, node(left_coordinate)),
        (right, node(right_coordinate)),
        (root, node(root_coordinate)),
    ]);
    let edges = [
        PlannedArtifactEdge::new(left, core, None),
        PlannedArtifactEdge::new(right, core, None),
        PlannedArtifactEdge::new(root, left, None),
        PlannedArtifactEdge::new(root, right, None),
    ];
    let plan = ArtifactClosurePlan::new(
        root,
        scoop_target_data::DARWIN_AARCH64_LLVM_22_1,
        vec![core, right, left, root],
        nodes,
        edges,
    );

    let reachable = plan.reachable_from(left);
    assert_eq!(plan.closure_order(&reachable), vec![core, left]);
    assert!(
        plan.validate_dependency_first(&reachable, &plan.closure_order(&reachable))
            .is_ok()
    );
}

#[test]
fn closure_rejects_missing_artifact_before_projecting_purpose_handles() {
    let coordinate = ConeCoordinate::reserved_core();
    let core = coordinate.identity().unwrap();
    let plan = ArtifactClosurePlan::new(
        core,
        scoop_target_data::DARWIN_AARCH64_LLVM_22_1,
        vec![core],
        BTreeMap::from([(core, node(coordinate))]),
        [],
    );

    assert!(matches!(
        plan.validate(core, &BTreeMap::new()),
        Err(ArtifactClosureValidationError::MissingArtifact(identity)) if identity == core
    ));
}

#[test]
fn dependency_first_invariant_is_rechecked_by_the_closure_gate() {
    let core_coordinate = ConeCoordinate::reserved_core();
    let core = core_coordinate.identity().unwrap();
    let root_coordinate = coordinate("root");
    let root = root_coordinate.identity().unwrap();
    let plan = ArtifactClosurePlan::new(
        root,
        scoop_target_data::DARWIN_AARCH64_LLVM_22_1,
        vec![root, core],
        BTreeMap::from([(core, node(core_coordinate)), (root, node(root_coordinate))]),
        [PlannedArtifactEdge::new(root, core, None)],
    );
    let reachable = plan.reachable_from(root);

    assert!(matches!(
        plan.validate_dependency_first(&reachable, &plan.closure_order(&reachable)),
        Err(ArtifactClosureValidationError::InvalidCanonicalOrder {
            dependent,
            dependency,
        }) if dependent == root && dependency == core
    ));
}

#[test]
fn completed_nodes_share_artifact_data_with_their_closure() {
    use scoop_slib::{ArtifactCapabilityProfile, ConeRecord, read_artifact_manifest_summary};
    use std::sync::Arc;

    let coordinate = ConeCoordinate::reserved_core();
    let core = coordinate.identity().unwrap();
    let plan = ArtifactClosurePlan::new(
        core,
        scoop_target_data::DARWIN_AARCH64_LLVM_22_1,
        vec![core],
        BTreeMap::from([(core, node(coordinate.clone()))]),
        [],
    );
    let archive = crate::test_artifacts::manifest_archive(
        scoop_target_data::DARWIN_AARCH64_LLVM_22_1,
        ArtifactCapabilityProfile::CROSS_CONE_GENERIC,
        ConeRecord::new(coordinate, ConeKind::Library, ConeSourceForm::Manifest).unwrap(),
        "shared-artifact",
        Vec::new(),
    );
    let directory = tempfile::tempdir().unwrap();
    let candidate = directory.path().join("candidate.slib");
    let cached_path = directory.path().join("cached.slib");
    std::fs::write(&candidate, archive.as_bytes()).unwrap();
    std::fs::copy(&candidate, &cached_path).unwrap();
    let summary = read_artifact_manifest_summary(
        archive.as_bytes(),
        scoop_target_data::DARWIN_AARCH64_LLVM_22_1,
    )
    .unwrap();
    let artifact = BuildArtifact::from_result(candidate.clone(), summary);
    let snapshot = Arc::clone(artifact.snapshot().unwrap());
    let closure = plan
        .collect(core, &BTreeMap::from([(core, Rc::clone(&artifact))]))
        .unwrap();
    let completed = CompletedNode::from_result(
        core,
        CompletedNodeOrigin::Compiled,
        Rc::clone(&artifact),
        closure.clone(),
        candidate,
        Vec::new(),
    );
    assert!(Arc::ptr_eq(
        completed.artifact().snapshot().unwrap(),
        &snapshot
    ));
    let cached = CompletedNode::from_result(
        core,
        CompletedNodeOrigin::CacheHit,
        artifact,
        closure,
        cached_path,
        Vec::new(),
    );
    let retained = [completed.clone(), cached];
    for node in retained {
        assert!(node.artifact_locator().is_file());
        let artifact = node.closure().artifact(core).unwrap();
        assert!(std::ptr::eq(artifact, node.artifact()));
        assert_eq!(artifact.summary().cone().identity(), core);
        assert_eq!(artifact.snapshot().unwrap().as_bytes(), archive.as_bytes());
    }
}
