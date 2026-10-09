use super::*;

#[test]
fn source_and_dependency_enum_instances_share_payload_and_gc_substitution() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/m23-shared-enums");
    with_provider_consumer(
        &std::fs::read_to_string(root.join("provider.scoop")).unwrap(),
        &std::fs::read_to_string(root.join("instances.scoop")).unwrap(),
        |output, _, _, _, _| {
            let export = output.output().export.module();
            let choices = export
                .loaded_enum_definitions
                .values()
                .filter(|definition| definition.declaration.name() == "Choice")
                .collect::<Vec<_>>();
            assert_eq!(choices.len(), 1);
            let choice = choices[0];
            let parameter = choice.definition.type_params[0].id;
            let payload = choice
                .definition
                .variants
                .iter()
                .find(|variant| variant.name == "Item")
                .unwrap();
            assert_eq!(
                export.types[payload.fields[0].ty],
                hir::Type::Param(parameter)
            );
            let module = output.output().local.module();
            let mut applications = std::collections::HashSet::new();
            for (_, enumeration) in module.enums.iter() {
                assert!(applications.insert((
                    enumeration.origin.declaration_id(),
                    enumeration.type_arguments.clone(),
                )));
            }
            for name in ["LocalChoice", "Choice"] {
                let instances = module
                    .enums
                    .iter()
                    .filter(|(_, enumeration)| enumeration.name == name)
                    .collect::<Vec<_>>();
                assert_eq!(instances.len(), 3, "{name}");
                for (_, instance) in instances {
                    assert_eq!(instance.type_arguments.len(), 1);
                    let argument = instance.type_arguments[0];
                    assert_eq!(instance.gc_free, module.types[argument].gc_free);
                    assert_eq!(
                        module.types[instance.canonical_type].gc_free,
                        instance.gc_free,
                    );
                    let payload = instance
                        .variants
                        .iter()
                        .find(|variant| {
                            variant.name == if name == "Choice" { "Item" } else { "Pair" }
                        })
                        .unwrap();
                    assert_eq!(payload.fields[0].ty, argument);
                }
            }
        },
    )
    .unwrap();
}

#[test]
fn enum_definitions_retain_recursive_payload_and_interface_environments() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/m23-shared-enum-definitions");
    for case in ["standalone", "combined"] {
        with_provider_consumer(
            &std::fs::read_to_string(root.join("provider.scoop")).unwrap(),
            &std::fs::read_to_string(root.join(format!("{case}.scoop"))).unwrap(),
            |output, _, _, _, _| {
                hir::CanonicalHirFoundation::from_dependency_output(&output).unwrap();
                let module = output.output().local.module();
                for (_, enumeration) in module
                    .enums
                    .iter()
                    .filter(|(_, value)| value.name == "Entry" || value.name == "LocalEntry")
                {
                    let argument = enumeration.type_arguments[0];
                    assert_eq!(enumeration.variants[0].fields[0].ty, argument);
                    let implementation = &enumeration.interface_implementations[0];
                    assert_eq!(
                        module.interfaces[implementation.interface].type_arguments,
                        vec![argument]
                    );
                }
            },
        )
        .unwrap_or_else(|error| panic!("{case}: {error:?}"));
    }
}

fn fixture(name: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/m23-generic-nominal-consumption")
            .join(format!("{name}.scoop")),
    )
    .unwrap()
}

#[test]
fn imported_generic_nominals_substitute_payloads_and_preserve_origin() {
    for case in ["standalone", "combined"] {
        with_provider_consumer(
            &fixture("provider"),
            &fixture(case),
            |output, _, _, _, _| {
                let export = output.output().export.module();
                assert!(
                    export.enums.is_empty(),
                    "dependency templates are not local declarations"
                );
                let local = output.output().local.module();
                let parcels = local
                    .enums
                    .iter()
                    .filter(|(_, value)| value.name == "Parcel")
                    .collect::<Vec<_>>();
                assert_eq!(parcels.len(), if case == "standalone" { 1 } else { 4 });
                for (_, value) in parcels {
                    assert_eq!(value.type_arguments.len(), 1);
                    assert_eq!(
                        value.origin.source().unwrap().declaration().origin(),
                        ConeCoordinate::new("test", "generic-provider", "1.0.0")
                            .unwrap()
                            .identity()
                            .unwrap()
                    );
                    assert!(
                        local
                            .exact_type_identities
                            .nominal_specialization(value.canonical_type)
                            .is_some()
                    );
                    let item = value
                        .variants
                        .iter()
                        .find(|variant| variant.name == "Item")
                        .unwrap();
                    assert_eq!(item.fields[0].ty, value.type_arguments[0]);
                }
                hir::CanonicalHirFoundation::from_dependency_output(&output).unwrap();
                let dependencies =
                    scoop_mir::SelectedExternalMirSet::try_from_callables(local.cone, Vec::new())
                        .unwrap();
                scoop_mir_lower::lower_current_cone(&output, dependencies, Default::default())
                    .unwrap();
            },
        )
        .unwrap_or_else(|error| panic!("{case}: {error:?}"));
    }
}
