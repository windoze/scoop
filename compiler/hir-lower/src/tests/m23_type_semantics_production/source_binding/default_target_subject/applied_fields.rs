use super::*;
use hir::{DefaultFieldRefV1 as Field, DefaultSourceFieldAccessSubjectV1 as AccessSubject};
use scoop_identity::{CoreBuiltinNominal, PersistentTypeId, SignatureTypeKey};
mod rejection;
mod resources;
const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-defaults/field-targets.scoop"
));
const COMBINATIONS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-defaults/field-target-combinations.scoop"
));
const CASES: &[(&str, u32)] = &[
    ("packet", 1),
    ("Cell.read", 0),
    ("Cache.read", 0),
    ("tuple", 1),
];
const COMBINED_CASES: &[(&str, u32)] = &[
    ("Envelope.packet", 1),
    ("Envelope.Cell.read", 0),
    ("Envelope.Cache.read", 0),
    ("Envelope.tuple", 1),
];
fn reference(
    output: &hir::DependencyHirOutput,
    name: &str,
    position: u32,
) -> hir::DefaultSourceReferenceV1<Field> {
    let export = output.output().export.module();
    let id = export
        .functions
        .iter()
        .find(|(_, f)| f.name == name)
        .unwrap()
        .0;
    let body = hir::DefaultSourceBodyProductionV1::from_dependency_hir(
        output,
        hir::ExportParameterOwner::Function(id),
        position,
        &mut meter(),
    )
    .unwrap();
    assert_eq!(body.references().fields().len(), 1, "{name}");
    body.references().fields()[0].clone()
}
fn with_owner(target: &Field, owner_type: SignatureTypeKey) -> Field {
    match target {
        Field::Struct { declaration, .. } => Field::Struct {
            declaration: *declaration,
            owner_type,
        },
        Field::Class { declaration, .. } => Field::Class {
            declaration: *declaration,
            owner_type,
        },
        Field::Tuple { .. } => panic!("declared field"),
    }
}
#[test]
fn applied_default_field_subjects_match_sealed_source_domains() {
    for (source, cases) in [(SOURCE, CASES), (COMBINATIONS, COMBINED_CASES)] {
        with_hir_source(source, |output, _| {
            let fixture = Fixture::from_output(output);
            let foundation = fixture.bind().unwrap();
            let records = cases
                .iter()
                .map(|(name, position)| (*name, reference(output, name, *position)))
                .collect::<Vec<_>>();
            let mut required = BTreeSet::new();
            for (_, record) in &records {
                if let AccessSubject::Declaration(subject) = foundation
                    .default_field_access_subject(record.target(), &mut meter())
                    .unwrap()
                {
                    required.insert(subject);
                }
            }
            assert_eq!(required.len(), 3);
            let table =
                Table::from_export_hir(&output.output().export, &required, &mut meter()).unwrap();
            let bound = foundation
                .bind_default_access_declarations(&table, &required, &mut meter())
                .unwrap();
            let mut snapshot = String::new();
            for (name, record) in &records {
                match foundation
                    .default_field_access_subject(record.target(), &mut meter())
                    .unwrap()
                {
                    AccessSubject::Declaration(subject) => {
                        let domain = bound.source_lookup_domain(subject, &mut meter()).unwrap();
                        assert_eq!(&domain, record.witness().target_domain(), "{name}");
                        let role = match subject {
                            Subject::Type(_) => "Type",
                            Subject::GenericType(_) => "GenericType",
                            Subject::Property(_) => "Property",
                            _ => panic!("field access declaration"),
                        };
                        snapshot.push_str(&format!("{name}: {role} {}\n", summary(&domain)));
                    }
                    AccessSubject::TupleElement { declaration_index } => {
                        assert_eq!(declaration_index, 0);
                        snapshot.push_str(&format!("{name}: TupleElement 0\n"));
                    }
                }
            }
            assert_eq!(
                snapshot,
                if source == SOURCE {
                    include_str!(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/../../tests/fixtures/m23-type-source-defaults/field-targets.snap"
                    ))
                } else {
                    include_str!(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/../../tests/fixtures/m23-type-source-defaults/field-target-combinations.snap"
                    ))
                }
            );
        });
    }
}
