use super::*;

impl Lowerer {
    /// Structural type equality (tuple and enum types compare
    /// elementwise; generic struct applications compare by struct and
    /// argument list, so `PinnedPtr<Int>` and `PinnedPtr<String>` are
    /// different types — just as `Int` and `UInt` are, spec 11.2).
    pub(crate) fn types_equal(&self, a: TypeId, b: TypeId) -> bool {
        type_value_equal(&self.types, a, b)
    }

    /// Render a type for diagnostics. Type parameters render with
    /// their declared name while the owning function or enum is in
    /// scope.
    pub(crate) fn type_name(&self, ty: TypeId) -> String {
        self.type_name_with_params(ty, &self.type_params_in_scope)
    }

    /// Render a declaration type with the declaration's own parameter names.
    /// Candidate diagnostics use this instead of the caller's lexical generic
    /// namespace so every displayed source signature remains self-contained.
    pub(crate) fn type_name_with_params(
        &self,
        ty: TypeId,
        type_params: &[hir::TypeParamDecl],
    ) -> String {
        type_name(
            &self.types,
            &self.function_types,
            &self.structs,
            &self.enums,
            &self.classes,
            &self.interfaces,
            NominalApplications {
                structs: &self.struct_applications,
                enums: &self.enum_applications,
                classes: &self.class_applications,
                interfaces: &self.interface_applications,
            },
            type_params,
            ty,
        )
    }
}

#[derive(Clone, Copy)]
struct NominalApplications<'a> {
    structs: &'a Arena<hir::StructApplication>,
    enums: &'a Arena<hir::EnumApplication>,
    classes: &'a Arena<hir::ClassApplication>,
    interfaces: &'a Arena<hir::InterfaceApplication>,
}

#[allow(clippy::too_many_arguments)]
fn type_name(
    types: &Arena<Type>,
    function_types: &Arena<hir::FunctionType>,
    structs: &Arena<StructDecl>,
    enums: &Arena<EnumDecl>,
    classes: &Arena<ClassDecl>,
    interfaces: &Arena<InterfaceDecl>,
    applications: NominalApplications<'_>,
    type_params: &[hir::TypeParamDecl],
    ty: TypeId,
) -> String {
    match &types[ty] {
        Type::Unit => "Unit".to_string(),
        Type::Int => "Int".to_string(),
        Type::UInt => "UInt".to_string(),
        Type::Boolean => "Boolean".to_string(),
        Type::String => "String".to_string(),
        Type::Struct(application) => {
            let application = &applications.structs[*application];
            let id = application.template;
            let args = &application.arguments;
            if args.is_empty() {
                structs[id].name.clone()
            } else {
                let inner: Vec<String> = args
                    .iter()
                    .map(|t| {
                        type_name(
                            types,
                            function_types,
                            structs,
                            enums,
                            classes,
                            interfaces,
                            applications,
                            type_params,
                            *t,
                        )
                    })
                    .collect();
                format!("{}<{}>", structs[id].name, inner.join(", "))
            }
        }
        Type::Class(application) => {
            let application = &applications.classes[*application];
            let id = application.template;
            let args = &application.arguments;
            if args.is_empty() {
                classes[id].name.clone()
            } else {
                let inner = args
                    .iter()
                    .map(|ty| {
                        type_name(
                            types,
                            function_types,
                            structs,
                            enums,
                            classes,
                            interfaces,
                            applications,
                            type_params,
                            *ty,
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{}<{inner}>", classes[id].name)
            }
        }
        Type::Interface(application) => {
            let application = &applications.interfaces[*application];
            let id = application.template;
            let args = &application.arguments;
            if args.is_empty() {
                interfaces[id].name.clone()
            } else {
                let inner: Vec<String> = args
                    .iter()
                    .map(|t| {
                        type_name(
                            types,
                            function_types,
                            structs,
                            enums,
                            classes,
                            interfaces,
                            applications,
                            type_params,
                            *t,
                        )
                    })
                    .collect();
                format!("{}<{}>", interfaces[id].name, inner.join(", "))
            }
        }
        Type::Any => "Any".to_string(),
        Type::Ptr(pointee) => {
            let inner = type_name(
                types,
                function_types,
                structs,
                enums,
                classes,
                interfaces,
                applications,
                type_params,
                *pointee,
            );
            format!("Ptr<{inner}>")
        }
        Type::FunPtr(id) => {
            let function = &function_types[*id];
            let parameters: Vec<_> = function
                .parameter_types
                .iter()
                .map(|ty| {
                    type_name(
                        types,
                        function_types,
                        structs,
                        enums,
                        classes,
                        interfaces,
                        applications,
                        type_params,
                        *ty,
                    )
                })
                .collect();
            let return_type = type_name(
                types,
                function_types,
                structs,
                enums,
                classes,
                interfaces,
                applications,
                type_params,
                function.return_type,
            );
            let suspend = if function.is_suspend { "suspend " } else { "" };
            format!(
                "FunPtr<{suspend}({}) -> {return_type}>",
                parameters.join(", ")
            )
        }
        Type::Enum(application) => {
            let application = &applications.enums[*application];
            let name = &enums[application.template].name;
            let args = &application.arguments;
            if args.is_empty() {
                name.clone()
            } else {
                let inner: Vec<String> = args
                    .iter()
                    .map(|t| {
                        type_name(
                            types,
                            function_types,
                            structs,
                            enums,
                            classes,
                            interfaces,
                            applications,
                            type_params,
                            *t,
                        )
                    })
                    .collect();
                format!("{}<{}>", name, inner.join(", "))
            }
        }
        Type::Tuple(elements) => {
            let inner: Vec<String> = elements
                .iter()
                .map(|t| {
                    type_name(
                        types,
                        function_types,
                        structs,
                        enums,
                        classes,
                        interfaces,
                        applications,
                        type_params,
                        *t,
                    )
                })
                .collect();
            format!("({})", inner.join(", "))
        }
        Type::Function(id) => {
            let function = &function_types[*id];
            let parameters: Vec<String> = function
                .parameter_types
                .iter()
                .map(|ty| {
                    type_name(
                        types,
                        function_types,
                        structs,
                        enums,
                        classes,
                        interfaces,
                        applications,
                        type_params,
                        *ty,
                    )
                })
                .collect();
            let return_type = type_name(
                types,
                function_types,
                structs,
                enums,
                classes,
                interfaces,
                applications,
                type_params,
                function.return_type,
            );
            let suspend = if function.is_suspend { "suspend " } else { "" };
            format!("{suspend}({}) -> {return_type}", parameters.join(", "))
        }
        Type::Param(index) => type_params
            .iter()
            .find(|parameter| parameter.id == *index)
            .map(|param| param.name.clone())
            .unwrap_or_else(|| format!("T{}", index.into_raw())),
    }
}
