mod support;

use scoop_identity::ConeIdentity;
use scoop_lir::{
    LayoutAbiDependencyV1, LayoutAbiSectionError, LayoutAbiSemanticClosureError,
    LayoutAbiSemanticTargetV1,
};
use scoop_wire::{Encoder, WireEncode, encode};

use super::super::artifact::{
    artifact_bytes, artifact_bytes_with_dependencies, artifact_bytes_with_lir_layout_abi,
    checked_artifact, checked_artifact_with_authorities, dependency_record, identity_graph,
    nominal_artifact_bytes, target,
};
use super::super::type_authority::NominalFixture;
use super::support::*;
use crate::{
    CrossConeLayoutLirSemanticClosureError, LayoutLirProviderSourceAuthorityV1,
    layout_compile_closure::layout_hir_semantic_closure_for_test,
    strong_compile_decode::tests::cone_named,
};
use support::*;

#[test]
fn empty_hir_mir_lir_closure_is_lent_from_one_scoped_chain() {
    let bytes = artifact_bytes("lir-empty");
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
    let mut lir_factories = [RecordingLirFactory::new(identity)];
    let mut lir = lir_authorities(&mut lir_factories);

    closure
        .with_checked_lir_layout_abi(&mut hir, &mut mir, &mut lir, |checked| {
            assert_eq!(checked.current(), ConeIdentity::CORE);
            assert_eq!(checked.target_selection(), target());
            let provider = checked.provider(identity).unwrap();
            assert_eq!(provider.position(), 0);
            assert_eq!(provider.mir().provider(), identity);
            assert_eq!(provider.ordinary_bridge().artifact(), identity);
            assert_eq!(provider.strong_production().provider(), identity);
            assert_eq!(provider.layout_abi().provider(), identity);
            assert!(provider.layout_abi().selected().is_empty());
        })
        .unwrap();
    drop(lir);
    assert_eq!(lir_factories[0].builds, 1);
}

#[test]
fn nonempty_dependency_context_matches_the_exact_lir_graph() {
    let terminal_bytes = artifact_bytes("lir-dependency-terminal");
    let terminal_record = dependency_record(&terminal_bytes);
    let consumer_bytes =
        artifact_bytes_with_dependencies("lir-dependency-consumer", vec![terminal_record]);
    let mut terminal = checked_artifact(&terminal_bytes);
    let consumer =
        checked_artifact_with_authorities(&consumer_bytes, [identity_graph(&mut terminal)]);
    let identities = [terminal.identity(), consumer.identity()];
    let closure = layout_hir_semantic_closure_for_test(
        ConeIdentity::CORE,
        target(),
        vec![identities[1]],
        vec![terminal, consumer],
        vec![vec![], vec![0]],
    );
    let mut hir_bundles = identities.map(HirAuthorityBundle::empty);
    let mut hir = hir_authorities(&mut hir_bundles);
    let mut mir_factories = identities.map(RecordingMirFactory::new);
    let mut mir = mir_authorities(&mut mir_factories);
    let mut lir_factories = identities.map(RecordingLirFactory::new);
    let mut lir = lir_authorities(&mut lir_factories);

    closure
        .with_checked_lir_layout_abi(&mut hir, &mut mir, &mut lir, |checked| {
            assert_eq!(checked.direct_providers(), &[identities[1]]);
            assert_eq!(checked.dependency_first().count(), 2);
        })
        .unwrap();
    drop(lir);
    assert_eq!(lir_factories[1].observation.direct, vec![identities[0]]);
    assert_eq!(lir_factories[1].observation.transitive, vec![identities[0]]);
    assert_eq!(
        lir_factories[1].observation.direct_layout_addresses.len(),
        1
    );
}

