use super::super::*;

/// Render a type for dumps.
pub fn type_name(module: &Module, ty: &Type) -> String {
    match ty {
        Type::Unit => "Unit".to_string(),
        Type::Int => "Int".to_string(),
        Type::UInt => "UInt".to_string(),
        Type::MachineScalar(kind) => format!("machine<{}>", kind.name()),
        Type::Boolean => "Boolean".to_string(),
        Type::String => "String".to_string(),
        Type::Struct(id) => module.structs[*id].name.clone(),
        Type::Class(id) => match &module.classes[*id].representation {
            ClassRepresentation::Intrinsic(IntrinsicTypeRepresentation::Array { element }) => {
                format!("Array<{}>", type_name(module, element))
            }
            ClassRepresentation::Intrinsic(IntrinsicTypeRepresentation::MutableArray {
                element,
            }) => format!("MutableArray<{}>", type_name(module, element)),
            ClassRepresentation::Declared { .. }
            | ClassRepresentation::Intrinsic(IntrinsicTypeRepresentation::String) => {
                module.classes[*id].name.clone()
            }
            ClassRepresentation::Intrinsic(_) => {
                unreachable!("the intrinsic registry fixes declaration targets")
            }
        },
        Type::Interface(id) => module.interfaces[*id].name.clone(),
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
            if args.is_empty() {
                name.clone()
            } else {
                let inner: Vec<String> = args.iter().map(|t| type_name(module, t)).collect();
                format!("{}<{}>", name, inner.join(", "))
            }
        }
    }
}
