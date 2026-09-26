use super::identity_support::{add_function, add_function_to};
use super::*;
use scoop_identity::{CallableOwner, DependencyCallableDeclarationId, SourceNominalKind};

pub(super) fn fixture() -> Fixture {
    let mut types =
        TypeFixture::with_source_provider("Unit", SourceNominalKind::Struct, ConeIdentity::CORE);
    let unit = ParamFreeMirTypeExportV1::try_new(
        types.authority(),
        types.payload.id(),
        MirTypeOriginV1::SourceNominal(types.empty.id()),
        types.empty_export().facts(),
        MirTypeRepresentationV1::Intrinsic(MirParamFreeIntrinsicV1::Unit),
        types.empty_export().base_and_interfaces().clone(),
    )
    .unwrap();
    let table = CanonicalParamFreeMirTypeExportsV1::try_new(vec![
        unit,
        types.boxed_export(),
        types.step_export(),
        types.slot_export(),
    ])
    .unwrap();
    let exports = exports(&types, ConeIdentity::CORE, table);
    let ordinary = crate::CrossConeMirBridgeSectionV1::try_new(
        ConeIdentity::CORE,
        &types.foundation,
        vec![],
        vec![],
    )
    .unwrap();
    let (cycle, _) = add_function_to(&mut types, ConeIdentity::CORE, "cycle");
    let production = crate::CoreBootstrapBridgeSectionV1::try_new(
        ConeIdentity::CORE,
        crate::EntryMirBridgeBranchV1::Library,
        crate::StrongCallableBridgeSurfaceV1::from_odr_free_foundation(&types.foundation)
            .with_initialization_cycle(cycle)
            .unwrap(),
    )
    .unwrap();
    Fixture {
        types,
        provider: ConeIdentity::CORE,
        exports,
        uses: Vec::new(),
        units: Vec::new(),
        production,
        ordinary,
    }
}

#[test]
fn core_shape_selection_uses_the_common_table_and_round_trips() {
    let provider = fixture();
    let mut consumer = Fixture::new("core-client");
    consumer.uses = vec![provider.shape_use()];
    let mut graph = graph(&[&provider, &consumer]);
    let core = provider.section(&[], &graph).unwrap();
    assert_eq!(core.shape_support().records().len(), 1);
    assert_eq!(
        core.shape_support().records()[0].source(),
        provider.types.empty.id()
    );
    let section = consumer.section(&[core.dependency_view()], &graph).unwrap();
    assert_eq!(section.selected().len(), 5);
    let bytes = encode(&core).unwrap();
    let decoded: DecodedCrossConeMirTypeBridgeSectionV1 = decode_canonical(&bytes).unwrap();
    let replayed = resolved_dependencies::read(&provider, decoded, &[], &mut graph).unwrap();
    assert_eq!(replayed.exports().types(), provider.exports.types());
}

#[test]
fn fixed_core_and_ordinary_callable_partitions_cannot_be_selected_again() {
    let core_provider = fixture();
    let mut ordinary = Fixture::new("ordinary-provider");
    let (function, signature) = add_function(&mut ordinary, "ordinary");
    ordinary.production = production(ordinary.provider, &ordinary.types.foundation);
    ordinary.ordinary = crate::CrossConeMirBridgeSectionV1::try_new(
        ordinary.provider,
        &ordinary.types.foundation,
        vec![
            crate::ParamFreeMirCallableExportV1::try_new(
                DependencyCallableDeclarationId::Function(function),
                StrongCallableDefinitionOwner::Function(function),
                signature,
            )
            .unwrap(),
        ],
        vec![],
    )
    .unwrap();
    let mut consumer = Fixture::new("partition-client");
    let graph = graph(&[&core_provider, &ordinary, &consumer]);
    let core = core_provider.section(&[], &graph).unwrap();
    let old = ordinary.section(&[], &graph).unwrap();
    let bridge = core_provider
        .production
        .strong_callable_bridges()
        .initialization_cycle()
        .unwrap();
    let CallableOwner::Function(cycle) = bridge.implementation() else {
        panic!("cycle function")
    };
    for (provider, target) in [
        (core_provider.provider, cycle),
        (ordinary.provider, function),
    ] {
        consumer.uses = vec![MirTypeBridgeDependencyV1::new(
            provider,
            MirTypeBridgeTargetV1::Callable(StrongCallableDefinitionOwner::Function(target)),
        )];
        assert!(
            matches!(consumer.section(&[core.dependency_view(), old.dependency_view()], &graph), Err(MirTypeBridgeSectionError::OldCallablePartition(actual)) if actual == StrongCallableDefinitionOwner::Function(target))
        );
    }
}

#[test]
fn mixed_core_and_ordinary_shape_selections_keep_their_terminal_records() {
    let core_provider = fixture();
    let ordinary_provider = Fixture::new("mixed-shape-provider");
    let mut consumer = Fixture::new("mixed-shape-client");
    consumer.uses = vec![core_provider.shape_use(), ordinary_provider.shape_use()];
    consumer.uses.sort_unstable();
    let mut graph = graph(&[&core_provider, &ordinary_provider, &consumer]);
    let core = core_provider.section(&[], &graph).unwrap();
    let ordinary = ordinary_provider.section(&[], &graph).unwrap();
    let section = consumer
        .section(
            &[core.dependency_view(), ordinary.dependency_view()],
            &graph,
        )
        .unwrap();
    assert_eq!(section.selected().len(), 10);
    for provider in [&core_provider, &ordinary_provider] {
        let reference = section
            .selected()
            .reference(provider.provider, provider.shape_use().target())
            .unwrap();
        let Some(MirTypeBridgeSemanticRecordV1::ShapeSupport(selected)) =
            section.selected().resolve(reference)
        else {
            panic!("shape selection resolves to its terminal shape record")
        };
        assert_eq!(selected.provider(), provider.provider);
        assert_eq!(selected.source(), provider.types.empty.id());
    }
    let bytes = encode(&section).unwrap();
    let decoded: DecodedCrossConeMirTypeBridgeSectionV1 = decode_canonical(&bytes).unwrap();
    let dependencies = [core.dependency_view(), ordinary.dependency_view()];
    let replayed =
        resolved_dependencies::read(&consumer, decoded, &dependencies, &mut graph).unwrap();
    replayed
        .replay_dependency_closure(&consumer.units, &dependencies, &consumer.uses, &graph)
        .unwrap();
    assert_eq!(
        replayed.selected_relations(),
        section.selected().relations().collect::<Vec<_>>()
    );
}
