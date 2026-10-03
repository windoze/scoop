use super::*;

fn fixture(name: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/m23-shared-bound-properties")
            .join(format!("{name}.scoop")),
    )
    .unwrap()
}

#[test]
fn class_bound_accessors_keep_the_declaring_application_and_original_receiver() {
    with_provider_consumer(
        &fixture("provider"),
        &fixture("combined"),
        |output, _, _, _, _| {
            let module = output.output().export.module();
            let accessors = module
                .bound_callable_refs
                .iter()
                .filter_map(|(_, bound)| match bound.source {
                    hir::BoundCallableSource::Class {
                        bound,
                        callable: hir::CallableTarget::Local(callable),
                    } => {
                        let application = &module.class_applications[bound];
                        (module.classes[module
                            .nominal_identities
                            .class_id(application.template)
                            .expect("an application retains its declaration")]
                        .name
                            == "Local")
                            .then_some((application, module.callable_function(callable)))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(accessors.len(), 2);
            for (application, function) in accessors {
                assert_eq!(application.arguments.len(), 2);
                assert!(matches!(
                    module.types[application.arguments[0]],
                    hir::Type::Unit
                ));
                assert!(matches!(
                    module.types[application.arguments[1]],
                    hir::Type::Param(_)
                ));
                assert!(module.functions[function].name.contains("exposed"));
            }
        },
    )
    .unwrap_or_else(|errors| panic!("{errors:?}"));
}
