use super::super::*;

/// Render a type for dumps.
pub fn type_name(module: &Module, ty: &Type) -> String {
    match ty {
        Type::Context(storage) => format!("internal<{}>", storage.role.name()),
        Type::Unit => "Unit".to_string(),
        Type::Integer(kind) => kind.canonical_name().to_string(),
        Type::MachineScalar(kind) => format!("machine<{}>", kind.name()),
        Type::Boolean => "Boolean".to_string(),
        Type::String => "String".to_string(),
        Type::Struct(id) => nominal_name(
            module,
            &module.structs[*id].name,
            &module.structs[*id].type_arguments,
        ),
        Type::Class(id) => match &module.classes[*id].representation {
            ClassRepresentation::Intrinsic(IntrinsicTypeRepresentation::Array { element }) => {
                format!("Array<{}>", type_name(module, element))
            }
            ClassRepresentation::Intrinsic(IntrinsicTypeRepresentation::MutableArray {
                element,
            }) => format!("MutableArray<{}>", type_name(module, element)),
            ClassRepresentation::Intrinsic(IntrinsicTypeRepresentation::Atomic(storage)) => {
                match storage {
                    scoop_identity::AtomicStorage::Reference(value) => {
                        format!("AtomicRef<{}>", type_name(module, value))
                    }
                    _ => storage.kind().source_name().to_owned(),
                }
            }
            ClassRepresentation::Declared { .. }
            | ClassRepresentation::Intrinsic(
                IntrinsicTypeRepresentation::String
                | IntrinsicTypeRepresentation::Any
                | IntrinsicTypeRepresentation::Nothing,
            ) => nominal_name(
                module,
                &module.classes[*id].name,
                &module.classes[*id].type_arguments,
            ),
            ClassRepresentation::Intrinsic(_) => {
                unreachable!("the intrinsic registry fixes declaration targets")
            }
        },
        Type::Interface(id) => nominal_name(
            module,
            &module.interfaces[*id].name,
            &module.interfaces[*id].type_arguments,
        ),
        Type::Any => "Any".to_string(),
        Type::Tuple(elements) => {
            let inner: Vec<String> = elements.iter().map(|t| type_name(module, t)).collect();
            format!("({})", inner.join(", "))
        }
        Type::Function(id) => {
            let function = &module.function_types[*id];
            let parameters: Vec<String> = function
                .parameter_types
                .iter()
                .map(|ty| type_name(module, ty))
                .collect();
            let suspend = if function.is_suspend { "suspend " } else { "" };
            format!(
                "{suspend}({}) -> {}",
                parameters.join(", "),
                type_name(module, &function.return_type)
            )
        }
        Type::Ptr(inner) => format!("Ptr<{}>", type_name(module, inner)),
        Type::FunPtr(id) => {
            let function = &module.function_types[*id];
            let parameters: Vec<_> = function
                .parameter_types
                .iter()
                .map(|ty| type_name(module, ty))
                .collect();
            format!(
                "FunPtr<({}) -> {}>",
                parameters.join(", "),
                type_name(module, &function.return_type)
            )
        }
        Type::Enum(id, args) => {
            let name = &module.enums[*id].name;
            nominal_name(module, name, args)
        }
    }
}

fn nominal_name(module: &Module, name: &str, arguments: &[Type]) -> String {
    if arguments.is_empty() {
        name.to_string()
    } else {
        let arguments = arguments
            .iter()
            .map(|argument| type_name(module, argument))
            .collect::<Vec<_>>()
            .join(", ");
        format!("{name}<{arguments}>")
    }
}
