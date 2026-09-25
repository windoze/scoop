use super::*;
use scoop_identity::{DeclarationName, ExactTypeKey};
use scoop_wire::{decode_canonical, encode};
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
            let foundation = production.foundation();
            let section = production.section();
            if case == "combined" {
                assert!(!section.protected_declarations().records().is_empty());
                assert_eq!(section.protected_defaults().records().len(), 1);
            }
            let graph = hir::CheckedNominalInheritanceGraphV1::validate_with_source_roots(
                foundation.local_inheritance_edges().iter(),
                foundation.source_roots().iter().copied(),
                foundation,
            )
            .unwrap();
            section
                .representation_support()
                .validate_source_semantics(foundation, &WirePath::root())
                .unwrap();
            let mut rows = Vec::new();
            for source in production.source_nominals().records() {
                let key = foundation.nominal_declaration_key(source.owner()).unwrap();
                let DeclarationName::Named(name) = key.name() else {
                    panic!("source name")
                };
                let exported = match source.owner() {
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
                        assert_eq!(graph.get(exact).is_some(), ready, "{name:?}");
                        ready
                    }
                    hir::SourceNominalId::GenericTemplate(_) => false,
                };
                assert!(
                    production
                        .source_roots()
                        .binary_search(&source.owner())
                        .is_ok()
                );
                let mut row = String::new();
                writeln!(row,
                    "{} {:?} {:?} binders={} supers={} fields={} constructors={} members={} machine={exported}",
                    name.as_str(), source.kind(), source.modality(), source.type_parameters().len_u32(),
                    source.supertypes().values().len(), source.source_shape().declared_fields().len(),
                    source.constructors().values().len(), source.members().values().len(),
                ).unwrap();
                rows.push(row);
            }
            rows.sort();
            snapshot(case, &rows.concat());
            let bytes = encode(production.source_nominals()).unwrap();
            let decoded: hir::DecodedCanonicalNominalSourceContractsV1 =
                decode_canonical(&bytes).unwrap();
            assert_eq!(
                decoded
                    .resolve(&mut source_inventory::identity_closure(output))
                    .unwrap(),
                *production.source_nominals()
            );
            for record in public.nominal_interfaces().records() {
                let contract = production
                    .source_nominals()
                    .get(record.declaration())
                    .unwrap();
                assert_eq!(contract.source_shape(), record.source_shape());
                assert_eq!(contract.supertypes(), record.exact_supertypes());
            }
        });
    }
}

#[test]
fn source_only_publication_still_rejects_missing_or_extra_machine_types() {
    with_hir_source(STANDALONE, |output, _| {
        let production =
            produce_cross_cone_type_semantics(output, &public_interface(output)).unwrap();
        let required = production.section().representation_support().records()[0].owner();
        let absent = hir::CanonicalNominalRepresentationSupportV1::try_new(Vec::new()).unwrap();
        assert!(matches!(
            absent.validate_source_semantics(production.foundation(),  &WirePath::root()),
            Err(hir::NominalRepresentationSourceSemanticError::Missing { owner }) if owner == required
        ));
        let foundation = production.foundation();
        let source = production
            .source_nominals()
            .records()
            .iter()
            .find(|source| source.kind() == hir::PublicNominalKindV1::Class)
            .unwrap();
        let hir::SourceNominalId::Concrete(owner) = source.owner() else {
            panic!("the deferred class has no own binders");
        };
        let extra = hir::NominalRepresentationSupportV1::try_new(
            foundation.nominal_declaration_key(source.owner()).unwrap(),
            foundation
                .nominal_access_source(source.owner())
                .unwrap()
                .clone(),
            hir::NominalRepresentationShapeV1::Class {
                base: scoop_identity::OptionalSignatureType::Absent,
                declared_fields: Vec::new(),
            },
        )
        .unwrap();
        let mut records = production
            .section()
            .representation_support()
            .records()
            .to_vec();
        records.push(extra);
        let extra = hir::CanonicalNominalRepresentationSupportV1::try_new(records).unwrap();
        assert!(matches!(
            extra.validate_source_semantics(foundation,  &WirePath::root()),
            Err(hir::NominalRepresentationSourceSemanticError::Extra { owner: extra, .. }) if extra == owner
        ));
    });
}

#[test]
fn source_only_machine_exports_ignore_unrelated_arena_allocation() {
    let project = |source: &str| {
        with_hir_source(source, |output, _| {
            let production =
                produce_cross_cone_type_semantics(output, &public_interface(output)).unwrap();
            let section = production.section();
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
