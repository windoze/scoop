use scoop_identity::ConeIdentity;
use scoop_mir::{
    MirBaseAndInterfacesV1, MirBaseClassV1, MirClassKindV1, MirGcKindV1, MirTypeBridgeAuthority,
    MirTypeBridgeDependencyV1, MirTypeBridgeSectionError, MirTypeBridgeSourceJoinError,
    MirTypeBridgeTargetV1, MirTypeFactsV1, MirTypeOriginV1, MirTypeRepresentationV1,
    MirValueKindV1, ParamFreeMirTypeExportV1,
};
use scoop_wire::{Encoder, WireEncode, encode};

use super::super::artifact::{
    artifact_bytes, artifact_bytes_with_dependencies, artifact_bytes_with_mir_type_bridge,
    checked_artifact, checked_artifact_with_authorities, dependency_record, identity_graph,
    nominal_artifact_bytes, target,
};
use super::super::type_authority::NominalFixture;
use super::support::*;
use crate::{
    CrossConeLayoutMirSemanticClosureError,
    layout_compile_closure::layout_hir_semantic_closure_for_test,
    strong_compile_decode::tests::cone_named,
};

#[test]
fn selected_type_resolves_to_the_terminal_provider_record() {
    let (preliminary, nominal) = nominal_artifact_bytes("mir-selected-terminal");
    let record = nominal_mir_export(&preliminary, &nominal);
    let semantics = nominal.section();
    let terminal_bytes = artifact_bytes_with_mir_type_bridge(
        "mir-selected-terminal",
        Vec::new(),
        Some(&nominal),
        Some(&semantics),
        encoded_candidate(std::slice::from_ref(&record), &[]),
    );
    let terminal = cone_named("mir-selected-terminal").identity();
    let relation =
        MirTypeBridgeDependencyV1::new(terminal, MirTypeBridgeTargetV1::Type(nominal.exact));
    let consumer_bytes = artifact_bytes_with_mir_type_bridge(
        "mir-selected-consumer",
        vec![dependency_record(&terminal_bytes)],
        None,
        None,
        encoded_candidate(&[], &[relation]),
    );
    let (closure, identities) = two_provider_closure(&terminal_bytes, &consumer_bytes);
    let mut hir_bundles = [
        HirAuthorityBundle::nominal(&nominal),
        HirAuthorityBundle::dependency(identities[1], &nominal),
    ];
    let mut hir = hir_authorities(&mut hir_bundles);
    let mut mir_factories = [
        RecordingMirFactory::with_type(identities[0], record),
        RecordingMirFactory::with_committed_use(identities[1], relation),
    ];
    let mut mir = mir_authorities(&mut mir_factories);

    closure
        .with_checked_mir_type_bridges(&mut hir, &mut mir, |checked| {
            let terminal = checked.provider(identities[0]).unwrap();
            let consumer = checked.provider(identities[1]).unwrap();
            let reference = consumer
                .type_bridge()
                .selected()
                .reference(identities[0], relation.target())
                .unwrap();
            let resolved = consumer.type_bridge().selected().resolve(reference);
            assert!(matches!(
                resolved,
                Some(scoop_mir::MirTypeBridgeSemanticRecordV1::Type(actual))
                    if std::ptr::eq(
                        actual,
                        terminal.type_bridge().types().get(nominal.exact).unwrap()
                    )
            ));
        })
        .unwrap();
}

#[test]
fn provider_and_returned_source_authority_must_match_the_artifact() {
    let bytes = artifact_bytes("mir-wrong-provider");
    let expected = cone_named("mir-wrong-provider").identity();
    let wrong = cone_named("mir-wrong-provider-other").identity();
    let closure = layout_hir_semantic_closure_for_test(
        ConeIdentity::CORE,
        target(),
        Vec::new(),
        vec![checked_artifact(&bytes)],
        vec![vec![]],
    );
    let mut hir_bundles = [HirAuthorityBundle::empty(expected)];
    let mut hir = hir_authorities(&mut hir_bundles);
    let mut wrong_factory = RecordingMirFactory::new(wrong);
    let mut mir = [crate::LayoutMirProviderSourceAuthorityV1::new(
        wrong,
        &mut wrong_factory,
    )];
    assert!(matches!(
        closure.with_checked_mir_type_bridges(&mut hir, &mut mir, |_| ()),
        Err(CrossConeLayoutMirSemanticClosureError::AuthorityProvider {
            position: 0,
            expected: actual_expected,
            actual,
        }) if actual_expected == expected && actual == wrong
    ));

    let closure = layout_hir_semantic_closure_for_test(
        ConeIdentity::CORE,
        target(),
        Vec::new(),
        vec![checked_artifact(&bytes)],
        vec![vec![]],
    );
    let mut hir = hir_authorities(&mut hir_bundles);
    let mut wrong_source = [RecordingMirFactory::with_source_provider(expected, wrong)];
    let mut mir = mir_authorities(&mut wrong_source);
    assert!(matches!(
        closure.with_checked_mir_type_bridges(&mut hir, &mut mir, |_| ()),
        Err(CrossConeLayoutMirSemanticClosureError::TypeBridge { provider, source })
            if provider == expected && matches!(
                source.as_ref(),
                MirTypeBridgeSectionError::SourceJoin(MirTypeBridgeSourceJoinError::Provider)
            )
    ));
}

