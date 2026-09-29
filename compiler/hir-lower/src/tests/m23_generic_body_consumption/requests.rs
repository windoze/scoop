use super::*;

fn fixture(name: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/m23-shared-requests")
            .join(format!("{name}.scoop")),
    )
    .unwrap()
}

#[test]
fn repeated_import_bindings_share_one_function_for_each_complete_application() {
    with_provider_consumer(
        &fixture("provider"),
        &fixture("standalone"),
        |output, _, _, _, _| {
            let local = output.output().local.module();
            let identities = local
                .functions
                .iter()
                .filter(|(_, function)| function.name == "identity")
                .map(|(_, function)| function.materialization)
                .collect::<Vec<_>>();
            assert_eq!(
                identities.len(),
                3,
                "Int, String and Unit each require one instance"
            );
            let unique = identities
                .into_iter()
                .collect::<std::collections::HashSet<_>>();
            assert_eq!(unique.len(), 3, "aliases share the original declaration");
        },
    )
    .unwrap();
}

#[test]
fn method_arguments_and_lexical_binders_remain_distinct_in_common_requests() {
    with_provider_consumer(
        &fixture("provider"),
        &fixture("methods-closures"),
        |output, _, _, _, _| {
            let local = output.output().local.module();
            for owner in ["Box", "Local"] {
                let methods = local
                    .functions
                    .iter()
                    .filter(|(_, function)| {
                        function.name.rsplit('.').next() == Some("mix")
                            && matches!(&function.receiver,
                                hir::concrete::FunctionReceiver::Method(method)
                                if matches!(local.types[method.owner].kind,
                                    hir::concrete::TypeKind::Class(id)
                                    if local.classes[id].name == owner))
                    })
                    .map(|(_, function)| function.materialization)
                    .collect::<Vec<_>>();
                assert_eq!(
                    methods.len(),
                    2,
                    "{owner}.mix keeps Int and Unit method arguments"
                );
                assert_ne!(methods[0], methods[1]);
            }
            let captures = local
                .functions
                .iter()
                .filter(|(_, function)| function.name == "keep")
                .collect::<Vec<_>>();
            assert_eq!(
                captures.len(),
                1,
                "the generic lexical body is instantiated once"
            );
        },
    )
    .unwrap();
}
