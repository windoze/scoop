use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    Unit,
    Int,
    /// Unsigned 64-bit integer (`UInt`, spec 11.2).
    UInt,
    Boolean,
    String,
    /// A struct type with resolved type arguments (empty for
    /// non-generic structs). Keeping the arguments in the type itself
    /// makes every `TypeId` structurally complete.
    Struct(StructApplicationId),
    /// A reference type declared with `class` (spec 9.1).
    /// A class application with complete host arguments (empty for a
    /// non-generic class). M14 gives generic classes the same nominal
    /// application semantics as the other declaration kinds.
    Class(ClassApplicationId),
    /// An interface application with complete type arguments (empty for a
    /// non-generic interface). Values behind it are references.
    Interface(InterfaceApplicationId),
    /// The root of all types (spec 3.1). Value types reaching it are
    /// boxed (spec 4.4.4).
    Any,
    Tuple(Vec<TypeId>),
    /// A managed function value type. The referenced entry carries the
    /// complete structural signature and is canonical within the Cone.
    Function(FunctionTypeId),
    /// A GC-free typed raw data pointer. The pointee remains explicit in all
    /// IR stages; it is never recovered from an integer representation.
    Ptr(TypeId),
    /// A GC-free native C function pointer. Its signature reuses M11's
    /// canonical function-type identity but is not a managed function value.
    FunPtr(FunctionTypeId),
    /// An enum type with resolved type arguments (empty for
    /// non-generic enums). `Option<T>` is one of these since M4
    /// (defined in `scoop.core`).
    Enum(EnumApplicationId),
    /// A type parameter, by typed local index. Only appears inside a
    /// generic function/type definition; instantiated MIR never
    /// contains it.
    Param(TypeParamId),
}

/// Canonical structural identity of an ordinary or suspend function type.
/// Declaration-only metadata such as parameter names/defaults is absent by
/// construction (spec 8.1.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionType {
    /// The unique ordinary managed-function type represented by this
    /// signature. Consumers use this edge directly; they never scan `types`
    /// to recover the reverse mapping.
    pub canonical_type: TypeId,
    pub is_suspend: bool,
    pub parameter_types: Vec<TypeId>,
    pub return_type: TypeId,
}

/// Structural type equality (tuple types are compared by elements,
/// enum types by identity plus arguments).
pub fn types_equal(module: &Module, a: TypeId, b: TypeId) -> bool {
    match (&module.types[a], &module.types[b]) {
        (Type::Unit, Type::Unit)
        | (Type::Int, Type::Int)
        | (Type::UInt, Type::UInt)
        | (Type::Boolean, Type::Boolean)
        | (Type::String, Type::String) => true,
        (Type::Struct(x), Type::Struct(y)) => x == y,
        (Type::Class(x), Type::Class(y)) => x == y,
        (Type::Interface(x), Type::Interface(y)) => x == y,
        (Type::Any, Type::Any) => true,
        (Type::Tuple(xs), Type::Tuple(ys)) => {
            xs.len() == ys.len()
                && xs
                    .iter()
                    .zip(ys.iter())
                    .all(|(x, y)| types_equal(module, *x, *y))
        }
        (Type::Function(x), Type::Function(y)) => x == y,
        (Type::Ptr(x), Type::Ptr(y)) => types_equal(module, *x, *y),
        (Type::FunPtr(x), Type::FunPtr(y)) => x == y,
        (Type::Enum(x), Type::Enum(y)) => x == y,
        (Type::Param(x), Type::Param(y)) => x == y,
        _ => false,
    }
}

/// Render a type for diagnostics and dumps.
pub fn type_name(module: &Module, ty: TypeId) -> String {
    type_name_with_params(module, ty, &[])
}

