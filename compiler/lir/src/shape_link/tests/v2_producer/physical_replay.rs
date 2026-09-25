use super::*;
use scoop_identity::{PendingIdentityValidation, ValidatedIdentityGraph};
use scoop_wire::WireEncode;
use std::convert::Infallible;

mod fixtures;
mod negative;
mod wire;
use fixtures::{graph, replayed_provider, support, view};

pub(super) fn check_join(
    provider: &Provider,
    consumer: &ReplayedStrongProductionSectionV2,
    layout: &CrossConeLayoutAbiSectionV1<'_>,
) {
    let production = replayed_provider(provider);
    let exports = provider.layout_section();
    let dependencies = [view(provider, &production, exports.exports())];
    let semantic = layout.selected().semantic_relations().collect::<Vec<_>>();
    let imports = layout.selected().physical_imports().records();
    let rows = imports
        .iter()
        .map(|value| value as &dyn WireEncode)
        .collect::<Vec<_>>();
    let replay = |rows: &[&dyn WireEncode], semantic: &[LayoutAbiDependencyV1]| {
        let mut identities = graph(provider, semantic);
        wire::resolve(layout.exports(), semantic, rows, &mut identities)
            .replay_physical_imports::<Infallible>(
                consumer.canonical_definitions(),
                &dependencies,
                &provider.initialization_support(),
                &mut identities,
            )
    };
    let checked = replay(&rows, &semantic).unwrap();
    consumer
        .validate_replayed_layout_selection(&checked)
        .unwrap();
    assert_eq!(
        encode(checked.physical_imports()).unwrap(),
        encode(layout.selected().physical_imports()).unwrap()
    );
    for omitted in 0..rows.len() {
        let incomplete = rows
            .iter()
            .enumerate()
            .filter_map(|(index, row)| (index != omitted).then_some(*row))
            .collect::<Vec<_>>();
        let checked = replay(&incomplete, &semantic).unwrap();
        assert!(matches!(
            consumer.validate_replayed_layout_selection(&checked),
            Err(
                StrongProductionLayoutJoinError::MissingPhysicalDescriptor { .. }
                    | StrongProductionLayoutJoinError::MissingPhysicalCallable { .. }
                    | StrongProductionLayoutJoinError::MissingPhysicalInitialization { .. }
            )
        ));
    }
    assert!(matches!(
        replay(&rows, &[]),
        Err(LayoutAbiSectionError::MissingPhysicalSemantic(_))
    ));
    let mut reversed = rows.clone();
    reversed.reverse();
    assert!(matches!(
        replay(&reversed, &semantic),
        Err(LayoutAbiSectionError::Physical(ShapeLinkError::Order))
    ));
    let duplicates = [&rows[..1], &rows].concat();
    assert!(matches!(
        replay(&duplicates, &semantic),
        Err(LayoutAbiSectionError::Physical(ShapeLinkError::Order))
    ));
}

#[test]
fn shared_physical_contract_replay_covers_all_ten_subjects() {
    for coordinate in [
        ConeCoordinate::reserved_core(),
        ConeCoordinate::new("test", "physical-provider", "1.0.0").unwrap(),
    ] {
        let provider = Provider::for_coordinate(coordinate);
        let production = replayed_provider(&provider);
        let exports = provider.layout_section();
        let view = view(&provider, &production, exports.exports());
        let consumer = ConeCoordinate::new("test", "physical-consumer", "1.0.0").unwrap();
        let module = consumer_module(&consumer);
        let foundation = OdrFreeLirFoundation::from_module(&module).unwrap();
        let definitions =
            StrongObjectSymbolSurfaceV1::from_odr_free_foundation(&foundation).unwrap();
        let support = support(&provider);
        let expected = fixtures::imports(&provider, &view, module.cone, &definitions, &support);
        let bytes = encode(&expected).unwrap();
        let replay = || {
            let decoded: DecodedCanonicalExternalShapeLinkImportsV1 =
                decode_canonical(&bytes).unwrap();
            decoded.replay(
                module.cone,
                &definitions,
                std::slice::from_ref(&view),
                &support,
                &mut graph(&provider, &[]),
            )
        };

        let actual = replay().unwrap();
        assert_eq!(actual.records().len(), 10);
        assert_eq!(encode(&actual).unwrap(), bytes);
        negative::check(
            &provider,
            &view,
            module.cone,
            &definitions,
            &support,
            &expected,
        );
    }
}
