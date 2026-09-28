use super::*;

fn fixture(name: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/m23-generic-delegate-consumption")
            .join(format!("{name}.scoop")),
    )
    .unwrap()
}

#[test]
fn imported_generic_delegates_share_storage_by_complete_receiver_arguments() {
    with_provider_consumer(
        &fixture("provider"),
        &fixture("consumer"),
        |output, _, _, interface, _| {
            let templates = interface.generic_delegates().records();
            assert_eq!(templates.len(), 1);
            assert_eq!(
                templates[0]
                    .initializer()
                    .type_parameters()
                    .arguments()
                    .len(),
                2
            );
            let export = output.output().export.module();
            assert_eq!(export.imported_generic_delegate_templates.len(), 1);
            assert!(export.generic_delegate_templates.is_empty());
            let local = output.output().local.module();
            assert_eq!(local.generic_delegate_specializations.len(), 3);
            assert_eq!(local.initialization_units.len(), 3);
            let mut identities = std::collections::BTreeSet::new();
            let mut storage = std::collections::HashSet::new();
            for (id, unit) in local.initialization_units.iter() {
                let scoop_identity::InitializationUnitKey::GenericDelegatedExtensionApplication {
                    property,
                    receiver_arguments,
                } = unit.identity.key()
                else {
                    panic!("a receiver specialization retains its delegated property identity")
                };
                assert_eq!(*property, templates[0].property());
                assert_eq!(receiver_arguments.as_slice().len(), 2);
                assert!(identities.insert(unit.identity.id()));
                assert_eq!(
                    unit.schedule,
                    hir::concrete::InitializationSchedule::LazyAccess
                );
                let hir::concrete::InitializationUnitKind::GenericDelegatedExtension {
                    specialization,
                } = unit.kind
                else {
                    panic!("a generic delegate retains its storage specialization")
                };
                let specialization = &local.generic_delegate_specializations[specialization];
                assert_eq!(specialization.initialization, id);
                assert!(storage.insert(specialization.storage));
                assert_eq!(
                    local.initialization_failure_roots[unit.failure_root].unit,
                    id
                );
                let initializer = &local.functions[unit.initializer];
                let ensure = &local.functions[unit.ensure];
                assert_ne!(initializer.materialization, ensure.materialization);
                for function in [initializer, ensure] {
                    assert_eq!(
                        function.materialization.context(),
                        scoop_identity::CallableMaterializationContext::InitializationApplication(
                            unit.identity.id()
                        )
                    );
                }
                let hir::concrete::FunctionKind::User(body) = &initializer.kind else {
                    panic!("the initializer contains the checked by expression and storage write")
                };
                assert!(!body.statements.is_empty());
            }
            hir::CanonicalHirFoundation::from_dependency_output(&output).unwrap();
        },
    )
    .unwrap_or_else(|errors| panic!("generic delegate consumption: {errors:?}"));
}

#[test]
fn imported_initializer_locals_keep_the_initialization_application_owner() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/m23-generic-delegate-generated");
    let source = |name: &str| std::fs::read_to_string(root.join(format!("{name}.scoop"))).unwrap();
    for (case, expected) in [("initializer-basic", 1), ("initializer-local", 2)] {
        with_provider_consumer(&source("provider"), &source(case), |output, _, _, _, _| {
            let local = output.output().local.module();
            assert_eq!(local.initialization_units.len(), 1);
            let unit = local
                .initialization_units
                .iter()
                .next()
                .unwrap()
                .1
                .identity
                .id();
            let functions = local
                .functions
                .iter()
                .filter(|(_, function)| matches!(function.name.as_str(), "read" | "keep"))
                .map(|(_, function)| function)
                .collect::<Vec<_>>();
            assert_eq!(functions.len(), expected);
            for function in functions {
                let scoop_identity::CallableMaterializationContext::Application(id) =
                    function.materialization.context()
                else {
                    panic!("an initializer local retains its callable application");
                };
                let key = local.callable_applications.get(id).unwrap().key();
                assert_eq!(
                    key.instantiation_owner(),
                    scoop_identity::CallableInstantiationOwner::EnclosingInitializationApplication(
                        unit
                    )
                );
                assert_eq!(
                    matches!(
                        key.origin(),
                        scoop_identity::CallableTemplateOrigin::GenericFunction(_)
                    ),
                    function.name == "keep"
                );
                assert_eq!(
                    matches!(
                        key.callable_arguments(),
                        scoop_identity::CallableArguments::NoCallableArguments
                    ),
                    function.name == "read"
                );
            }
        })
        .unwrap_or_else(|error| panic!("{case}: {error:?}"));
    }
}
