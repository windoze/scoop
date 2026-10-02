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
            let actual = render(section.selected(), &identities);
            let snapshot = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
                "../../tests/fixtures/m23-hir-materialized-selections/{case}.hir.snap"
            ));
            if std::env::var_os("SCOOP_UPDATE_MATERIALIZED_SELECTION_SNAPSHOTS").is_some() {
                std::fs::write(&snapshot, &actual).unwrap();
            }
            assert_eq!(actual, std::fs::read_to_string(snapshot).unwrap());
            if case == "combined" {
                assert!(actual.contains("TypeTest String"));
                assert!(actual.contains("ShapeSupport String"));
                assert!(actual.contains("Signature Long"));
                assert!(!actual.contains("ULong"));
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

fn render(
    selected: &CanonicalSelectedExternalTypeUsesV1,
    identities: &ValidatedIdentityGraph,
) -> String {
    let mut rows = selected
        .records()
        .iter()
        .map(|record| {
            let kind = match record.usage() {
                SelectedTypeUseV1::Signature { .. } => "Signature",
                SelectedTypeUseV1::Representation { .. } => "Representation",
                SelectedTypeUseV1::TypeTest { .. } => "TypeTest",
                SelectedTypeUseV1::ShapeSupport { .. } => "ShapeSupport",
                SelectedTypeUseV1::MemberCall { .. } => "MemberCall",
                other => panic!("unexpected operation use in the fixture: {other:?}"),
            };
            format!(
                "{kind} {} provider=dependency\n",
                type_name(record.usage().exact(), identities)
            )
        })
        .collect::<Vec<_>>();
    rows.sort();
    rows.concat()
}

fn type_name(exact: PersistentExactTypeId, identities: &ValidatedIdentityGraph) -> String {
    let key = identities.canonical_key::<_, ExactTypeKey>(exact).unwrap();
    let ExactTypeKey::Nominal(owner) = key.as_ref() else {
        panic!("persistent type selection has a nominal target");
    };
    declaration_dump::nominal(hir::SourceNominalId::Concrete(*owner), identities)
}
