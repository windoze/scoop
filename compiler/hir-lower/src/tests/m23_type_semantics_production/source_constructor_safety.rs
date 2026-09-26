use super::source_dispatch::with_hir_source;
use super::*;
use scoop_identity::CallableTemplateOrigin;
use scoop_wire::{decode_canonical, encode};

#[test]
fn constructor_source_safety_matches_public_interfaces_and_survives_wire() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/m19-constructor-safety/safety.scoop"
    ));
    with_hir_source(source, |output, _| {
        let public =
            hir::CanonicalCallableInterfacesV1::from_export_hir(output.output().export.module())
                .unwrap();

        let section = produce_cross_cone_type_semantics(output, &public_interface(output)).unwrap();
        let sources = section.inheritance();
        let mut unsafe_count = 0;
        for record in sources
            .records()
            .iter()
            .flat_map(|owner| owner.constructors().records())
            .map(|record| record.source())
        {
            let callable = public
                .get(CallableTemplateOrigin::Constructor(record.declaration()))
                .unwrap();
            assert_eq!(callable.effects(), record.payload().effects());
            unsafe_count +=
                usize::from(record.payload().effects().safety() == hir::CallableSafetyV1::Unsafe);
        }
        assert_eq!(unsafe_count, 4);
        let bytes = encode(sources).unwrap();
        let decoded: hir::DecodedCanonicalNominalInheritanceInterfacesV1 =
            decode_canonical(&bytes).unwrap();
        let restored = decoded
            .resolve(&mut source_inventory::identity_closure(output))
            .unwrap();
        assert_eq!(sources, &restored);
        assert_eq!(encode(&restored).unwrap(), bytes);
    });
}
