use super::*;
use hir::{
    CanonicalSelectedExternalTypeUsesV1, SelectedExternalTypeUseV1, SelectedTypeUseV1,
    SharedTypeMetadataError,
};
use scoop_identity::{ExactTypeKey, ValidatedIdentityGraph};
use scoop_wire::WirePath;
use scoop_wire::{WireDecode, WireEncode, decode_canonical, encode};

mod validation;

const STANDALONE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-hir-materialized-selections/standalone.scoop"
));
const COMBINED: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-hir-materialized-selections/combined.scoop"
));

fn decoded<T: WireDecode>(value: &impl WireEncode) -> T {
    decode_canonical(&encode(value).unwrap()).unwrap()
}

#[test]
fn materialized_selections_are_produced_and_replayed_from_shared_hir_bytes() {
    for (case, source) in [("standalone", STANDALONE), ("combined", COMBINED)] {
        source_dispatch::with_hir_source(source, |output, _| {
            let mut public = public_interface(output);
            let production = produce_cross_cone_type_semantics(output, &public).unwrap();
            let mut identities = source_inventory::identity_closure(output);
            let section: hir::DecodedCrossConeTypeSemanticsSectionV1 = decoded(&production);
            let section = section.resolve(&mut identities, &WirePath::root()).unwrap();
            assert_eq!(section, production);
            let wire_public: hir::DecodedCrossConeHirInterfaceSectionV1 =
                decoded(&public.index_for_wire().unwrap());
            let wire_public = wire_public.resolve(&mut identities).unwrap();
            assert_eq!(wire_public, public);
            production::with_metadata(output, &wire_public, |metadata, dependencies, _| {
                metadata
                    .validate_materialized_type_uses(section.selected(), dependencies)
                    .unwrap();
                let expected = metadata.materialized_type_uses(dependencies).unwrap();
                assert_eq!(&expected, section.selected());
                assert!(
                    expected
                        .records()
                        .iter()
                        .all(|record| record.provider() == dependencies[0].provider)
                );
            });
            let records = section.selected().records();
            assert_eq!(records.len(), if case == "combined" { 21 } else { 14 });
            if case == "combined" {
                for expected in ["String", "Long"] {
                    let selected = records
                        .iter()
                        .filter(|record| type_name(record.usage().exact(), &identities) == expected)
                        .map(|record| record.usage())
                        .collect::<Vec<_>>();
                    assert!(
                        selected
                            .iter()
                            .any(|usage| matches!(usage, SelectedTypeUseV1::Signature { .. }))
                    );
                    if expected == "String" {
                        assert!(
                            selected
                                .iter()
                                .any(|usage| matches!(usage, SelectedTypeUseV1::TypeTest { .. }))
                        );
                        assert!(
                            selected.iter().any(|usage| matches!(
                                usage,
                                SelectedTypeUseV1::ShapeSupport { .. }
                            ))
                        );
                    }
                }
                assert!(
                    records
                        .iter()
                        .all(|record| type_name(record.usage().exact(), &identities) != "ULong")
                );
            }
        });
    }
}

#[test]
fn unused_generic_declarations_and_defaults_do_not_create_selected_type_roots() {
    let project = |source: &str| {
        source_dispatch::with_hir_source(source, |output, _| {
            let public = public_interface(output);
            let production = produce_cross_cone_type_semantics(output, &public).unwrap();
            encode(production.selected()).unwrap()
        })
    };
    let extra = concat!(
        "\npublic struct Deferred<T>(val payload: T, val other: UInt16)\n",
        "public fun <T> dormant(input: T, count: ULong = 1u): T = input\n"
    );
    assert_eq!(
        project(STANDALONE),
        project(&format!("{STANDALONE}{extra}"))
    );
}

fn type_name(exact: PersistentExactTypeId, identities: &ValidatedIdentityGraph) -> String {
    let key = identities.canonical_key::<_, ExactTypeKey>(exact).unwrap();
    let ExactTypeKey::Nominal(owner) = key.as_ref() else {
        panic!("persistent type selection has a nominal target");
    };
    declaration_dump::nominal(hir::SourceNominalId::Concrete(*owner), identities)
}
