use super::*;

fn fixture(name: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/m23-shared-classes")
            .join(format!("{name}.scoop")),
    )
    .unwrap()
}

#[test]
fn source_and_dependency_class_instances_share_fields_bases_and_interfaces() {
    for case in ["standalone", "combined"] {
        with_provider_consumer(
            &fixture("provider"),
            &fixture(case),
            |output, _, _, _, _| {
                let module = output.output().local.module();
                let mut applications = std::collections::HashSet::new();
                for (_, class) in module.classes.iter() {
                    assert!(
                        applications
                            .insert((class.origin.declaration_id(), class.type_arguments.clone()))
                    );
                }
                for name in ["Base", "Remote", "Local"] {
                    let instances = module
                        .classes
                        .iter()
                        .filter(|(_, class)| class.name == name)
                        .collect::<Vec<_>>();
                    assert_eq!(
                        instances.len(),
                        if name == "Local" && case == "standalone" {
                            0
                        } else {
                            3
                        },
                        "{case}: {name}"
                    );
                    let mut field_identities = std::collections::HashSet::new();
                    for (_, instance) in instances {
                        let argument = instance.type_arguments[0];
                        assert!(!module.types[instance.canonical_type].gc_free);
                        let hir::concrete::ClassRepresentation::Declared { fields, base_class } =
                            &instance.representation
                        else {
                            panic!("{name} retains its declared field layout");
                        };
                        assert_eq!(fields.len(), 1);
                        field_identities.insert(fields[0].identity);
                        if name == "Local" {
                            let hir::concrete::TypeKind::Class(remote) =
                                module.types[fields[0].ty].kind
                            else {
                                panic!("Local keeps its dependency Remote field");
                            };
                            assert_eq!(module.classes[remote].name, "Remote");
                            assert_eq!(
                                module.classes[remote].type_arguments,
                                instance.type_arguments
                            );
                        } else {
                            assert_eq!(fields[0].ty, argument);
                        }
                        if name == "Base" {
                            assert!(base_class.is_none());
                        } else {
                            let base = &module.classes[base_class.unwrap()];
                            assert_eq!(base.name, "Base");
                            assert_eq!(base.type_arguments, instance.type_arguments);
                        }
                        assert_eq!(instance.interface_implementations.len(), 1);
                    }
                    assert_eq!(
                        field_identities.len(),
                        usize::from(name != "Local" || case == "combined")
                    );
                }
            },
        )
        .unwrap_or_else(|errors| panic!("{case}: {errors:?}"));
    }
}
