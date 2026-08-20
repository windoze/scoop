use super::*;
use scoop_identity::{DeclarationName, ExactTypeKey, SourceDeclarationKey};
use scoop_wire::encode;
use source_dispatch::with_hir_source;

const STANDALONE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-source-only-nominals/standalone.scoop"
));
const COMBINED: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-source-only-nominals/combined.scoop"
));

#[test]
fn closed_parent_nominals_preserve_declarations_and_close_machine_dependencies() {
    for (case, source) in [("standalone", STANDALONE), ("combined", COMBINED)] {
        with_hir_source(source, |output, _| {
            let public = public_interface(output);
            let production = produce_cross_cone_type_semantics(output, &public).unwrap();
            let identities = source_inventory::identity_closure(output);
            let section = &production;
            if case == "combined" {
                assert!(public.nominal_interfaces().all_records().any(|record| {
                    record.declaration_details().declared_visibility()
                        == hir::DeclaredVisibilityV1::Protected
                }));
                assert_eq!(public.default_templates().records().len(), 1);
            }
            for source in public.nominal_interfaces().all_records() {
                let key = match source.declaration() {
                    hir::SourceNominalId::Concrete(id) => identities
                        .canonical_key::<_, SourceDeclarationKey>(id)
                        .unwrap(),
                    hir::SourceNominalId::GenericTemplate(id) => identities
                        .canonical_key::<_, SourceDeclarationKey>(id)
                        .unwrap(),
                };
                let DeclarationName::Named(name) = key.name() else {
                    panic!("source name")
                };
                match source.declaration() {
                    hir::SourceNominalId::Concrete(owner) => {
                        let exact =
                            PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(owner)).unwrap();
                        let ready = true;
                        assert_eq!(
                            section.representation_support().get(owner).is_some(),
                            ready,
                            "{name:?}"
                        );
                        assert_eq!(
                            section.inheritance().get(exact).is_some(),
                            ready,
                            "{name:?}"
                        );
                        assert_eq!(
                            section.exact_facts().get(exact).is_some(),
                            ready,
                            "{name:?}"
                        );
                    }
                    hir::SourceNominalId::GenericTemplate(_) => continue,
                };
            }
        });
    }
}

#[test]
fn source_only_machine_exports_ignore_unrelated_arena_allocation() {
    let project = |source: &str| {
        with_hir_source(source, |output, _| {
            let production =
                produce_cross_cone_type_semantics(output, &public_interface(output)).unwrap();
            let section = &production;
            (
                encode(section.representation_support()).unwrap(),
                encode(section.inheritance()).unwrap(),
                encode(section.exact_facts()).unwrap(),
            )
        })
    };
    let prefix = "private class Unrelated {}\n";
    // Keep source spans fixed while inserting an unrelated arena entry.
    let padding = format!("//{}\n", " ".repeat(prefix.len() - 3));
    assert_eq!(
        project(&format!("{padding}{STANDALONE}")),
        project(&format!("{prefix}{STANDALONE}"))
    );
}
