use super::*;
use scoop_identity::DeclarationName;
use scoop_wire::{decode_canonical, encode};
use source_dispatch::with_hir_source;

const STANDALONE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-shared-nominal-declarations/standalone.scoop"
));
const COMBINED: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-shared-nominal-declarations/combined.scoop"
));

#[test]
fn shared_nominal_declarations_survive_ordinary_foundation_bytes_and_keep_lookup_separate() {
    for (case, source) in [("standalone", STANDALONE), ("combined", COMBINED)] {
        with_hir_source(source, |output, _| {
            let foundation = hir::CanonicalHirFoundation::from_dependency_output(output).unwrap();
            let table =
                hir::CanonicalNominalInterfacesV1::from_export_hir(output.output().export.module())
                    .unwrap();
            table
                .validate_declared_field_inventory(&foundation)
                .unwrap();
            table
                .validate_declared_relation_inventory(&foundation)
                .unwrap();
            let mut identities =
                source_inventory::identity_closure_for_foundation(output, foundation);
            let bytes = encode(&table).unwrap();
            let decoded: hir::DecodedCanonicalNominalInterfacesV1 =
                decode_canonical(&bytes).unwrap();
            assert_eq!(decoded.resolve(&mut identities).unwrap(), table);
            let mut rows = Vec::new();
            for record in table.all_records() {
                let key = match record.declaration() {
                    hir::SourceNominalId::Concrete(id) => identities
                        .canonical_key::<_, scoop_identity::SourceDeclarationKey>(id)
                        .unwrap(),
                    hir::SourceNominalId::GenericTemplate(id) => identities
                        .canonical_key::<_, scoop_identity::SourceDeclarationKey>(id)
                        .unwrap(),
                };
                let DeclarationName::Named(name) = key.name() else {
                    panic!("source name")
                };
                assert_ne!(name.as_str(), "Unrelated");
                let public = table.get(record.declaration()).is_some();
                let details = record.declaration_details();
                if !public {
                    assert!(record.constructors().is_empty());
                    assert!(record.members().members().is_empty());
                    assert!(record.nested_bindings().is_empty());
                }
                rows.push(format!(
                    "{} {:?} {:?} {:?} public={} binders={} supers={} fields={} constructors={} members={} children={}\n",
                    name.as_str(), record.kind(), details.modality(), details.declared_visibility(), public,
                    record.type_parameters().len_u32(), record.exact_supertypes().values().len(),
                    record.source_shape().declared_fields().len(), details.constructors().values().len(),
                    details.members().values().len(), details.children().values().len()));
            }
            rows.sort();
            snapshot(case, &rows.concat());
        });
    }
}

#[test]
fn shared_nominal_declarations_reject_omitted_private_relationships() {
    with_hir_source(COMBINED, |output, _| {
        let foundation = hir::CanonicalHirFoundation::from_dependency_output(output).unwrap();
        let original =
            hir::CanonicalNominalInterfacesV1::from_export_hir(output.output().export.module())
                .unwrap();
        for relation in ["constructor", "member", "child"] {
            let mut support = original.support_records().to_vec();
            let record = support
                .iter_mut()
                .find(|record| {
                    let details = record.declaration_details();
                    match relation {
                        "constructor" => !details.constructors().is_empty(),
                        "member" => !details.members().values().is_empty(),
                        "child" => !details.children().values().is_empty(),
                        _ => unreachable!(),
                    }
                })
                .unwrap();
            let owner = record.declaration();
            let details = record.declaration_details();
            let replacement = hir::NominalDeclarationDetailsV1::new(
                details.modality(),
                details.declared_visibility(),
                if relation == "constructor" {
                    hir::CanonicalPersistentIdsV1::empty()
                } else {
                    details.constructors().clone()
                },
                if relation == "member" {
                    hir::CanonicalNestedMemberRefsV1::try_new(vec![]).unwrap()
                } else {
                    details.members().clone()
                },
                if relation == "child" {
                    hir::CanonicalNestedNominalRefsV1::try_new(vec![]).unwrap()
                } else {
                    details.children().clone()
                },
                details.dispatch_order().clone(),
                details.dispatch_selections().clone(),
                if relation == "constructor" {
                    None
                } else {
                    details.primary_value_constructor()
                },
            );
            *record = replace_details(record, replacement);
            let corrupt = hir::CanonicalNominalInterfacesV1::with_support(
                original.records().to_vec(),
                support,
            )
            .unwrap();
            let error = corrupt
                .validate_declared_relation_inventory(&foundation)
                .unwrap_err();
            match (relation, error) {
                (
                    "constructor",
                    hir::NominalDeclarationInventoryError::MissingConstructor {
                        owner: actual, ..
                    },
                )
                | (
                    "member",
                    hir::NominalDeclarationInventoryError::MissingMember { owner: actual, .. },
                )
                | (
                    "child",
                    hir::NominalDeclarationInventoryError::MissingChild { owner: actual, .. },
                ) => assert_eq!(actual, owner),
                (_, error) => panic!("wrong declaration failure: {error}"),
            }
        }
    });
}

#[test]
fn shared_nominal_projection_ignores_unrelated_arena_entries() {
    let project = |source: &str| {
        with_hir_source(source, |output, _| {
            let export = output.output().export.module();

            let table = hir::CanonicalNominalInterfacesV1::from_export_hir(export).unwrap();

            encode(&table).unwrap()
        })
    };
    let prefix = "private class Unrelated {}\n";
    let padding = format!("//{}\n", " ".repeat(prefix.len() - 3));
    assert_eq!(
        project(&format!("{padding}{STANDALONE}")),
        project(&format!("{prefix}{STANDALONE}"))
    );
}

fn replace_details(
    record: &hir::NominalInterfaceRecordV1,
    details: hir::NominalDeclarationDetailsV1,
) -> hir::NominalInterfaceRecordV1 {
    hir::NominalInterfaceRecordV1::try_new(
        record.declaration(),
        record.kind(),
        record.type_parameters().clone(),
        record.exact_supertypes().clone(),
        record.constructors().clone(),
        record.members().clone(),
        record.nested_bindings().clone(),
        record.source_shape().clone(),
        details,
    )
    .unwrap()
}

fn snapshot(case: &str, actual: &str) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../../tests/fixtures/m23-shared-nominal-declarations/{case}.snap"
    ));
    if std::env::var_os("SCOOP_UPDATE_SHARED_NOMINAL_SNAPSHOTS").is_some() {
        std::fs::write(&path, actual).unwrap();
    }
    assert_eq!(actual, std::fs::read_to_string(path).unwrap());
}
