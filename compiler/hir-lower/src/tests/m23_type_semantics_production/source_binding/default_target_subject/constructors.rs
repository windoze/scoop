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
    output: &hir::OrdinaryHirOutput<'_>,
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
    let body = hir::DefaultSourceBodyProductionV1::from_ordinary_hir(
        output,
        hir::ExportParameterOwner::Function(id),
        position,
        &mut meter(),
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
                        .default_constructor_access_subject(record.target(), &mut meter())
                        .unwrap(),
                );
            }
            let table =
                Table::from_export_hir(&output.output().export, &required, &mut meter()).unwrap();
            let bound = foundation
                .bind_default_access_declarations(&table, &required, &mut meter())
                .unwrap();
            let mut snapshot = String::new();
            for (name, record) in &records {
                let subject = foundation
                    .default_constructor_access_subject(record.target(), &mut meter())
                    .unwrap();
                let domain = bound.source_lookup_domain(subject, &mut meter()).unwrap();
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

#[test]
fn default_constructor_subject_queries_share_work_node_edge_and_shape_budgets() {
    with_hir_source(COMBINATIONS, |output, _| {
        let fixture = Fixture::from_output(output);
        let foundation = fixture.bind().unwrap();
        for (name, position) in COMBINED_CASES {
            let record = reference(output, name, *position);
            let target = record.target();
            let mut measured = meter();
            foundation
                .default_constructor_access_subject(target, &mut measured)
                .unwrap();
            let mut shared = BudgetMeter::new(DecodeLimits {
                validation_work_units: measured.usage().validation_work_units * 2 - 1,
                ..DecodeLimits::default()
            });
            foundation
                .default_constructor_access_subject(target, &mut shared)
                .unwrap();
            assert!(matches!(
                foundation.default_constructor_access_subject(target, &mut shared),
                Err(Error::Resource(_))
            ));
            for limits in [
                DecodeLimits {
                    validation_work_units: 0,
                    ..DecodeLimits::default()
                },
                DecodeLimits {
                    decoded_nodes: 0,
                    ..DecodeLimits::default()
                },
                DecodeLimits {
                    decoded_edges: 0,
                    ..DecodeLimits::default()
                },
                DecodeLimits {
                    semantic_table_entries: 0,
                    ..DecodeLimits::default()
                },
                DecodeLimits {
                    semantic_recursion: 0,
                    ..DecodeLimits::default()
                },
                DecodeLimits {
                    semantic_leaf_bytes: 0,
                    ..DecodeLimits::default()
                },
            ] {
                assert!(
                    matches!(
                        foundation.default_constructor_access_subject(
                            target,
                            &mut BudgetMeter::new(limits)
                        ),
                        Err(Error::Resource(_))
                    ),
                    "{name} {limits:?}"
                );
            }
        }
    });
}
