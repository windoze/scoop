use scoop_identity::ConeIdentity;

use super::*;
use crate::{
    layout_compile_decode::tests::layout_artifact,
    strong_compile_decode::tests::{cone, cone_named, open_graph},
};

#[test]
fn core_can_have_the_only_empty_layout_profile_graph() {
    let closure = DecodedCrossConeLayoutCompileClosure::new(
        ConeIdentity::CORE,
        scoop_lir::ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1,
        Vec::new(),
        Vec::new(),
    )
    .validate_profile_graph()
    .unwrap();

    assert_eq!(closure.current(), ConeIdentity::CORE);
    assert!(closure.direct_providers().is_empty());
    assert_eq!(closure.dependency_first().count(), 0);
    assert_eq!(closure.dependency_count(ConeIdentity::CORE), None);

    let closure = closure
        .validate_identities()
        .unwrap()
        .validate_foundation_structure()
        .unwrap()
        .resolve_hir_sections()
        .unwrap()
        .validate_hir_productions()
        .unwrap();
    assert_eq!(closure.current(), ConeIdentity::CORE);
    assert_eq!(closure.dependency_first().count(), 0);
}

#[test]
fn layout_profile_graph_requires_the_trusted_core_provider() {
    let bytes = layout_artifact(false, None);
    let artifact = open_graph(&bytes)
        .decode_cross_cone_layout_compile_sections()
        .unwrap();
    let identity = artifact.identity();

    assert_eq!(
        DecodedCrossConeLayoutCompileClosure::new(
            cone_named("layout-current").identity(),
            scoop_lir::ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1,
            vec![identity],
            vec![artifact],
        )
        .validate_profile_graph()
        .err(),
        Some(CrossConeClosureGraphError::MissingTrustedCore)
    );
}

#[test]
fn layout_profile_graph_rejects_current_artifact_in_provider_storage() {
    let bytes = layout_artifact(false, None);
    let artifact = open_graph(&bytes)
        .decode_cross_cone_layout_compile_sections()
        .unwrap();
    let identity = artifact.identity();

    assert_eq!(
        DecodedCrossConeLayoutCompileClosure::new(
            identity,
            scoop_lir::ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1,
            vec![ConeIdentity::CORE],
            vec![artifact],
        )
        .validate_profile_graph()
        .err(),
        Some(CrossConeClosureGraphError::CurrentArtifactPresent {
            current: cone().identity(),
        })
    );
}
