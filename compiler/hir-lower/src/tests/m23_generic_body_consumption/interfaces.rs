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
fn interface_definitions_keep_recursive_slots_in_the_original_binder_domain() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/m23-shared-interface-definitions");
    let provider = std::fs::read_to_string(root.join("provider.scoop")).unwrap();
    for case in ["standalone", "combined"] {
        eprintln!("interface definition case: {case}");
        let source = std::fs::read_to_string(root.join(format!("{case}.scoop"))).unwrap();
        with_provider_consumer(&provider, &source, |output, world, _, _, _| {
            let export = output.output().export.module();
            let views = export
                .loaded_interface_definitions
                .values()
                .filter(|value| value.declaration.name() == "View")
                .collect::<Vec<_>>();
            assert_eq!(views.len(), 1);
            let view = views[0];
            let peer = view
                .methods
                .iter()
                .find(|method| method.name == "peer")
                .unwrap();
            let self_type =
                export.interface_applications[view.definition.self_application].canonical_type;
            assert_eq!(peer.parameters[0].1, self_type);
            assert_eq!(peer.return_type, self_type);
            let swap = export
                .loaded_interface_definitions
                .values()
                .find(|value| value.declaration.name() == "Swap")
                .unwrap();
            let peer = swap
                .methods
                .iter()
                .find(|method| method.name == "peer")
                .unwrap();
            let hir::Type::Interface(parent) = export.types[peer.return_type] else {
                panic!("the inherited peer result remains an interface application");
            };
            let parent = &export.interface_applications[parent];
            assert_eq!(parent.template, view.declaration.owner());
            assert_eq!(
                export.types[parent.arguments[0]],
                hir::Type::Param(swap.definition.type_params[1].id)
            );
            assert_eq!(
                export.types[parent.arguments[1]],
                hir::Type::Param(swap.definition.type_params[0].id)
            );
            let foundation = hir::CanonicalHirFoundation::from_dependency_output(&output).unwrap();
            let mut authority = hir::CrossConeHirProductionAuthority::new(
                &foundation,
                &export.public_export_bindings,
                world,
            );
            hir::CrossConeHirInterfaceSectionV1::from_dependency_hir(&output, &[], &mut authority)
                .unwrap();
        })
        .unwrap_or_else(|errors| panic!("{case}: {errors:?}"));
    }
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
