use super::*;

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
