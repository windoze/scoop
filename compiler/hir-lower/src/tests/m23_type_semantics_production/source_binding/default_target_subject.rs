use super::super::source_dispatch::with_hir_source;
use super::*;
use hir::{
    CanonicalDefaultSourceAccessDeclarationsV1 as Table, DefaultSourceAccessDomainV1 as Domain,
    DefaultSourceIndirectTargetV1 as Target, DefaultSourceTargetSubjectError as Error,
};
use scoop_identity::DefinitionOriginSubject as Subject;
mod constructors;
mod expected;
mod rejection;
mod relations;
const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-defaults/indirect-targets.scoop"
));
const COMBINATIONS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-defaults/indirect-target-combinations.scoop"
));
fn kind(target: Target) -> &'static str {
    match target {
        Target::StructField(_) => "StructField",
        Target::ClassField(_) => "ClassField",
        Target::EnumVariant(_) => "EnumVariant",
        Target::Singleton(_) => "Singleton",
    }
}
fn summary(domain: &Domain) -> String {
    use hir::PersistentAccessConstraintV1 as Constraint;
    let names = domain
        .persistent()
        .constraints()
        .iter()
        .map(|c| match c {
            Constraint::Cone(_) => "Cone",
            Constraint::File(_) => "File",
            Constraint::LexicalOwner(_) => "LexicalOwner",
            Constraint::SubclassesOf(_) => "SubclassesOf",
        })
        .collect::<Vec<_>>();
    format!(
        "[{}]; generic {}",
        names.join(", "),
        domain.generic_subclasses().values().len()
    )
}
#[test]
fn indirect_default_targets_derive_actual_declaration_demands_and_lookup_domains() {
    for source in [SOURCE, COMBINATIONS] {
        with_hir_source(source, |output, _| {
            let export = output.output().export.module();
            let fixture = Fixture::from_output(output);
            assert!(fixture.source.entries().sources.records().is_empty());
            let foundation = fixture.bind().unwrap();
            let targets = expected::targets(export);
            let mut required = BTreeSet::new();
            for (target, expected, _) in &targets {
                let subject = foundation
                    .default_indirect_access_subject(*target, &mut meter())
                    .unwrap();
                assert_eq!(&subject, expected, "{target:?}");
                required.insert(subject);
            }
            assert_eq!(
                targets
                    .iter()
                    .map(|(t, _, _)| kind(*t))
                    .collect::<BTreeSet<_>>()
                    .len(),
                4
            );
            let table =
                Table::from_export_hir(&output.output().export, &required, &mut meter()).unwrap();
            let bound = foundation
                .bind_default_access_declarations(&table, &required, &mut meter())
                .unwrap();
            let mut snapshot = String::new();
            for (target, subject, domain) in targets {
                let actual = bound.source_lookup_domain(subject, &mut meter()).unwrap();
                let expected = Domain::from_export_hir(export, domain, &mut meter()).unwrap();
                assert_eq!(actual, expected, "{target:?}");
                let role = match subject {
                    Subject::Type(_) => "Type",
                    Subject::GenericType(_) => "GenericType",
                    Subject::Property(_) => "Property",
                    other => panic!("unexpected indirect access subject {other:?}"),
                };
                snapshot.push_str(&format!(
                    "{} -> {role}: {}\n",
                    kind(target),
                    summary(&actual)
                ));
            }
            if source == SOURCE {
                assert_eq!(
                    snapshot,
                    include_str!(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/../../tests/fixtures/m23-type-source-defaults/indirect-targets.snap"
                    ))
                );
            } else {
                assert!(snapshot.contains("GenericType"));
                assert!(snapshot.contains("generic 1"));
                assert!(export.class_fields.iter().any(|(id, _)| matches!(
                    export.field_identities[id].key().view(),
                    scoop_identity::FieldIdentityView::SourcePropertyDelegate { .. }
                )));
            }
        });
    }
}

#[test]
fn indirect_default_target_queries_charge_one_shared_budget() {
    with_hir_source(COMBINATIONS, |output, _| {
        let fixture = Fixture::from_output(output);
        let foundation = fixture.bind().unwrap();
        for (target, _, _) in expected::targets(output.output().export.module()) {
            let mut measured = meter();
            foundation
                .default_indirect_access_subject(target, &mut measured)
                .unwrap();
            let work = measured.usage().validation_work_units;
            let mut shared = BudgetMeter::new(DecodeLimits {
                validation_work_units: work * 2 - 1,
                ..DecodeLimits::default()
            });
            foundation
                .default_indirect_access_subject(target, &mut shared)
                .unwrap();
            assert!(matches!(
                foundation.default_indirect_access_subject(target, &mut shared),
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
                        foundation
                            .default_indirect_access_subject(target, &mut BudgetMeter::new(limits)),
                        Err(Error::Resource(_))
                    ),
                    "{target:?} {limits:?}"
                );
            }
        }
    });
}
