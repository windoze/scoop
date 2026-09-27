use super::*;
use scoop_identity::{DeclarationName, SourceDeclarationKey, ValidatedIdentityGraph};
use scoop_wire::{WirePath, decode_canonical, encode};

mod core_extensions;
mod render;

#[test]
fn complete_shared_declarations_cover_nested_members_parameters_and_storage() {
    let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/m23-type-source-nominals");
    for case in [
        "declarations",
        "nested",
        "binding",
        "callables",
        "constructors",
        "parameters",
        "properties",
        "inheritance",
        "c-layout",
        "c-layout-single",
        "roots",
    ] {
        check_shared_declarations(&fixtures, case);
    }
}

#[test]
fn complete_shared_declarations_cover_protected_members_and_dispatch_signatures() {
    let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/m23-type-source-dispatch");
    for case in [
        "callables",
        "parameter-protocols",
        "properties",
        "property-direct",
        "protected-binding",
        "protected-callables",
        "protected-direct",
        "protected-signatures",
    ] {
        check_shared_declarations(&fixtures, case);
    }
}

fn check_shared_declarations(fixtures: &std::path::Path, case: &str) {
    let source = std::fs::read_to_string(fixtures.join(format!("{case}.scoop"))).unwrap();
    source_dispatch::with_hir_source(&source, |output, _| {
        let mut public = public_interface(output);
        let section = produce_cross_cone_type_semantics(output, &public)
            .unwrap_or_else(|error| panic!("{case}: {error:?}"));
        let mut identities = source_inventory::identity_closure(output);
        let restored: hir::DecodedCrossConeHirInterfaceSectionV1 =
            decode_canonical(&encode(&public.index_for_wire().unwrap()).unwrap()).unwrap();
        let restored = restored.resolve(&mut identities).unwrap();
        assert_eq!(restored, public, "{case}");
        let types: hir::DecodedCrossConeTypeSemanticsSectionV1 =
            decode_canonical(&encode(&section).unwrap()).unwrap();
        let types = types.resolve(&mut identities, &WirePath::root()).unwrap();
        assert_eq!(types, section, "{case}");
        assert!(
            restored.nominal_interfaces().declaration_count() > 0,
            "{case}"
        );
        production::with_metadata(output, &restored, |metadata, dependencies, _| {
            metadata
                .validate_materialized_type_uses(types.selected(), dependencies)
                .unwrap();
        });
        if matches!(case, "declarations" | "nested") {
            assert_eq!(
                render::nominals(restored.nominal_interfaces(), &identities),
                std::fs::read_to_string(fixtures.join(format!("{case}.snap"))).unwrap(),
                "{case}"
            );
        }
    });
}

fn name(
    id: impl scoop_identity::PersistentId + 'static,
    identities: &ValidatedIdentityGraph,
) -> String {
    let key = identities
        .canonical_key::<_, SourceDeclarationKey>(id)
        .unwrap();
    let DeclarationName::Named(name) = key.name() else {
        panic!("named declaration")
    };
    name.as_str().to_owned()
}
