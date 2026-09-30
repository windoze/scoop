use super::*;

fn fixture(name: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/m23-shared-interfaces")
            .join(format!("{name}.scoop")),
    )
    .unwrap()
}

#[test]
fn interface_instances_share_families_and_keep_each_inherited_substitution() {
    for case in ["standalone", "combined"] {
        with_provider_consumer(
            &fixture("provider"),
            &fixture(case),
            |output, _, _, _, _| {
                let module = output.output().local.module();
                let mut applications = std::collections::HashSet::new();
                let mut families = std::collections::HashMap::new();
                for (_, interface) in module.interfaces.iter() {
                    let origin = interface.origin.declaration_id();
                    assert!(applications.insert((origin, interface.type_arguments.clone())));
                    if let Some(previous) = families.insert(origin, interface.family) {
                        assert_eq!(previous, interface.family);
                    }
                }
                for name in ["Values", "Reversed", "Restored", "Diamond"] {
                    let instances = module
                        .interfaces
                        .iter()
                        .filter(|(_, interface)| interface.name == name)
                        .collect::<Vec<_>>();
                    assert_eq!(
                        instances.len(),
                        if name == "Values" || case == "combined" {
                            3
                        } else {
                            0
                        },
                        "{case}: {name}"
                    );
                    for (_, interface) in instances {
                        assert!(!module.types[interface.canonical_type].gc_free);
                        let arguments = &interface.type_arguments;
                        let (first, second) = if name == "Reversed" {
                            (arguments[1], arguments[0])
                        } else {
                            (arguments[0], arguments[1])
                        };
                        assert_eq!(interface.methods.len(), 4, "{name}");
                        let getter = interface
                            .methods
                            .iter()
                            .find(|method| method.name == "first" && method.params.is_empty())
                            .expect("the first getter remains in the interface");
                        assert_eq!(getter.return_ty, first, "{case}: {name}.first");
                        let getter = interface
                            .methods
                            .iter()
                            .find(|method| method.name == "second" && method.params.is_empty())
                            .expect("the second getter remains in the interface");
                        assert_eq!(getter.return_ty, second, "{case}: {name}.second");
                        let setter = interface
                            .methods
                            .iter()
                            .find(|method| method.name == "second" && method.params.len() == 1)
                            .expect("the second setter remains in the interface");
                        assert_eq!(setter.params.len(), 1);
                        assert_eq!(setter.params[0].ty, second, "{case}: {name}.second setter");
                    }
                }
            },
        )
        .unwrap_or_else(|errors| panic!("{case}: {errors:?}"));
    }
}
