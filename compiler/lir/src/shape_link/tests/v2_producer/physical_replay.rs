use super::*;
use scoop_identity::{PendingIdentityValidation, ValidatedIdentityGraph};
use scoop_wire::WireEncode;

mod fixtures;
mod negative;
mod wire;
use fixtures::{graph, produced_provider, view};

pub(super) fn check_join(
    provider: &Provider,
    layout: &CrossConeLayoutAbiSectionV1<'_>,
    foundation: &ConeLirFoundation,
) {
    let production = produced_provider(provider);
    let exports = provider.layout_section();
    let dependencies = [view(provider, &production, exports.exports())];
    let semantic = layout.selected().semantic_relations().collect::<Vec<_>>();
    let imports = layout.selected().physical_imports().records();
    let rows = imports
        .iter()
        .map(|value| value as &dyn WireEncode)
        .collect::<Vec<_>>();
    let replay = |rows: &[&dyn WireEncode], semantic: &[LayoutAbiDependencyV1]| {
        let mut identities = fixtures::consumer_graph(provider, semantic, layout.exports());
        wire::resolve(
            layout.exports(),
            semantic,
            rows,
            foundation,
            &mut identities,
        )
        .replay_physical_imports(&dependencies, &mut identities)
    };
    let checked = replay(&rows, &semantic).unwrap();
    assert_eq!(
        encode(checked.physical_imports()).unwrap(),
        encode(layout.selected().physical_imports()).unwrap()
    );
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
        let production = produced_provider(&provider);
        let exports = provider.layout_section();
        let view = view(&provider, &production, exports.exports());
        let consumer = ConeCoordinate::new("test", "physical-consumer", "1.0.0").unwrap();
        let module = consumer_module(&consumer);
        let expected = fixtures::imports(&provider, &view, module.cone);
        let bytes = encode(&expected).unwrap();
        let replay = || {
            let decoded: DecodedCanonicalExternalShapeLinkImportsV1 =
                decode_canonical(&bytes).unwrap();
            decoded.replay(
                module.cone,
                std::slice::from_ref(&view),
                &mut graph(&provider, &[]),
            )
        };

        let actual = replay().unwrap();
        assert_eq!(actual.records().len(), 10);
        assert_eq!(encode(&actual).unwrap(), bytes);
        negative::check(&provider, &view, module.cone, &expected);
    }
}
