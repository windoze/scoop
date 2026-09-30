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

#[test]
fn class_definitions_keep_recursive_fields_in_their_original_binder_domain() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/m23-shared-class-definitions");
    let provider = std::fs::read_to_string(root.join("provider.scoop")).unwrap();
    for case in ["standalone", "combined"] {
        let source = std::fs::read_to_string(root.join(format!("{case}.scoop"))).unwrap();
        with_provider_consumer(&provider, &source, |output, world, _, _, _| {
            let export = output.output().export.module();
            let links = export
                .loaded_class_definitions
                .values()
                .filter(|value| value.declaration.name() == "Link")
                .collect::<Vec<_>>();
            assert_eq!(links.len(), 1);
            let link = &links[0].definition;
            assert_eq!(
                export.types[link.fields[0].ty],
                hir::Type::Param(link.type_params[0].id)
            );
            let hir::Type::Enum(next) = export.types[link.fields[1].ty] else {
                panic!("next is an Option");
            };
            assert_eq!(
                export.enum_applications[next].arguments,
                vec![export.class_applications[link.self_application].canonical_type]
            );
            let remote = &export
                .loaded_class_definitions
                .values()
                .find(|value| value.declaration.name() == "Remote")
                .unwrap()
                .definition;
            assert_eq!(
                export.types[remote.fields[0].ty],
                hir::Type::Param(remote.type_params[0].id)
            );
            for (_, class) in export.classes.iter() {
                assert_eq!(class.fields.len(), class.definition.fields.len());
                for (index, &field) in class.fields.iter().enumerate() {
                    assert_eq!(export.class_fields[field].definition_index, index);
                    assert_eq!(
                        export.class_field_definition(field),
                        &class.definition.fields[index]
                    );
                }
            }
            let local = output.output().local.module();
            let instances = local
                .classes
                .iter()
                .filter(|(_, value)| value.name == "Link")
                .collect::<Vec<_>>();
            assert_eq!(instances.len(), 3);
            for (_, instance) in instances {
                assert_eq!(instance.declared_fields()[0].ty, instance.type_arguments[0]);
            }
            let foundation = hir::CanonicalHirFoundation::from_dependency_output(&output).unwrap();
            if case == "combined" {
                assert_inherited_default_mappings(&output, world, &foundation);
            }
        })
        .unwrap_or_else(|errors| panic!("{case}: {errors:?}"));
    }
}

fn assert_inherited_default_mappings(
    output: &hir::DependencyHirOutput,
    world: &hir::ImportedSemanticWorld,
    foundation: &hir::CanonicalHirFoundation,
) {
    use scoop_identity::{CallableTemplateOrigin, SignatureTypeKey};

    let export = output.output().export.module();
    let mut authority = hir::CrossConeHirProductionAuthority::new(
        foundation,
        &export.public_export_bindings,
        world,
    );
    let interface =
        hir::CrossConeHirInterfaceSectionV1::from_dependency_hir(output, &[], &mut authority)
            .unwrap();
    let templates = interface.default_templates();
    let mut roots = std::collections::HashSet::new();
    for (name, indices) in [("Local.select", [1, 0]), ("Leaf.select", [0, 1])] {
        let (function, _) = export
            .functions
            .iter()
            .find(|(_, function)| function.name == name)
            .unwrap();
        let owner = match &export.function_identities[function] {
            hir::HirFunctionIdentity::Source(hir::HirSourceFunctionIdentity::Plain(identity)) => {
                CallableTemplateOrigin::Function(identity.id())
            }
            hir::HirFunctionIdentity::Source(hir::HirSourceFunctionIdentity::Generic(identity)) => {
                CallableTemplateOrigin::GenericFunction(identity.id())
            }
            identity => panic!("the override retains its source declaration: {identity:?}"),
        };
        let template = templates
            .get(hir::ExportDefaultTemplateKeyV1::new(owner, 0))
            .unwrap();
        assert_ne!(template.definition_root().declaration(), owner);
        roots.insert(template.definition_root());
        assert_eq!(
            template.type_parameters().arguments(),
            indices.map(|index| SignatureTypeKey::Binder { depth: 0, index })
        );
    }
    assert_eq!(roots.len(), 1, "both overrides keep the original default");
}
