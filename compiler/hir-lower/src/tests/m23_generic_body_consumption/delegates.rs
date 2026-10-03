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
fn imported_initializer_references_keep_provider_invokes_and_capture_values() {
    use hir::concrete::{CallableReferenceTarget, CallableTarget};
    use scoop_identity::{
        CallableInstantiationOwner, CallableMaterializationContext, GeneratedCallableKey,
    };

    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/m23-generic-delegate-references");
    let source = |name: &str| std::fs::read_to_string(root.join(format!("{name}.scoop"))).unwrap();
    for case in [
        "reference-named",
        "reference-generic",
        "reference-local",
        "reference-local-generic",
        "reference-member",
        "reference-computed",
        "reference-value-member",
        "reference-generic-member",
        "reference-virtual",
        "reference-interface",
        "reference-extension",
        "reference-unbound",
        "reference-values",
        "reference-nested",
        "reference-local-factory",
    ] {
        eprintln!("initializer reference case: {case}");
        let result =
            with_provider_consumer(&source("provider"), &source(case), |output, _, _, _, _| {
                let module = output.output().local.module();
                assert_eq!(module.initialization_units.len(), 1);
                let unit = module
                    .initialization_units
                    .iter()
                    .next()
                    .unwrap()
                    .1
                    .identity
                    .id();
                assert_eq!(module.callable_references.len(), 2);
                let executable = output
                    .executable_dependency_callables()
                    .unwrap()
                    .into_iter()
                    .map(|use_| use_.callee())
                    .collect::<std::collections::BTreeSet<_>>();
                for (id, reference) in module.callable_references.iter() {
                    if let Some(CallableTarget::Imported(callee)) = reference.target.callee() {
                        assert!(
                            executable.contains(&callee),
                            "an invoked dependency is a machine root"
                        );
                    }
                    assert!(matches!(
                        reference.identity.callable_record().key(),
                        GeneratedCallableKey::CallableReferenceInvoke { .. }
                    ));
                    match reference.identity.materialization().context() {
                        CallableMaterializationContext::InitializationApplication(actual) => {
                            assert_eq!(actual, unit)
                        }
                        CallableMaterializationContext::Application(application) => {
                            assert_eq!(
                                module
                                    .callable_applications
                                    .get(application)
                                    .unwrap()
                                    .key()
                                    .instantiation_owner(),
                                CallableInstantiationOwner::EnclosingInitializationApplication(
                                    unit
                                )
                            );
                        }
                        CallableMaterializationContext::NoSubstitution => panic!(
                            "a generic initializer reference retains its enclosing application"
                        ),
                    }
                    let receiver = module
                        .local_value_identities
                        .callable_reference_receiver(id);
                    assert_eq!(
                        receiver.is_some(),
                        matches!(
                            reference.target,
                            CallableReferenceTarget::BoundMember { .. }
                                | CallableReferenceTarget::BoundExtension { .. }
                        )
                    );
                }
                if case == "reference-local-factory" {
                    let (id, reference) = module
                        .callable_references
                        .iter()
                        .find(|(_, reference)| !reference.captures.is_empty())
                        .unwrap();
                    assert_eq!(reference.captures.len(), 1);
                    let captured = module
                        .local_value_identities
                        .callable_reference_capture(id, 0)
                        .id();
                    let functions = module
                        .functions
                        .iter()
                        .filter(|(_, function)| matches!(function.name.as_str(), "make" | "read"))
                        .collect::<Vec<_>>();
                    assert_eq!(functions.len(), 2);
                    for (id, function) in functions {
                        assert_eq!(function.capture_parameters.len(), 1);
                        assert_eq!(
                            module
                                .local_value_identities
                                .function_local(id, function.capture_parameters[0].local)
                                .id(),
                            captured
                        );
                    }
                }
                hir::CanonicalHirFoundation::from_dependency_output(&output).unwrap();
            });
        result.unwrap_or_else(|error| panic!("{case}: {error:?}"));
    }
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

#[test]
fn imported_initializer_closures_keep_the_initialization_materialization() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/m23-generic-delegate-closures");
    let source = |name: &str| std::fs::read_to_string(root.join(format!("{name}.scoop"))).unwrap();
    for (case, expected, anonymous_count) in [
        ("initializer-closure", 2, 0),
        ("initializer-combined", 3, 0),
        ("initializer-plain", 1, 0),
        ("initializer-anonymous", 1, 1),
        ("initializer-values", 2, 0),
        ("initializer-local-closure", 2, 0),
    ] {
        with_provider_consumer(&source("provider"), &source(case), |output, _, foundation, _, _| {
            let materializations = hir::CanonicalHirFoundation::from_dependency_output(&output).unwrap();
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
            assert_eq!(local.lambdas.len(), expected);
            assert_eq!(local.anonymous_functions.len(), anonymous_count);
            if case == "initializer-local-closure" {
                let captures = local.lambdas.iter()
                    .filter(|(_, closure)| !closure.captures.is_empty())
                    .collect::<Vec<_>>();
                assert_eq!(captures.len(), 1);
                let (closure, declaration) = captures[0];
                assert_eq!(declaration.captures.len(), 1);
                let captured = local.local_value_identities.lambda_capture(closure, 0);
                let functions = local.functions.iter()
                    .filter(|(_, function)| matches!(function.name.as_str(), "build" | "read"))
                    .collect::<Vec<_>>();
                assert_eq!(functions.len(), 2);
                for (id, function) in functions {
                    assert_eq!(function.capture_parameters.len(), 1);
                    assert_eq!(local.local_value_identities.function_local(
                        id, function.capture_parameters[0].local,
                    ).id(), captured.id());
                }
            }
            let functions = local.lambdas.iter().map(|(_, closure)| closure.function)
                .chain(local.anonymous_functions.iter().map(|(_, closure)| closure.function));
            for function in functions {
                let function = &local.functions[function];
                assert!(matches!(
                    function.materialization.template(),
                    scoop_identity::CallableTemplateOwner::Generated(_)
                ));
                match function.materialization.context() {
                    scoop_identity::CallableMaterializationContext::InitializationApplication(actual) => assert_eq!(actual, unit),
                    scoop_identity::CallableMaterializationContext::Application(id) => {
                        assert_eq!(local.callable_applications.get(id).unwrap().key().instantiation_owner(),
                            scoop_identity::CallableInstantiationOwner::EnclosingInitializationApplication(unit));
                    }
                    scoop_identity::CallableMaterializationContext::NoSubstitution => panic!("initializer closures retain their enclosing application"),
                }
            }
            let export = output.output().export.module();
            local.visit_executable_expressions(|occurrence| {
                let position = occurrence.position;
                if position.root.context()
                    != scoop_identity::CallableMaterializationContext::InitializationApplication(unit)
                {
                    return Ok::<_, ()>(());
                }
                let evaluation = occurrence.expression.origin.evaluation;
                let source = &export.source_files[evaluation.file as usize];
                let context = export.source_context_identities.get(evaluation.context).unwrap();
                let span = scoop_identity::SourceSpan::new(
                    evaluation.span.start.into(), evaluation.span.end.into(),
                ).unwrap();
                let origin = scoop_identity::EvaluationOrigin::new(
                    source.identity.clone(), span, context.key(),
                ).unwrap();
                foundation.validate_executable_evaluation_origin(
                    source.identity.cone(), position.root, &origin, &materializations,
                ).unwrap_or_else(|error| panic!(
                    "{case} {position:?} {:?} {:?}: {error:?}",
                    occurrence.expression.kind, context.key(),
                ));
                Ok(())
            }).unwrap();
        })
        .unwrap_or_else(|error| panic!("{case}: {error:?}"));
    }
}
