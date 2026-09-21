use super::source_dispatch::with_hir_source;
use super::*;
use scoop_identity::CallableTemplateOrigin;
use scoop_wire::{decode_canonical, encode};

#[test]
fn constructor_nogc_source_effects_match_public_interfaces_and_survive_wire() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/m19-constructor-nogc/scalar.scoop"
    ));
    with_hir_source(source, |output, _| {
        let public =
            hir::CanonicalCallableInterfacesV1::from_export_hir(output.output().export.module())
                .unwrap();
        let mut meter = BudgetMeter::new(DecodeLimits::default());
        let sources =
            hir::CanonicalInheritanceSourceConstructorsV1::from_dependency_hir(output, &mut meter)
                .unwrap();
        let mut no_gc_count = 0;
        for record in sources.records() {
            let callable = public
                .get(CallableTemplateOrigin::Constructor(record.declaration()))
                .unwrap();
            assert_eq!(callable.effects(), record.payload().effects());
            no_gc_count += usize::from(
                record.payload().effects().gc_effect() == scoop_identity::GcEffect::NoGc,
            );
        }
        assert_eq!(no_gc_count, 6);
        let bytes = encode(&sources).unwrap();
        let decoded: hir::DecodedCanonicalInheritanceSourceConstructorsV1 =
            decode_canonical(&bytes, DecodeLimits::default()).unwrap();
        let restored = decoded
            .resolve(&mut source_inventory::identity_closure(output), &mut meter)
            .unwrap();
        assert_eq!(sources, restored);
        assert_eq!(encode(&restored).unwrap(), bytes);
    });
}