#[test]
fn diamond_dependencies_share_one_terminal_layout_proof() {
    let terminal_bytes = artifact_bytes("lir-diamond-terminal");
    let terminal_record = dependency_record(&terminal_bytes);
    let left_bytes =
        artifact_bytes_with_dependencies("lir-diamond-left", vec![terminal_record.clone()]);
    let right_bytes = artifact_bytes_with_dependencies("lir-diamond-right", vec![terminal_record]);
    let facade_bytes = artifact_bytes_with_dependencies(
        "lir-diamond-facade",
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
        ConeIdentity::CORE,
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
    let mut lir_factories = identities
        .iter()
        .copied()
        .map(RecordingLirFactory::new)
        .collect::<Vec<_>>();
    let mut lir = lir_authorities(&mut lir_factories);

    closure
        .with_checked_lir_layout_abi(&mut hir, &mut mir, &mut lir, |checked| {
            assert_eq!(checked.dependency_first().count(), 4);
        })
        .unwrap();
    drop(lir);
    let terminal_from_left = lir_factories[1].observation.direct_layout_addresses[0];
    let terminal_from_right = lir_factories[2].observation.direct_layout_addresses[0];
    let terminal_from_facade = lir_factories[3].observation.transitive_layout_addresses[0];
    assert_eq!(terminal_from_left, terminal_from_right);
    assert_eq!(terminal_from_left, terminal_from_facade);
    assert_eq!(lir_factories[3].observation.transitive.len(), 3);
}

#[test]
fn provider_and_returned_source_authority_must_match_the_artifact() {
    let bytes = artifact_bytes("lir-wrong-provider");
    let expected = cone_named("lir-wrong-provider").identity();
    let wrong = cone_named("lir-wrong-provider-other").identity();
    let closure = single_closure(&bytes);
    let mut hir_bundles = [HirAuthorityBundle::empty(expected)];
    let mut hir = hir_authorities(&mut hir_bundles);
    let mut mir_factories = [RecordingMirFactory::new(expected)];
    let mut mir = mir_authorities(&mut mir_factories);
    let mut wrong_factory = RecordingLirFactory::new(wrong);
    let mut lir = [LayoutLirProviderSourceAuthorityV1::new(
        wrong,
        &mut wrong_factory,
    )];
    assert!(matches!(
        closure.with_checked_lir_layout_abi(&mut hir, &mut mir, &mut lir, |_| ()),
        Err(CrossConeLayoutLirSemanticClosureError::AuthorityProvider {
            position: 0,
            expected: actual_expected,
            actual,
        }) if actual_expected == expected && actual == wrong
    ));

    let closure = single_closure(&bytes);
    let mut hir = hir_authorities(&mut hir_bundles);
    let mut mir = mir_authorities(&mut mir_factories);
    let mut lir_factories = [RecordingLirFactory::with_output_provider(expected, wrong)];
    let mut lir = lir_authorities(&mut lir_factories);
    assert!(matches!(
        closure.with_checked_lir_layout_abi(&mut hir, &mut mir, &mut lir, |_| ()),
        Err(CrossConeLayoutLirSemanticClosureError::SourceProvider { provider, actual })
            if provider == expected && actual == wrong
    ));

    let closure = single_closure(&bytes);
    let mut hir = hir_authorities(&mut hir_bundles);
    let mut mir = mir_authorities(&mut mir_factories);
    let mut lir_factories = [RecordingLirFactory::rejecting_source(expected)];
    let mut lir = lir_authorities(&mut lir_factories);
    assert!(matches!(
        closure.with_checked_lir_layout_abi(&mut hir, &mut mir, &mut lir, |_| ()),
        Err(CrossConeLayoutLirSemanticClosureError::LayoutAbi { provider, source })
            if provider == expected && matches!(
                source.as_ref(),
                LayoutAbiSectionError::Source(LirSourceError::RejectedExports)
            )
    ));
}

#[test]
fn selected_and_recursive_layout_closure_corruption_are_rejected() {
    let (terminal_bytes, nominal) = nominal_artifact_bytes("lir-corrupt-terminal");
    let terminal = cone_named("lir-corrupt-terminal").identity();
    let relation = LayoutAbiDependencyV1::new(
        terminal,
        LayoutAbiSemanticTargetV1::ShapeSupport(nominal.owner()),
    );
    let consumer_bytes = artifact_bytes_with_lir_layout_abi(
        "lir-corrupt-selected-consumer",
        vec![dependency_record(&terminal_bytes)],
        encoded_layout_candidate(std::slice::from_ref(&relation)),
    );
    let (closure, identities) = two_provider_closure(&terminal_bytes, &consumer_bytes);
    let mut hir_bundles = nominal_hir_bundles(&nominal, identities);
    let mut hir = hir_authorities(&mut hir_bundles);
    let mut mir_factories = identities.map(RecordingMirFactory::new);
    let mut mir = mir_authorities(&mut mir_factories);
    let mut lir_factories = identities.map(RecordingLirFactory::new);
    let mut lir = lir_authorities(&mut lir_factories);
    assert!(matches!(
        closure.with_checked_lir_layout_abi(&mut hir, &mut mir, &mut lir, |_| ()),
        Err(CrossConeLayoutLirSemanticClosureError::LayoutAbi { provider, source })
            if provider == identities[1]
                && matches!(source.as_ref(), LayoutAbiSectionError::SelectedClosure)
    ));

    let consumer_bytes = artifact_bytes_with_dependencies(
        "lir-corrupt-recursive-consumer",
        vec![dependency_record(&terminal_bytes)],
    );
    let (closure, identities) = two_provider_closure(&terminal_bytes, &consumer_bytes);
    let mut hir_bundles = nominal_hir_bundles(&nominal, identities);
    let mut hir = hir_authorities(&mut hir_bundles);
    let mut mir_factories = identities.map(RecordingMirFactory::new);
    let mut mir = mir_authorities(&mut mir_factories);
    let mut lir_factories = [
        RecordingLirFactory::new(identities[0]),
        RecordingLirFactory::with_roots(identities[1], vec![relation]),
    ];
    let mut lir = lir_authorities(&mut lir_factories);
    assert!(matches!(
        closure.with_checked_lir_layout_abi(&mut hir, &mut mir, &mut lir, |_| ()),
        Err(CrossConeLayoutLirSemanticClosureError::LayoutAbi { provider, source })
            if provider == identities[1]
                && matches!(
                    source.as_ref(),
                    LayoutAbiSectionError::Semantic(LayoutAbiSemanticClosureError::MissingTarget(
                        actual
                    )) if *actual == relation.target()
                )
    ));
}

fn single_closure<'a>(bytes: &'a [u8]) -> crate::HirProductionValidatedCrossConeLayoutClosure<'a> {
    layout_hir_semantic_closure_for_test(
        ConeIdentity::CORE,
        target(),
        Vec::new(),
        vec![checked_artifact(bytes)],
        vec![vec![]],
    )
}

