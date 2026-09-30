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
    let declarations = module
        .local_functions
        .values()
        .filter_map(|local| local.source().map(|(function, _)| function))
        .collect::<std::collections::HashSet<_>>();
    let arguments = module
        .instantiations
        .values()
        .filter(|application| {
            declarations.contains(&module.generic_functions[application.generic].function)
                && application.type_args.iter().all(|&argument| {
                    matches!(
                        module.types[argument],
                        hir::Type::String | hir::Type::Integer(_)
                    )
                })
        })
        .map(|application| application.type_args.clone())
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(
        arguments.len(),
        2,
        "unused owner arguments remain on actual local calls"
    );
    assert!(arguments.iter().all(|arguments| arguments.len() == 2));
    assert_eq!(
        arguments
            .iter()
            .map(|arguments| arguments[0])
            .collect::<std::collections::HashSet<_>>()
            .len(),
        2
    );
    assert_eq!(
        arguments
            .iter()
            .map(|arguments| arguments[1])
            .collect::<std::collections::HashSet<_>>()
            .len(),
        1
    );

    let bodies = output
        .local
        .functions
        .iter()
        .filter(|(_, function)| !function.capture_parameters.is_empty())
        .map(|(id, _)| id)
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(bodies.len(), 2);
    scoop_mir_lower::lower(&output.local).unwrap();
}
