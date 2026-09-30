use super::*;

fn fixture(name: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/m23-shared-structs")
            .join(format!("{name}.scoop")),
    )
    .unwrap()
}

#[test]
fn source_and_dependency_struct_instances_share_fields_gc_and_interfaces() {
    for case in ["standalone", "combined"] {
        with_provider_consumer(
            &fixture("provider"),
            &fixture(case),
            |output, _, _, _, _| {
                let module = output.output().local.module();
                let mut applications = std::collections::HashSet::new();
                for (_, structure) in module.structs.iter() {
                    assert!(applications.insert((
                        structure.origin.declaration_id(),
                        structure.type_arguments.clone(),
                    )));
                }
                for name in ["RemoteCell", "LocalCell"] {
                    let instances = module
                        .structs
                        .iter()
                        .filter(|(_, structure)| structure.name == name)
                        .collect::<Vec<_>>();
                    assert_eq!(
                        instances.len(),
                        if name == "LocalCell" && case == "standalone" {
                            0
                        } else {
                            3
                        }
                    );
                    for (_, instance) in instances {
                        let argument = instance.type_arguments[0];
                        assert_eq!(instance.gc_free, module.types[argument].gc_free);
                        assert_eq!(
                            module.types[instance.canonical_type].gc_free,
                            instance.gc_free
                        );
                        let hir::concrete::StructRepresentation::Declared { fields, .. } =
                            &instance.representation
                        else {
                            panic!("{name} retains its source fields");
                        };
                        assert_eq!(fields[0].name, "value");
                        assert_eq!(fields[0].ty, argument);
                        assert_eq!(instance.direct_interfaces.len(), 1);
                        assert_eq!(instance.interfaces.len(), 2);
                        assert_eq!(instance.interface_implementations.len(), 2);
                    }
                }
                for name in ["NativePair", "LocalNative"] {
                    let instances = module
                        .structs
                        .iter()
                        .filter(|(_, structure)| structure.name == name)
                        .collect::<Vec<_>>();
                    assert_eq!(
                        instances.len(),
                        usize::from(name == "NativePair" || case == "combined")
                    );
                    for (_, instance) in instances {
                        let hir::concrete::StructRepresentation::Declared {
                            attributes,
                            c_abi,
                            fields,
                        } = &instance.representation
                        else {
                            panic!("{name} retains its C layout");
                        };
                        assert!(attributes.c_layout.is_some());
                        assert_eq!(*c_abi, hir::concrete::StructCAbi::SourceRepresentation);
                        assert!(instance.gc_free);
                        assert_eq!(fields.len(), 2);
                    }
                }
            },
        )
        .unwrap_or_else(|errors| panic!("{case}: {errors:?}"));
    }
}
