use super::*;

fn fixture(name: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/m23-shared-singleton-reads")
            .join(format!("{name}.scoop")),
    )
    .unwrap()
}

#[test]
fn singleton_reads_retain_original_storage_and_share_one_operation() {
    use hir::concrete::{ExprKind, SingletonValueTarget};

    for case in ["standalone", "combined"] {
        with_provider_consumer(
            &fixture("provider"),
            &fixture(case),
            |output, _, _, _, _| {
                let module = output.output().local.module();
                let mut current_reads = 0;
                let mut dependency_reads = std::collections::HashSet::new();
                module
                    .visit_executable_expressions(|occurrence| {
                        match occurrence.expression.kind {
                            ExprKind::SingletonValue(SingletonValueTarget::Local(value)) => {
                                current_reads += 1;
                                assert_eq!(
                                    module.object_types[module.singleton_values[value].object_type]
                                        .canonical_type,
                                    occurrence.expression.ty
                                );
                            }
                            ExprKind::SingletonValue(SingletonValueTarget::Dependency(value)) => {
                                dependency_reads.insert(value);
                            }
                            _ => {}
                        }
                        Ok::<_, ()>(())
                    })
                    .unwrap();
                assert!(current_reads > 0);
                if case == "combined" {
                    assert_eq!(dependency_reads.len(), 1);
                    assert!(
                        module
                            .singleton_values
                            .values()
                            .all(|value| { !dependency_reads.contains(&value.identity) }),
                        "reading a dependency singleton does not copy its storage"
                    );
                } else {
                    assert!(dependency_reads.is_empty());
                }
            },
        )
        .unwrap_or_else(|errors| panic!("{case}: {errors:?}"));
    }
}
