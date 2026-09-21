use super::*;

#[test]
fn unused_local_owner_arguments_keep_distinct_body_materializations() {
    let source = [
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/m23-type-source-defaults/local-owner-arguments.scoop"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/m23-type-source-defaults/local-owner-argument-combinations.scoop"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/m23-type-source-defaults/local-owner-argument-calls.scoop"
        )),
    ]
    .join("\n");
    let output = lower_source(&source).unwrap();
    let module = output.export.module();
    let mapped = module
        .local_functions
        .iter()
        .filter(|(_, descriptor)| {
            descriptor.owner_type_arguments.iter().all(|&argument| {
                matches!(
                    module.types[argument],
                    hir::Type::String | hir::Type::Integer(_)
                )
            })
        })
        .map(|(_, descriptor)| descriptor.owner_type_arguments.clone())
        .collect::<Vec<_>>();
    assert_eq!(mapped.len(), 6);
    assert_eq!(
        mapped
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len(),
        2
    );

    let bodies = output
        .local
        .local_functions
        .iter()
        .map(|(_, descriptor)| descriptor.function)
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(bodies.len(), 2);
    scoop_mir_lower::lower(&output.local).unwrap();
}
