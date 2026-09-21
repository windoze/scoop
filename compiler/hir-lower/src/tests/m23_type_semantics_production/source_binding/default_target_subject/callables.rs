use super::*;
use hir::{
    DefaultSourceCallableAccessSubjectV1 as Access, ExportDefaultCallableTargetV1 as Callable,
};
mod rejection;
mod resources;
const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-defaults/callable-targets.scoop"
));
const COMBINATIONS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-defaults/callable-target-combinations.scoop"
));
const CASES: &[(&str, u32)] = &[
    ("direct", 0),
    ("generic", 1),
    ("global", 0),
    ("getter", 1),
    ("setter", 1),
    ("extension", 1),
    ("extensionSetter", 1),
    ("lambda", 0),
    ("anonymous", 0),
    ("reference", 0),
    ("local", 0),
    ("equality", 2),
];
const COMBINED_CASES: &[(&str, u32)] = &[
    ("classBound", 1),
    ("interfaceBound", 1),
    ("localGeneric", 1),
    ("Envelope.Scope.direct", 0),
    ("Envelope.Scope.reference", 0),
];
fn template(
    output: &hir::DependencyHirOutput,
    name: &str,
    position: u32,
) -> hir::DefaultSourceTemplateV1 {
    let id = output
        .output()
        .export
        .module()
        .functions
        .iter()
        .find(|(_, f)| f.name == name)
        .unwrap()
        .0;
    hir::DefaultSourceBodyProductionV1::from_dependency_hir(
        output,
        hir::ExportParameterOwner::Function(id),
        position,
        &mut meter(),
    )
    .unwrap()
    .into_source_template(&mut meter())
    .unwrap()
}
fn label(subject: Subject) -> &'static str {
    match subject {
        Subject::Function(_) => "Function",
        Subject::GenericFunction(_) => "GenericFunction",
        Subject::PropertyAccessor(_) => "PropertyAccessor",
        Subject::Property(_) => "Property",
        _ => panic!("callable/global declaration"),
    }
}
#[test]
fn callable_and_global_targets_replay_sealed_domains_and_preserve_nested_demands() {
    for (source, cases) in [(SOURCE, CASES), (COMBINATIONS, COMBINED_CASES)] {
        with_hir_source(source, |output, _| {
            let fixture = Fixture::from_output(output);
            let foundation = fixture.bind().unwrap();
            let templates = cases
                .iter()
                .map(|(name, position)| (*name, template(output, name, *position)))
                .collect::<Vec<_>>();
            let mut required = BTreeSet::new();
            for (_, template) in &templates {
                for record in template.references().callables() {
                    if let Access::Declaration(subject) = foundation
                        .default_callable_access_subject(record.target(), &mut meter())
                        .unwrap()
                    {
                        required.insert(subject);
                    }
                }
                for record in template.references().globals() {
                    required.insert(
                        foundation
                            .default_global_access_subject(*record.target(), &mut meter())
                            .unwrap(),
                    );
                }
            }
            let table =
                Table::from_export_hir(&output.output().export, &required, &mut meter()).unwrap();
            let bound = foundation
                .bind_default_access_declarations(&table, &required, &mut meter())
                .unwrap();
            let mut snapshot = String::new();
            for (name, template) in &templates {
                let nested = template
                    .index_nested_callables(&mut meter(), &scoop_wire::WirePath::root())
                    .unwrap();
                assert!(
                    !template.references().callables().is_empty()
                        || !template.references().globals().is_empty(),
                    "{name}"
                );
                for record in template.references().callables() {
                    match foundation
                        .default_callable_access_subject(record.target(), &mut meter())
                        .unwrap()
                    {
                        Access::Declaration(subject) => {
                            let domain = bound.source_lookup_domain(subject, &mut meter()).unwrap();
                            assert_eq!(&domain, record.witness().target_domain(), "{name}");
                            snapshot.push_str(&format!(
                                "{name}: {} {}\n",
                                label(subject),
                                summary(&domain)
                            ));
                        }
                        Access::Nested(identity) => {
                            assert!(
                                nested
                                    .occurrences()
                                    .iter()
                                    .any(|item| item.descriptor().identity() == identity),
                                "{name} {identity:?}"
                            );
                            snapshot.push_str(&format!("{name}: {:?}\n", identity.kind()));
                        }
                        Access::DerivedEquality { owner_type } => {
                            assert!(
                                matches!(record.target(), Callable::DerivedEquality { owner_type: expected } if expected == owner_type)
                            );
                            snapshot.push_str(&format!("{name}: DerivedEquality\n"));
                        }
                    }
                }
                for record in template.references().globals() {
                    let subject = foundation
                        .default_global_access_subject(*record.target(), &mut meter())
                        .unwrap();
                    let domain = bound.source_lookup_domain(subject, &mut meter()).unwrap();
                    assert_eq!(&domain, record.witness().target_domain(), "{name}");
                    snapshot.push_str(&format!("{name}: Global {}\n", summary(&domain)));
                }
            }
            assert_eq!(
                snapshot,
                if source == SOURCE {
                    include_str!(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/../../tests/fixtures/m23-type-source-defaults/callable-targets.snap"
                    ))
                } else {
                    include_str!(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/../../tests/fixtures/m23-type-source-defaults/callable-target-combinations.snap"
                    ))
                }
            );
        });
    }
}
