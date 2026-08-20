use super::*;

#[test]
fn abstract_signatures_keep_parameter_locals_without_source_bodies() {
    with_provider_consumer(
        &fixture("provider"),
        &fixture("standalone"),
        |output, _, _, _, _| {
            let export = output.output().export.module();
            let declarations = export
                .functions
                .iter()
                .filter_map(|(_, function)| match &function.kind {
                    hir::FunctionKind::Abstract { locals } => Some((function, locals)),
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(declarations.len(), 6);
            for (function, locals) in declarations {
                assert_eq!(locals.len(), function.params.len());
                for parameter in &function.params {
                    assert_eq!(locals[parameter.local].ty, parameter.ty);
                }
            }
            let concrete = output.output().local.module();
            let mut has_abstract_class_method = false;
            for (id, function) in concrete.functions.iter() {
                if let hir::concrete::FunctionKind::Abstract { locals } = &function.kind {
                    has_abstract_class_method |= function.name == "BaseReader.stored";
                    assert_eq!(locals.len(), function.params.len());
                    for parameter in &function.params {
                        assert_eq!(locals[parameter.local].ty, parameter.ty);
                        concrete
                            .local_value_identities
                            .function_local(id, parameter.local);
                    }
                }
            }
            assert!(has_abstract_class_method);
        },
    )
    .unwrap();
}

#[test]
fn dependency_abstract_slots_compose_with_generic_overrides_and_references() {
    with_provider_consumer(
        &fixture("provider"),
        &fixture("combined"),
        |output, _, _, _, _| {
            let mut abstract_count = 0;
            let mut abstract_names = Vec::new();
            for (_, template) in output
                .output()
                .export
                .module()
                .imported_generic_templates
                .iter()
            {
                if matches!(
                    template.declaration,
                    hir::ImportedCallableTemplateOrigin::Nominal {
                        modifier: hir::MethodModifier::Abstract,
                        ..
                    }
                ) {
                    let hir::FunctionKind::Abstract { locals } = &template.implementation else {
                        panic!("a dependency abstract slot cannot become an executable body");
                    };
                    abstract_count += 1;
                    abstract_names.push(template.name.as_str());
                    assert_eq!(locals.len(), template.params.len());
                }
            }
            assert!(abstract_count > 0);
            for name in [
                "BaseReader.stored",
                "BaseReader.$get$value",
                "BaseReader.$set$value",
            ] {
                assert!(
                    abstract_names.contains(&name),
                    "missing abstract slot {name}"
                );
            }
        },
    )
    .unwrap();
}

fn fixture(name: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/m23-shared-abstract")
            .join(format!("{name}.scoop")),
    )
    .unwrap()
}
