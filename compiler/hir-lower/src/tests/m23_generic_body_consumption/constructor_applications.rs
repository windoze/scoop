use super::*;

fn fixture(name: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/m23-shared-constructor-applications")
            .join(format!("{name}.scoop")),
    )
    .unwrap()
}

#[test]
fn constructor_applications_keep_complete_owners_and_original_definitions() {
    for case in ["standalone", "combined"] {
        with_provider_consumer(
            &fixture("provider"),
            &fixture(case),
            |output, _, _, _, _| {
                let module = output.output().export.module();
                let mut current_classes = 0;
                let mut current_structs = 0;
                let mut imported_classes = 0;
                let mut imported_structs = 0;
                let mut classes = std::collections::HashSet::new();
                let mut structs = std::collections::HashSet::new();
                for application in module.class_constructor_applications.values() {
                    assert!(classes.insert((application.constructor, application.owner)));
                    let owner = &module.class_applications[application.owner];
                    match application.constructor {
                        hir::ClassConstructorDefinition::Local(_) => current_classes += 1,
                        hir::ClassConstructorDefinition::Template(template) => {
                            imported_classes += 1;
                            let template = &module.imported_constructor_templates[template];
                            let hir::Type::Class(original) = module.types[template.owner] else {
                                panic!("class constructors retain their class owner");
                            };
                            assert_eq!(
                                owner.template,
                                module.class_applications[original].template
                            );
                            assert_eq!(owner.arguments.len(), template.type_parameters.len());
                            assert_eq!(owner.arguments.len(), 2, "unused owner parameters remain");
                        }
                    }
                }
                for application in module.struct_constructor_applications.values() {
                    assert!(structs.insert((application.constructor, application.owner)));
                    let owner = &module.struct_applications[application.owner];
                    match application.constructor {
                        hir::StructConstructorDefinition::Local(_) => current_structs += 1,
                        hir::StructConstructorDefinition::Template(template) => {
                            imported_structs += 1;
                            let template = &module.imported_constructor_templates[template];
                            let hir::Type::Struct(original) = module.types[template.owner] else {
                                panic!("struct constructors retain their struct owner");
                            };
                            assert_eq!(
                                owner.template,
                                module.struct_applications[original].template
                            );
                            assert_eq!(owner.arguments.len(), template.type_parameters.len());
                            assert_eq!(owner.arguments.len(), 2, "unused owner parameters remain");
                        }
                    }
                }
                assert!(current_classes > 0 && current_structs > 0);
                if case == "combined" {
                    assert!(imported_classes > 0 && imported_structs > 0);
                } else {
                    assert_eq!((imported_classes, imported_structs), (0, 0));
                }
            },
        )
        .unwrap_or_else(|errors| panic!("{case}: {errors:?}"));
    }
}
