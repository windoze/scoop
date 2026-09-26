use super::*;
use scoop_identity::{DeclarationName, ExactTypeKey, SourceDeclarationKey};
use scoop_wire::encode;
use source_dispatch::with_hir_source;
use std::fmt::Write;

const STANDALONE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-source-only-nominals/standalone.scoop"
));
const COMBINED: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-source-only-nominals/combined.scoop"
));

#[test]
fn source_only_nominals_preserve_complete_declarations_and_close_machine_dependencies() {
    for (case, source) in [("standalone", STANDALONE), ("combined", COMBINED)] {
        with_hir_source(source, |output, _| {
            let public = public_interface(output);
            let production = produce_cross_cone_type_semantics(output, &public).unwrap();
            let identities = source_inventory::identity_closure(output);
            let section = &production;
            if case == "combined" {
                assert!(!section.protected_declarations().records().is_empty());
                assert_eq!(public.default_templates().records().len(), 1);
            }
            let mut rows = Vec::new();
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
                let exported = match source.declaration() {
                    hir::SourceNominalId::Concrete(owner) => {
                        let exact =
                            PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(owner)).unwrap();
                        let ready = name.as_str().starts_with("Ready");
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
                        ready
                    }
                    hir::SourceNominalId::GenericTemplate(_) => false,
                };
                let mut row = String::new();
                writeln!(row,
                    "{} {:?} {:?} binders={} supers={} fields={} constructors={} members={} machine={exported}",
                    name.as_str(), source.kind(), source.declaration_details().modality(), source.type_parameters().len_u32(),
                    source.exact_supertypes().values().len(), source.source_shape().declared_fields().len(),
                    source.declaration_details().constructors().values().len(), source.declaration_details().members().values().len(),
                ).unwrap();
                rows.push(row);
            }
            rows.sort();
            snapshot(case, &rows.concat());
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

fn snapshot(case: &str, actual: &str) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../../tests/fixtures/m23-source-only-nominals/{case}.snap"
    ));
    if std::env::var_os("SCOOP_UPDATE_SOURCE_ONLY_SNAPSHOTS").is_some() {
        std::fs::write(&path, actual).unwrap();
    }
    assert_eq!(actual, std::fs::read_to_string(path).unwrap());
}
