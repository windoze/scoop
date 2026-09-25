use super::*;
use hir::SourceNominalId;
use hir::{DefaultClassConstructorIdV1 as ClassId, DefaultConstructorRefV1 as Constructor};
use scoop_identity::SignatureTypeKey;
mod adapters;
mod rejection;
const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-defaults/constructor-targets.scoop"
));
const COMBINATIONS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-defaults/constructor-target-combinations.scoop"
));
const CASES: &[(&str, u32)] = &[
    ("packet", 0),
    ("packetSecondary", 0),
    ("cell", 0),
    ("Cell.nested", 0),
    ("choice", 0),
    ("Hidden.nested", 0),
];
const COMBINED_CASES: &[(&str, u32)] = &[
    ("Envelope.packet", 1),
    ("Envelope.cell", 1),
    ("Envelope.choice", 1),
    ("Envelope.nested", 0),
];
pub(super) fn reference(
    output: &hir::DependencyHirOutput,
    name: &str,
    position: u32,
) -> hir::DefaultSourceReferenceV1<Constructor> {
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
    )
    .unwrap();
    assert_eq!(body.references().constructors().len(), 1, "{name}");
    body.references().constructors()[0].clone()
}
fn with_owner(target: &Constructor, owner_type: SignatureTypeKey) -> Constructor {
    match target {
        Constructor::Struct { declaration, .. } => Constructor::Struct {
            declaration: *declaration,
            owner_type,
        },
        Constructor::Class { declaration, .. } => Constructor::Class {
            declaration: *declaration,
            owner_type,
        },
        Constructor::Variant { declaration, .. } => Constructor::Variant {
            declaration: *declaration,
            owner_type,
        },
    }
}

#[test]
fn default_constructor_subjects_replay_actual_source_reference_domains() {
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
                required.insert(
                    foundation
                        .default_constructor_access_subject(record.target())
                        .unwrap(),
                );
            }
            let table = Table::from_export_hir(&output.output().export, &required).unwrap();
            let bound = foundation
                .bind_default_access_declarations(&table, &required)
                .unwrap();
            let mut snapshot = String::new();
            for (name, record) in &records {
                let subject = foundation
                    .default_constructor_access_subject(record.target())
                    .unwrap();
                let domain = bound.source_lookup_domain(subject).unwrap();
                assert_eq!(&domain, record.witness().target_domain(), "{name}");
                let role = match subject {
                    Subject::Constructor(_) => "Constructor",
                    Subject::Type(_) => "Type",
                    Subject::GenericType(_) => "GenericType",
                    _ => panic!("constructor access declaration"),
                };
                snapshot.push_str(&format!("{name}: {role} {}\n", summary(&domain)));
            }
            if source == SOURCE {
                assert_eq!(
                    snapshot,
                    include_str!(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/../../tests/fixtures/m23-type-source-defaults/constructor-targets.snap"
                    ))
                );
                assert_eq!(required.len(), CASES.len());
            } else {
                assert!(records.iter().filter(|(name, _)| *name != "Envelope.nested").all(|(_, r)| matches!(r.target().owner_type(), SignatureTypeKey::NominalApplication { arguments, .. } if arguments.as_slice().len() == 1)));
                assert!(matches!(
                    records.last().unwrap().1.target().owner_type(),
                    SignatureTypeKey::Nominal(_)
                ));
                assert!(snapshot.lines().all(|line| line.ends_with("generic 1")));
            }
        });
    }
}
