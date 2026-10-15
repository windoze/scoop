use super::*;
use scoop_identity::{PendingIdentityValidation, ValidatedIdentityGraph};

mod fixtures;
use fixtures::{graph, produced_provider, view};

#[test]
fn saved_physical_imports_cover_all_ten_subjects() {
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
            decoded.read(
                module.cone,
                std::slice::from_ref(&view),
                &mut graph(&provider, &[]),
            )
        };

        let actual = replay().unwrap();
        assert_eq!(actual.records().len(), 10);
        assert_eq!(encode(&actual).unwrap(), bytes);
    }
}