fn two_provider_closure<'a>(
    terminal_bytes: &'a [u8],
    consumer_bytes: &'a [u8],
) -> (
    crate::HirProductionValidatedCrossConeLayoutClosure<'a>,
    [ConeIdentity; 2],
) {
    let mut terminal = checked_artifact(terminal_bytes);
    let consumer =
        checked_artifact_with_authorities(consumer_bytes, [identity_graph(&mut terminal)]);
    let identities = [terminal.identity(), consumer.identity()];
    (
        layout_hir_semantic_closure_for_test(
            ConeIdentity::CORE,
            target(),
            vec![identities[1]],
            vec![terminal, consumer],
            vec![vec![], vec![0]],
        ),
        identities,
    )
}

fn nominal_hir_bundles(
    nominal: &NominalFixture,
    identities: [ConeIdentity; 2],
) -> [HirAuthorityBundle; 2] {
    [
        HirAuthorityBundle::nominal(nominal),
        HirAuthorityBundle::dependency(identities[1], nominal),
    ]
}

fn encoded_layout_candidate(selected: &[LayoutAbiDependencyV1]) -> Vec<u8> {
    struct Candidate<'a>(&'a [LayoutAbiDependencyV1]);
    impl WireEncode for Candidate<'_> {
        fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
            encoder.map(6)?;
            for field in 1..=5 {
                encoder.field(field)?;
                encoder.array(0)?;
            }
            encoder.field(6)?;
            encoder.map(2)?;
            encoder.field(1)?;
            encoder.array(self.0.len() as u64)?;
            for relation in self.0 {
                relation.encode(encoder)?;
            }
            encoder.field(2)?;
            encoder.array(0)
        }
    }
    encode(&Candidate(selected)).unwrap()
}
