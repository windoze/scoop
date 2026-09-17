use scoop_identity::{ConeCoordinate, ConeIdentity};

use super::*;
use crate::{
    ConeKind, ConeRecord, ConeSourceForm, HirFingerprint,
    cross_cone_compile_decode::tests::{cross_cone_artifact_for, empty_cross_cone_hir_interface},
    strong_compile_decode::tests::{cone_named, open_graph},
};

#[test]
fn profile_graph_assigns_direct_and_support_roles_after_closure_validation() {
    let core_bytes = artifact(core_cone(), Vec::new());
    let core = decode(&core_bytes);
    let terminal_bytes = artifact(cone_named("terminal"), vec![core.dependency_record()]);
    let terminal = decode(&terminal_bytes);
    let facade_bytes = artifact(cone_named("facade"), vec![terminal.dependency_record()]);

    let core = decode(&core_bytes);
    let terminal = decode(&terminal_bytes);
    let facade = decode(&facade_bytes);
    let mut direct = vec![ConeIdentity::CORE, facade.identity()];
    direct.sort_unstable();
    let closure = DecodedCrossConeClosure::new(
        cone_named("current").identity(),
        target(),
        direct,
        vec![core, terminal, facade],
    )
    .validate_profile_graph()
    .unwrap();

    assert_eq!(closure.current(), cone_named("current").identity());
    assert_eq!(closure.target_selection(), target());
    assert_eq!(closure.dependency_first().count(), 3);
    assert_eq!(
        closure.role(ConeIdentity::CORE),
        Some(CrossConeProviderRole::Direct)
    );
    assert_eq!(
        closure.role(cone_named("facade").identity()),
        Some(CrossConeProviderRole::Direct)
    );
    assert_eq!(
        closure.role(cone_named("terminal").identity()),
        Some(CrossConeProviderRole::Support)
    );
    assert!(
        closure
            .artifact(cone_named("terminal").identity())
            .is_some()
    );
    assert_eq!(closure.role(cone_named("absent").identity()), None);
}

#[test]
fn core_current_has_the_only_valid_empty_provider_closure() {
    let closure =
        DecodedCrossConeClosure::new(ConeIdentity::CORE, target(), Vec::new(), Vec::new())
            .validate_profile_graph()
            .unwrap();
    assert_eq!(closure.current(), ConeIdentity::CORE);
    assert_eq!(closure.dependency_first().count(), 0);

    let core_bytes = artifact(core_cone(), Vec::new());
    assert_eq!(
        DecodedCrossConeClosure::new(
            ConeIdentity::CORE,
            target(),
            vec![ConeIdentity::CORE],
            vec![decode(&core_bytes)],
        )
        .validate_profile_graph()
        .err(),
        Some(CrossConeClosureGraphError::CoreHasDependencyProviders)
    );
}

#[test]
fn non_core_closure_requires_implicit_core_and_canonical_direct_set() {
    let dependency_bytes = artifact(cone_named("dependency"), Vec::new());
    let dependency = decode(&dependency_bytes);
    let identity = dependency.identity();
    assert_eq!(
        DecodedCrossConeClosure::new(
            cone_named("current").identity(),
            target(),
            vec![identity],
            vec![dependency],
        )
        .validate_profile_graph()
        .err(),
        Some(CrossConeClosureGraphError::MissingTrustedCore)
    );

    let core_bytes = artifact(core_cone(), Vec::new());
    assert!(matches!(
        DecodedCrossConeClosure::new(
            cone_named("current").identity(),
            target(),
            vec![ConeIdentity::CORE, ConeIdentity::CORE],
            vec![decode(&core_bytes)],
        )
        .validate_profile_graph(),
        Err(CrossConeClosureGraphError::NonCanonicalDirectProviders { .. })
    ));
}

#[test]
fn profile_graph_rejects_non_dependency_first_artifacts() {
    let core_bytes = artifact(core_cone(), Vec::new());
    let core = decode(&core_bytes);
    let dependent_bytes = artifact(cone_named("dependent"), vec![core.dependency_record()]);
    let dependent = decode(&dependent_bytes);
    let mut direct = vec![ConeIdentity::CORE, dependent.identity()];
    direct.sort_unstable();

    assert!(matches!(
        DecodedCrossConeClosure::new(
            cone_named("current").identity(),
            target(),
            direct,
            vec![dependent, decode(&core_bytes)],
        )
        .validate_profile_graph(),
        Err(CrossConeClosureGraphError::InvalidDependencyFirstOrder {
            dependency: ConeIdentity::CORE,
            ..
        })
    ));
}

