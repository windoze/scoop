mod bridge_validation;
mod support;

use scoop_identity::ConeIdentity;

use super::artifact::{
    artifact_bytes, artifact_bytes_with_dependencies, checked_artifact,
    checked_artifact_with_authorities, dependency_record, identity_graph, target,
};
use crate::{
    layout_compile_closure::layout_hir_semantic_closure_for_test,
    strong_compile_decode::tests::cone_named,
};
use support::*;

#[test]
fn empty_hir_to_mir_closure_is_lent_from_scoped_arenas() {
    let bytes = artifact_bytes("mir-empty");
    let provider = checked_artifact(&bytes);
    let identity = provider.identity();
    let closure = layout_hir_semantic_closure_for_test(
        ConeIdentity::CORE,
        target(),
        Vec::new(),
        vec![provider],
        vec![vec![]],
    );
    let mut hir_bundles = [HirAuthorityBundle::empty(identity)];
    let mut hir = hir_authorities(&mut hir_bundles);
    let mut mir_factories = [RecordingMirFactory::new(identity)];
    let mut mir = mir_authorities(&mut mir_factories);

    closure
        .with_checked_mir_type_bridges(&mut hir, &mut mir, |checked| {
            assert_eq!(checked.current(), ConeIdentity::CORE);
            assert_eq!(checked.target_selection(), target());
            let provider = checked.provider(identity).unwrap();
            assert_eq!(provider.position(), 0);
            assert_eq!(provider.hir().provider(), identity);
            assert_eq!(provider.type_bridge().provider(), identity);
            assert!(provider.type_bridge().selected().is_empty());
            let _ = provider.lir_foundation().as_canonical();
            let _ = provider.lir_strong_production_wire();
            let _ = provider.lir_cross_cone_bridge_wire();
            let _ = provider.lir_layout_abi_wire();
        })
        .unwrap();
    drop(mir);
    assert_eq!(mir_factories[0].builds, 1);
}

#[test]
fn nonempty_dependency_view_matches_the_exact_graph_edge() {
    let terminal_bytes = artifact_bytes("mir-dependency-terminal");
    let terminal_record = dependency_record(&terminal_bytes);
    let consumer_bytes =
        artifact_bytes_with_dependencies("mir-dependency-consumer", vec![terminal_record]);
    let mut terminal = checked_artifact(&terminal_bytes);
    let consumer =
        checked_artifact_with_authorities(&consumer_bytes, [identity_graph(&mut terminal)]);
    let identities = [terminal.identity(), consumer.identity()];
    let closure = layout_hir_semantic_closure_for_test(
        cone_named("mir-dependency-current").identity(),
        target(),
        vec![identities[1]],
        vec![terminal, consumer],
        vec![vec![], vec![0]],
    );
    let mut hir_bundles = identities.map(HirAuthorityBundle::empty);
    let mut hir = hir_authorities(&mut hir_bundles);
    let mut mir_factories = identities.map(RecordingMirFactory::new);
    let mut mir = mir_authorities(&mut mir_factories);

    closure
        .with_checked_mir_type_bridges(&mut hir, &mut mir, |checked| {
            assert_eq!(checked.direct_providers(), &[identities[1]]);
            assert_eq!(checked.dependency_first().count(), 2);
        })
        .unwrap();
    drop(mir);
    assert_eq!(mir_factories[1].observation.direct, vec![identities[0]]);
    assert_eq!(mir_factories[1].observation.transitive, vec![identities[0]]);
}

#[test]
fn diamond_dependencies_share_one_terminal_mir_proof() {
    let terminal_bytes = artifact_bytes("mir-diamond-terminal");
    let terminal_record = dependency_record(&terminal_bytes);
    let left_bytes =
        artifact_bytes_with_dependencies("mir-diamond-left", vec![terminal_record.clone()]);
    let right_bytes = artifact_bytes_with_dependencies("mir-diamond-right", vec![terminal_record]);
    let facade_bytes = artifact_bytes_with_dependencies(
        "mir-diamond-facade",
        vec![
            dependency_record(&left_bytes),
            dependency_record(&right_bytes),
        ],
    );
    let mut terminal = checked_artifact(&terminal_bytes);
    let mut left = checked_artifact_with_authorities(&left_bytes, [identity_graph(&mut terminal)]);
    let mut right =
        checked_artifact_with_authorities(&right_bytes, [identity_graph(&mut terminal)]);
    let facade = checked_artifact_with_authorities(
        &facade_bytes,
        [identity_graph(&mut left), identity_graph(&mut right)],
    );
    let providers = vec![terminal, left, right, facade];
    let identities = providers
        .iter()
        .map(|provider| provider.identity())
        .collect::<Vec<_>>();
    let closure = layout_hir_semantic_closure_for_test(
        cone_named("mir-diamond-current").identity(),
        target(),
        vec![identities[3]],
        providers,
        vec![vec![], vec![0], vec![0], vec![1, 2]],
    );
    let mut hir_bundles = identities
        .iter()
        .copied()
        .map(HirAuthorityBundle::empty)
        .collect::<Vec<_>>();
    let mut hir = hir_authorities(&mut hir_bundles);
    let mut mir_factories = identities
        .iter()
        .copied()
        .map(RecordingMirFactory::new)
        .collect::<Vec<_>>();
    let mut mir = mir_authorities(&mut mir_factories);

    closure
        .with_checked_mir_type_bridges(&mut hir, &mut mir, |checked| {
            assert_eq!(checked.dependency_first().count(), 4);
        })
        .unwrap();
    drop(mir);
    let terminal_from_left = mir_factories[1].observation.direct_bridge_addresses[0];
    let terminal_from_right = mir_factories[2].observation.direct_bridge_addresses[0];
    let terminal_from_facade = mir_factories[3].observation.transitive_bridge_addresses[0];
    assert_eq!(terminal_from_left, terminal_from_right);
    assert_eq!(terminal_from_left, terminal_from_facade);
    assert_eq!(mir_factories[3].observation.transitive.len(), 3);
}