pub(crate) fn type_name_with_params(
    module: &Module,
    ty: TypeId,
    params: &[TypeParamDecl],
) -> String {
    match &module.types[ty] {
        Type::Unit => "Unit".to_string(),
        Type::Int => "Int".to_string(),
        Type::UInt => "UInt".to_string(),
        Type::Boolean => "Boolean".to_string(),
        Type::String => "String".to_string(),
        Type::Struct(application) => {
            let application = &module.struct_applications[*application];
            let name = nominal_declaration_name(
                module,
                &module.structs[application.template].name,
                module.structs[application.template].owner,
            );
            let args = &application.arguments;
            if args.is_empty() {
                name
            } else {
                let inner: Vec<String> = args
                    .iter()
                    .map(|t| type_name_with_params(module, *t, params))
                    .collect();
                format!("{}<{}>", name, inner.join(", "))
            }
        }
        Type::Class(application) => {
            let application = &module.class_applications[*application];
            let name = nominal_declaration_name(
                module,
                &module.classes[application.template].name,
                module.classes[application.template].owner,
            );
            let args = &application.arguments;
            if args.is_empty() {
                name
            } else {
                let inner: Vec<String> = args
                    .iter()
                    .map(|ty| type_name_with_params(module, *ty, params))
                    .collect();
                format!("{}<{}>", name, inner.join(", "))
            }
        }
        Type::Interface(application) => {
            let application = &module.interface_applications[*application];
            let name = nominal_declaration_name(
                module,
                &module.interfaces[application.template].name,
                module.interfaces[application.template].owner,
            );
            let args = &application.arguments;
            if args.is_empty() {
                name
            } else {
                let inner: Vec<String> = args
                    .iter()
                    .map(|t| type_name_with_params(module, *t, params))
                    .collect();
                format!("{}<{}>", name, inner.join(", "))
            }
        }
        Type::Any => "Any".to_string(),
        Type::Enum(application) => {
            let application = &module.enum_applications[*application];
            let name = nominal_declaration_name(
                module,
                &module.enums[application.template].name,
                module.enums[application.template].owner,
            );
            let args = &application.arguments;
            if args.is_empty() {
                name
            } else {
                let inner: Vec<String> = args
                    .iter()
                    .map(|t| type_name_with_params(module, *t, params))
                    .collect();
                format!("{}<{}>", name, inner.join(", "))
            }
        }
        Type::Tuple(elements) => {
            let inner: Vec<String> = elements
                .iter()
                .map(|t| type_name_with_params(module, *t, params))
                .collect();
            format!("({})", inner.join(", "))
        }
        Type::Function(id) => {
            let function = &module.function_types[*id];
            let parameters: Vec<String> = function
                .parameter_types
                .iter()
                .map(|ty| type_name_with_params(module, *ty, params))
                .collect();
            let suspend = if function.is_suspend { "suspend " } else { "" };
            format!(
                "{suspend}({}) -> {}",
                parameters.join(", "),
                type_name_with_params(module, function.return_type, params)
            )
        }
        Type::Ptr(pointee) => format!("Ptr<{}>", type_name_with_params(module, *pointee, params)),
        Type::FunPtr(id) => {
            let function = &module.function_types[*id];
            let parameters = function
                .parameter_types
                .iter()
                .map(|ty| type_name_with_params(module, *ty, params))
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "FunPtr<({parameters}) -> {}>",
                type_name_with_params(module, function.return_type, params)
            )
        }
        Type::Param(index) => params
            .iter()
            .find(|parameter| parameter.id == *index)
            .map(|parameter| parameter.name.clone())
            .unwrap_or_else(|| format!("T{}", index.into_raw())),
    }
}

pub(crate) fn nominal_declaration_name(
    module: &Module,
    name: &str,
    owner: Option<NominalOwner>,
) -> String {
    let Some(owner) = owner else {
        return name.to_string();
    };
    let prefix = match owner {
        NominalOwner::Class(id) => {
            nominal_declaration_name(module, &module.classes[id].name, module.classes[id].owner)
        }
        NominalOwner::Interface(id) => nominal_declaration_name(
            module,
            &module.interfaces[id].name,
            module.interfaces[id].owner,
        ),
        NominalOwner::Struct(id) => {
            nominal_declaration_name(module, &module.structs[id].name, module.structs[id].owner)
        }
        NominalOwner::Enum(id) => {
            nominal_declaration_name(module, &module.enums[id].name, module.enums[id].owner)
        }
    };
    format!("{prefix}.{name}")
}