#[test]
fn profile_graph_rejects_a_stale_dependency_fingerprint() {
    let core_bytes = artifact(core_cone(), Vec::new());
    let core = decode(&core_bytes);
    let actual = core.dependency_record();
    let stale = crate::DependencyRecord::new(
        actual.coordinate().clone(),
        HirFingerprint::from_array([9; 32]),
        actual.mir_fingerprint(),
        actual.lir_fingerprint(),
    )
    .unwrap();
    let dependent_bytes = artifact(cone_named("dependent"), vec![stale]);
    let dependent = decode(&dependent_bytes);
    let mut direct = vec![ConeIdentity::CORE, dependent.identity()];
    direct.sort_unstable();

    assert!(matches!(
        DecodedCrossConeClosure::new(
            cone_named("current").identity(),
            target(),
            direct,
            vec![decode(&core_bytes), dependent],
        )
        .validate_profile_graph(),
        Err(CrossConeClosureGraphError::StaleDependency {
            dependency: ConeIdentity::CORE,
            ..
        })
    ));
}

#[test]
fn profile_graph_rejects_an_unreachable_support_artifact() {
    let core_bytes = artifact(core_cone(), Vec::new());
    let direct_bytes = artifact(cone_named("direct"), Vec::new());
    let unused_bytes = artifact(cone_named("unused"), Vec::new());
    let direct_identity = decode(&direct_bytes).identity();
    let unused_identity = decode(&unused_bytes).identity();
    let mut direct = vec![ConeIdentity::CORE, direct_identity];
    direct.sort_unstable();

    assert_eq!(
        DecodedCrossConeClosure::new(
            cone_named("current").identity(),
            target(),
            direct,
            vec![
                decode(&core_bytes),
                decode(&unused_bytes),
                decode(&direct_bytes),
            ],
        )
        .validate_profile_graph()
        .err(),
        Some(CrossConeClosureGraphError::UnreachableSupport {
            identity: unused_identity,
        })
    );
}

#[test]
fn profile_graph_rejects_two_versions_of_one_coordinate_family() {
    let core_bytes = artifact(core_cone(), Vec::new());
    let first = ConeRecord::new(
        ConeCoordinate::new("test", "versioned", "1.0.0").unwrap(),
        ConeKind::Library,
        ConeSourceForm::Manifest,
    )
    .unwrap();
    let second = ConeRecord::new(
        ConeCoordinate::new("test", "versioned", "2.0.0").unwrap(),
        ConeKind::Library,
        ConeSourceForm::Manifest,
    )
    .unwrap();
    let first_bytes = artifact(first, Vec::new());
    let second_bytes = artifact(second, Vec::new());
    let mut direct = vec![
        ConeIdentity::CORE,
        decode(&first_bytes).identity(),
        decode(&second_bytes).identity(),
    ];
    direct.sort_unstable();

    assert!(matches!(
        DecodedCrossConeClosure::new(
            cone_named("current").identity(),
            target(),
            direct,
            vec![
                decode(&core_bytes),
                decode(&first_bytes),
                decode(&second_bytes),
            ],
        )
        .validate_profile_graph(),
        Err(CrossConeClosureGraphError::MultipleVersions { .. })
    ));
}

fn artifact(cone: ConeRecord, dependencies: Vec<crate::DependencyRecord>) -> Vec<u8> {
    cross_cone_artifact_for(cone, dependencies, empty_cross_cone_hir_interface())
}

fn decode(bytes: &[u8]) -> DecodedCrossConeHirFrontSections<'_> {
    open_graph(bytes)
        .decode_cross_cone_hir_front_sections()
        .unwrap()
}

fn core_cone() -> ConeRecord {
    ConeRecord::new(
        ConeCoordinate::reserved_core(),
        ConeKind::Library,
        ConeSourceForm::Manifest,
    )
    .unwrap()
}

fn target() -> ValidatedLirTargetSelection {
    ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1
}
