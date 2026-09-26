use super::*;
use crate::tests::m23_type_semantics_production::source_dispatch::with_hir_sources;

const MEMBERS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-protected-production/members.scoop"
));
const NESTED: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-protected-production/nested.scoop"
));

#[test]
fn complete_type_section_publishes_protected_members_constructors_and_nested_sources() {
    for (source, name, expected) in [
        (
            MEMBERS,
            "members",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m23-type-protected-production/members.scoop.snap"
            )),
        ),
        (
            NESTED,
            "nested",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m23-type-protected-production/nested.scoop.snap"
            )),
        ),
    ] {
        with_hir_source(source, |output, _| {
            let produced =
                produce_cross_cone_type_semantics(output, &public_interface(output)).unwrap();
            let mut fixture = Fixture::from_output(output);
            let bytes = encode(produced.section()).unwrap();
            let decoded: hir::DecodedCrossConeTypeSemanticsSectionV1 =
                decode_canonical(&bytes).unwrap();
            let section = decoded
                .resolve(&mut fixture.identities, &WirePath::root())
                .unwrap();
            assert_eq!(encode(&section).unwrap(), bytes);
            let inventory =
                hir::CanonicalSourceInheritanceInventoriesV1::from_dependency_hir(output).unwrap();
            for record in section.inheritance().records() {
                let source = inventory.get(record.owner()).unwrap();
                assert_eq!(record.protected_members(), source.protected_members());
                assert_eq!(
                    record
                        .constructors()
                        .records()
                        .iter()
                        .map(|c| c.declaration())
                        .collect::<Vec<_>>(),
                    source.constructors().values()
                );
            }
            let foundation = fixture.bind().unwrap();
            section
                .representation_support()
                .validate_source_semantics(produced.foundation(), &WirePath::root())
                .unwrap();
            let dump = outline(&foundation, section.protected_declarations());
            if std::env::var_os("SCOOP_UPDATE_PROTECTED_PRODUCTION_SNAPSHOTS").is_some() {
                let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
                    "../../tests/fixtures/m23-type-protected-production/{name}.scoop.snap"
                ));
                std::fs::write(path, dump).unwrap();
            } else {
                assert_eq!(dump, expected);
            }
        });
    }
}

#[test]
fn protected_type_section_is_stable_across_unrelated_arena_allocation() {
    let produce = |files: &[(&str, &str)]| {
        with_hir_sources(files, |output, _| {
            let production =
                produce_cross_cone_type_semantics(output, &public_interface(output)).unwrap();
            encode(production.section()).unwrap()
        })
    };
    let first = produce(&[("src/main.scoop", NESTED)]);
    let second = produce(&[
        ("src/a-unused.scoop", "private fun unrelated(): Int = 0"),
        ("src/main.scoop", NESTED),
    ]);
    assert!(
        first == second,
        "type section bytes changed with unrelated arena allocation"
    );
}

#[test]
fn protected_type_production_publishes_the_required_default_body() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/m23-type-protected-production/default.scoop"
    ));
    with_hir_source(source, |output, _| {
        let interface = public_interface(output);
        let defaults = interface.default_templates().records();
        assert_eq!(defaults.len(), 1);
        assert!(matches!(
            defaults[0].key().owner(),
            CallableTemplateOrigin::Constructor(_)
        ));
        assert_eq!(
            defaults[0].definition_root().declaration(),
            defaults[0].key().owner()
        );
        assert_eq!(
            defaults[0].body().value().result_type(),
            defaults[0].result()
        );
    });
}
