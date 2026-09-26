use super::source_dispatch::with_hir_source;
use super::*;
use scoop_identity::CallableTemplateOrigin;
use scoop_wire::{decode_canonical, encode};

#[test]
fn constructor_safety_survives_shared_declaration_wire() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/m19-constructor-safety/safety.scoop"
    ));
    with_hir_source(source, |output, _| {
        let public = public_interface(output);
        let sources = public.callable_interfaces();
        assert_eq!(
            sources
                .all_declarations()
                .filter(|record| {
                    matches!(record.declaration(), CallableTemplateOrigin::Constructor(_))
                        && record.effects().safety() == hir::CallableSafetyV1::Unsafe
                })
                .count(),
            4
        );
        let bytes = encode(sources).unwrap();
        let decoded: hir::DecodedCanonicalCallableInterfacesV1 = decode_canonical(&bytes).unwrap();
        let restored = decoded
            .resolve(&mut source_inventory::identity_closure(output))
            .unwrap();
        assert_eq!(sources, &restored);
        assert_eq!(encode(&restored).unwrap(), bytes);
    });
}
