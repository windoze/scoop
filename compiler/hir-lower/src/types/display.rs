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
                objects: &self.objects,
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
    objects: &'a Arena<hir::ObjectDecl>,
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
            let name = nominal_name(
                structs,
                enums,
                classes,
                interfaces,
                applications.objects,
                &structs[id].name,
                structs[id].owner,
            );
            if args.is_empty() {
                name
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
        Type::Class(application) => {
            let application = &applications.classes[*application];
            let id = application.template;
            let args = &application.arguments;
            let name = nominal_name(
                structs,
                enums,
                classes,
                interfaces,
                applications.objects,
                &classes[id].name,
                classes[id].owner,
            );
            if args.is_empty() {
                name
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
                format!("{name}<{inner}>")
            }
        }
        Type::Interface(application) => {
            let application = &applications.interfaces[*application];
            let id = application.template;
            let args = &application.arguments;
            let name = nominal_name(
                structs,
                enums,
                classes,
                interfaces,
                applications.objects,
                &interfaces[id].name,
                interfaces[id].owner,
            );
            if args.is_empty() {
                name
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
            let id = application.template;
            let name = nominal_name(
                structs,
                enums,
                classes,
                interfaces,
                applications.objects,
                &enums[id].name,
                enums[id].owner,
            );
            let args = &application.arguments;
            if args.is_empty() {
                name
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

fn nominal_name(
    structs: &Arena<StructDecl>,
    enums: &Arena<EnumDecl>,
    classes: &Arena<ClassDecl>,
    interfaces: &Arena<InterfaceDecl>,
    objects: &Arena<hir::ObjectDecl>,
    name: &str,
    owner: Option<hir::NominalOwner>,
) -> String {
    let Some(owner) = owner else {
        return name.to_string();
    };
    let prefix = match owner {
        hir::NominalOwner::Class(id) => nominal_name(
            structs,
            enums,
            classes,
            interfaces,
            objects,
            &classes[id].name,
            classes[id].owner,
        ),
        hir::NominalOwner::Interface(id) => nominal_name(
            structs,
            enums,
            classes,
            interfaces,
            objects,
            &interfaces[id].name,
            interfaces[id].owner,
        ),
        hir::NominalOwner::Struct(id) => nominal_name(
            structs,
            enums,
            classes,
            interfaces,
            objects,
            &structs[id].name,
            structs[id].owner,
        ),
        hir::NominalOwner::Enum(id) => nominal_name(
            structs,
            enums,
            classes,
            interfaces,
            objects,
            &enums[id].name,
            enums[id].owner,
        ),
        hir::NominalOwner::Object(id) => nominal_name(
            structs,
            enums,
            classes,
            interfaces,
            objects,
            &objects[id].name,
            objects[id].owner,
        ),
    };
    format!("{prefix}.{name}")
}
