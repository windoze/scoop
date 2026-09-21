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
    let source = Source::new(
        ConeIdentity::CORE,
        vec![types.empty.id()],
        exports(&types, ConeIdentity::CORE, table),
    );
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
        source,
        production,
        ordinary,
    }
}

#[test]
fn core_shape_selection_uses_the_common_table_and_round_trips() {
    let provider = fixture();
    let mut consumer = Fixture::new("core-client");
    consumer.source.uses = vec![provider.shape_use()];
    let mut graph = graph(&[&provider, &consumer]);
    let core = provider.section(&[], &graph).unwrap();
    assert_eq!(core.shape_support().records().len(), 1);
    assert_eq!(
        core.shape_support().records()[0].source(),
        provider.types.empty.id()
    );
    let section = consumer.section(&[&core], &graph).unwrap();
    assert_eq!(section.selected().len(), 5);
    let bytes = encode(&core).unwrap();
    let decoded: DecodedCrossConeMirTypeBridgeSectionV1 =
        decode_canonical(&bytes, DecodeLimits::default()).unwrap();
    let replayed = decoded
        .validate(
            provider.authority(),
            &[],
            &provider.source,
            &mut graph,
            &mut meter(),
        )
        .unwrap();
    assert_eq!(encode(&replayed).unwrap(), bytes);
}

#[test]
fn fixed_core_and_ordinary_callable_partitions_cannot_be_selected_again() {
    let core_provider = fixture();
    let mut ordinary = Fixture::new("ordinary-provider");
    let (function, signature) = add_function(&mut ordinary, "ordinary");
    ordinary.production = production(ordinary.source.provider, &ordinary.types.foundation);
    ordinary.ordinary = crate::CrossConeMirBridgeSectionV1::try_new(
        ordinary.source.provider,
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
        (core_provider.source.provider, cycle),
        (ordinary.source.provider, function),
    ] {
        consumer.source.uses = vec![MirTypeBridgeDependencyV1::new(
            provider,
            MirTypeBridgeTargetV1::Callable(StrongCallableDefinitionOwner::Function(target)),
        )];
        assert!(
            matches!(consumer.section(&[&core, &old], &graph), Err(MirTypeBridgeSectionError::OldCallablePartition(actual)) if actual == StrongCallableDefinitionOwner::Function(target))
        );
    }
}

#[test]
fn mixed_core_and_ordinary_shape_selections_keep_their_terminal_records() {
    let core_provider = fixture();
    let ordinary_provider = Fixture::new("mixed-shape-provider");
    let mut consumer = Fixture::new("mixed-shape-client");
    consumer.source.uses = vec![core_provider.shape_use(), ordinary_provider.shape_use()];
    consumer.source.uses.sort_unstable();
    let mut graph = graph(&[&core_provider, &ordinary_provider, &consumer]);
    let core = core_provider.section(&[], &graph).unwrap();
    let ordinary = ordinary_provider.section(&[], &graph).unwrap();
    let section = consumer.section(&[&core, &ordinary], &graph).unwrap();
    assert_eq!(section.selected().len(), 10);
    for provider in [&core_provider, &ordinary_provider] {
        let reference = section
            .selected()
            .reference(provider.source.provider, provider.shape_use().target())
            .unwrap();
        let Some(MirTypeBridgeSemanticRecordV1::ShapeSupport(selected)) =
            section.selected().resolve(reference)
        else {
            panic!("shape selection resolves to its terminal shape record")
        };
        assert_eq!(selected.provider(), provider.source.provider);
        assert_eq!(selected.source(), provider.types.empty.id());
    }
    let bytes = encode(&section).unwrap();
    let decoded: DecodedCrossConeMirTypeBridgeSectionV1 =
        decode_canonical(&bytes, DecodeLimits::default()).unwrap();
    let replayed = decoded
        .validate(
            consumer.authority(),
            &[&core, &ordinary],
            &consumer.source,
            &mut graph,
            &mut meter(),
        )
        .unwrap();
    assert_eq!(encode(&replayed).unwrap(), bytes);
}
