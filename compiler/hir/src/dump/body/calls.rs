use super::*;

pub(super) fn lexical_function_name(
    module: &Module,
    definition: LexicalFunctionDefinition,
) -> &str {
    match definition {
        LexicalFunctionDefinition::Source { function, .. } => &module.functions[function].name,
        LexicalFunctionDefinition::Template(template) => {
            &module.imported_generic_templates[template].name
        }
    }
}

pub(super) fn callable_target_name(module: &Module, callee: CallableTarget) -> String {
    match callee {
        CallableTarget::Local(callable) => {
            let (function, arguments) = callable_dump_parts(module, callable);
            let name = &module.functions[function].name;
            if arguments.is_empty() {
                name.clone()
            } else {
                format!("{name}<{}>", type_arguments(module, &arguments))
            }
        }
        CallableTarget::Application(application) => callable_application_name(module, application),
        CallableTarget::Dependency(callee) => {
            let dispatch = match module.imported_dependency_callables[callee].dispatch() {
                ImportedDependencyDispatch::Direct => String::new(),
                ImportedDependencyDispatch::Virtual { slot } => format!(" virtual[{slot}]"),
                ImportedDependencyDispatch::Interface { slot, .. } => format!(" interface[{slot}]"),
            };
            format!("external #{}{dispatch}", callee.into_raw().into_u32())
        }
    }
}

pub(super) fn callable_application_name(
    module: &Module,
    application: ImportedGenericCallableApplicationId,
) -> String {
    let application = &module.imported_generic_applications[application];
    let template = &module.imported_generic_templates[application.template];
    let arguments = application.arguments.substitution(
        &module.types,
        &module.enum_applications,
        &module.struct_applications,
        &module.class_applications,
        &module.interface_applications,
    );
    format!("{}<{}>", template.name, type_arguments(module, &arguments))
}

fn type_arguments(module: &Module, arguments: &[TypeId]) -> String {
    arguments
        .iter()
        .map(|ty| type_name(module, *ty))
        .collect::<Vec<_>>()
        .join(", ")
}

pub(super) fn method_callee_name(module: &Module, callee: MethodCallee) -> String {
    if let MethodCallee::ImportedDerivedEquality { target, .. } = callee {
        return format!(
            "external derived {}",
            module.imported_derived_equalities[target].callable()
        );
    }
    match callee.declared_callable(&module.bound_callable_refs) {
        Some(CallableTarget::Local(callable)) => module.functions
            [callable_function(module, callable)]
        .name
        .clone(),
        Some(target) => callable_target_name(module, target),
        None => {
            let MethodCallee::DerivedEquality(application) = callee else {
                unreachable!("a method without a declaration is derived equality")
            };
            module.functions[module.derived_equality_applications[application].function]
                .name
                .clone()
        }
    }
}