#[test]
fn selected_and_recursive_closure_corruption_are_rejected() {
    let (terminal_bytes, nominal) = nominal_artifact_bytes("mir-corrupt-terminal");
    let terminal = cone_named("mir-corrupt-terminal").identity();
    let relation =
        MirTypeBridgeDependencyV1::new(terminal, MirTypeBridgeTargetV1::Type(nominal.exact));
    let selected_wire = encoded_candidate(&[], &[relation]);
    let consumer_bytes = artifact_bytes_with_mir_type_bridge(
        "mir-corrupt-consumer",
        vec![dependency_record(&terminal_bytes)],
        None,
        None,
        selected_wire,
    );
    let (closure, identities) = two_provider_closure(&terminal_bytes, &consumer_bytes);
    let mut hir_bundles = [
        HirAuthorityBundle::nominal(&nominal),
        HirAuthorityBundle::dependency(identities[1], &nominal),
    ];
    let mut hir = hir_authorities(&mut hir_bundles);
    let mut mir_factories = identities.map(RecordingMirFactory::new);
    let mut mir = mir_authorities(&mut mir_factories);
    let result = closure.with_checked_mir_type_bridges(&mut hir, &mut mir, |_| ());
    assert!(
        matches!(
            &result,
            Err(CrossConeLayoutMirSemanticClosureError::TypeBridge { provider, source })
                if *provider == identities[1]
                    && matches!(source.as_ref(), MirTypeBridgeSectionError::SelectedClosure)
        ),
        "{result:?}"
    );

    let consumer_bytes = artifact_bytes_with_dependencies(
        "mir-corrupt-recursive-consumer",
        vec![dependency_record(&terminal_bytes)],
    );
    let (closure, identities) = two_provider_closure(&terminal_bytes, &consumer_bytes);
    let mut hir_bundles = [
        HirAuthorityBundle::nominal(&nominal),
        HirAuthorityBundle::dependency(identities[1], &nominal),
    ];
    let mut hir = hir_authorities(&mut hir_bundles);
    let mut mir_factories = [
        RecordingMirFactory::new(identities[0]),
        RecordingMirFactory::with_committed_use(identities[1], relation),
    ];
    let mut mir = mir_authorities(&mut mir_factories);
    assert!(matches!(
        closure.with_checked_mir_type_bridges(&mut hir, &mut mir, |_| ()),
        Err(CrossConeLayoutMirSemanticClosureError::TypeBridge { provider, source })
            if provider == identities[1]
                && matches!(
                    source.as_ref(),
                    MirTypeBridgeSectionError::MissingDependency(target)
                        if *target == relation.target()
                )
    ));
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
            cone_named("mir-corrupt-current").identity(),
            target(),
            vec![identities[1]],
            vec![terminal, consumer],
            vec![vec![], vec![0]],
        ),
        identities,
    )
}

fn nominal_mir_export(bytes: &[u8], nominal: &NominalFixture) -> ParamFreeMirTypeExportV1 {
    let artifact = checked_artifact(bytes);
    let (mut artifact, _, _) = artifact.prepare_mir_semantics().unwrap();
    let parts = artifact.semantic_parts();
    ParamFreeMirTypeExportV1::try_new(
        MirTypeBridgeAuthority {
            identities: parts.identities,
            foundation: parts.mir_foundation,
        },
        nominal.exact,
        MirTypeOriginV1::SourceNominal(nominal.owner()),
        MirTypeFactsV1::try_new(
            MirValueKindV1::Reference,
            MirGcKindV1::ContainsManagedReferences,
        )
        .unwrap(),
        MirTypeRepresentationV1::Class {
            kind: MirClassKindV1::Final,
            declared_fields: Vec::new(),
        },
        MirBaseAndInterfacesV1 {
            base: MirBaseClassV1::None,
            interfaces: Vec::new(),
        },
    )
    .unwrap()
}

fn encoded_candidate(
    types: &[ParamFreeMirTypeExportV1],
    selected: &[MirTypeBridgeDependencyV1],
) -> Vec<u8> {
    struct Candidate<'a> {
        types: &'a [ParamFreeMirTypeExportV1],
        selected: &'a [MirTypeBridgeDependencyV1],
    }
    impl WireEncode for Candidate<'_> {
        fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
            encoder.map(7)?;
            encoder.field(1)?;
            encoder.array(self.types.len() as u64)?;
            for record in self.types {
                record.encode(encoder)?;
            }
            for field in 2..=6 {
                encoder.field(field)?;
                encoder.array(0)?;
            }
            encoder.field(7)?;
            encoder.array(self.selected.len() as u64)?;
            for relation in self.selected {
                relation.encode(encoder)?;
            }
            Ok(())
        }
    }
    encode(&Candidate { types, selected }).unwrap()
}
