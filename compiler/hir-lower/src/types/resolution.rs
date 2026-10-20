use super::*;
mod names;

impl Lowerer {
    pub(crate) fn resolve_type_ref(&mut self, ty_ref: &ast::TypeRef) -> Option<TypeId> {
        let recording = self.begin_type_annotations(ty_ref);
        let ty = self.resolve_type_ref_shape(ty_ref);
        if recording {
            self.end_type_annotations(ty_ref, ty);
        }
        let ty = ty?;
        self.document_type_use(ty_ref, ty);
        if matches!(
            self.types[ty],
            Type::Struct(_) | Type::Class(_) | Type::Enum(_) | Type::Interface(_)
        ) {
            self.nominal_type_uses
                .push((ty, self.current_file, ty_ref.span));
        }
        Some(ty)
    }

    fn resolve_type_ref_shape(&mut self, ty_ref: &ast::TypeRef) -> Option<TypeId> {
        let ty = self.resolve_type_ref_unchecked(ty_ref)?;
        if !self.nominal_is_accessible(ty) {
            let name = self.type_name(ty);
            self.error(
                ty_ref.span,
                format!("type `{name}` is not accessible from this source location"),
            );
            return None;
        }
        Some(ty)
    }

    /// Resolve a type annotation (`Int`, `Point`, `(Int, String)`,
    /// `T?`, ...). Function (or enum) type parameters shadow well-known
    /// and declared types: they only scope over one function's
    /// signature/body or one enum's variants (`type_params_in_scope`
    /// is empty everywhere else).
    fn resolve_type_ref_unchecked(&mut self, ty_ref: &ast::TypeRef) -> Option<TypeId> {
        match &ty_ref.kind {
            ast::TypeRefKind::AppliedMember {
                owner,
                name,
                arguments,
            } => {
                let owner = self.resolve_type_ref(owner)?;
                self.resolve_applied_member_type(owner, name, arguments, ty_ref.span)
            }
            ast::TypeRefKind::Unit => Some(self.unit),
            ast::TypeRefKind::Generic(name, args) => {
                // `Name<T1, ...>`: generic type application. M4: only
                // generic enums. M5: the built-in `Array<T>` /
                // `MutableArray<T>`. M9: generic structs (the core GC
                // handle types).
                if let Some(index) = self
                    .type_params_in_scope
                    .iter()
                    .position(|param| param.name == name.text)
                {
                    self.error(
                        name.span,
                        format!(
                            "type parameter `{}` takes no type arguments",
                            self.type_params_in_scope[index].name
                        ),
                    );
                    return None;
                }
                if self.outer_type_parameter_owner(&name.text).is_some() {
                    self.error(
                        name.span,
                        if self.current_owner_is_companion() {
                            format!(
                                "companion object cannot use host type parameter `{}`",
                                name.text
                            )
                        } else {
                            format!(
                                "static nested declaration cannot use outer type parameter `{}`",
                                name.text
                            )
                        },
                    );
                    return None;
                }
                if let Some(target) = self.lexical_nested_nominal_target(&name.text) {
                    return self
                        .resolve_nested_nominal_application(target, args, name.span, &name.text);
                }
                match self.resolve_type_lookup(name).ok()? {
                    Some(crate::imports::lookup::TypeLookupTarget::Current(
                        crate::namespace::TopLevelTypeTarget::Annotation(_),
                    )) => {
                        self.error(
                            name.span,
                            format!("annotation `{}` cannot be used as a value type", name.text),
                        );
                        return None;
                    }
                    Some(crate::imports::lookup::TypeLookupTarget::Current(
                        crate::namespace::TopLevelTypeTarget::Alias(alias),
                    )) => return self.apply_source_type_alias(alias, name, args),
                    Some(crate::imports::lookup::TypeLookupTarget::Dependency(binding)) => {
                        return self.resolve_imported_generic_type_target(&binding, name, args);
                    }
                    Some(crate::imports::lookup::TypeLookupTarget::Current(
                        crate::namespace::TopLevelTypeTarget::Nominal(_),
                    ))
                    | None => {}
                }
                // Generic structs (M9, spec 3.2).
                if let Some((struct_id, _)) = self.top_level_struct_named(&name.text) {
                    let arity = self.structs[struct_id].type_params.len();
                    if arity == 0 {
                        self.error(name.span, format!("struct `{}` is not generic", name.text));
                        return None;
                    }
                    if arity != args.len() {
                        self.error(
                            name.span,
                            format!(
                                "struct `{}` takes {arity} type argument(s), but {} were supplied",
                                name.text,
                                args.len()
                            ),
                        );
                        return None;
                    }
                    let mut resolved = Vec::with_capacity(args.len());
                    for arg in args {
                        resolved.push(self.resolve_type_ref(arg)?);
                    }
                    let params = self.structs[struct_id].type_params.clone();
                    if !self.check_type_argument_kinds(
                        &params,
                        &resolved,
                        name.span,
                        &format!("struct `{}`", name.text),
                    ) {
                        return None;
                    }
                    if Some(struct_id) == self.ffi_ptr {
                        let pointee = resolved[0];
                        let ty = self.intern_type(Type::Ptr(pointee));
                        self.pointer_type_uses
                            .push((ty, self.current_file, name.span));
                        return Some(ty);
                    }
                    if Some(struct_id) == self.ffi_fun_ptr {
                        if self.allow_deferred_fun_ptr
                            && matches!(self.types[resolved[0]], Type::Param(_))
                        {
                            return Some(self.struct_application(struct_id, resolved));
                        }
                        let Type::Function(function_type) = self.types[resolved[0]] else {
                            self.error(
                                name.span,
                                "`FunPtr` type argument must be an ordinary concrete function type"
                                    .to_string(),
                            );
                            return None;
                        };
                        if !self.validate_fun_ptr_signature(function_type, name.span) {
                            return None;
                        }
                        let ty = self.intern_type(Type::FunPtr(function_type));
                        self.fun_ptr_type_uses
                            .push((ty, self.current_file, name.span));
                        return Some(ty);
                    }
                    return Some(self.struct_application(struct_id, resolved));
                }
                if let Some((interface_id, _)) = self.top_level_interface_named(&name.text) {
                    let arity = self.interfaces[interface_id].type_params.len();
                    if arity == 0 {
                        self.error(
                            name.span,
                            format!("interface `{}` is not generic", name.text),
                        );
                        return None;
                    }
                    if arity != args.len() {
                        self.error(
                            name.span,
                            format!(
                                "interface `{}` takes {arity} type argument(s), but {} were supplied",
                                name.text,
                                args.len()
                            ),
                        );
                        return None;
                    }
                    let mut resolved = Vec::with_capacity(args.len());
                    for arg in args {
                        resolved.push(self.resolve_type_ref(arg)?);
                    }
                    let params = self.interfaces[interface_id].type_params.clone();
                    if !self.check_type_argument_kinds(
                        &params,
                        &resolved,
                        name.span,
                        &format!("interface `{}`", name.text),
                    ) {
                        return None;
                    }
                    return Some(self.source_interface_type(interface_id, resolved));
                }
                if let Some((class_id, _)) = self.top_level_class_named(&name.text) {
                    let arity = self.classes[class_id].type_params.len();
                    if arity == 0 {
                        self.error(name.span, format!("class `{}` is not generic", name.text));
                        return None;
                    }
                    if arity != args.len() {
                        self.error(
                            name.span,
                            format!(
                                "class `{}` takes {arity} type argument(s), but {} were supplied",
                                name.text,
                                args.len()
                            ),
                        );
                        return None;
                    }
                    let mut resolved = Vec::with_capacity(args.len());
                    for arg in args {
                        resolved.push(self.resolve_type_ref(arg)?);
                    }
                    let params = self.classes[class_id].type_params.clone();
                    if !self.check_type_argument_kinds(
                        &params,
                        &resolved,
                        name.span,
                        &format!("class `{}`", name.text),
                    ) {
                        return None;
                    }
                    return Some(self.class_application(class_id, resolved));
                }
                if self.top_level_object_named(&name.text).is_some() {
                    self.error(name.span, format!("object `{}` is not generic", name.text));
                    return None;
                }
                let Some(enum_id) = self.top_level_enum_named(&name.text) else {
                    let what = if name.text == "Any" {
                        "type `Any` takes no type arguments".to_string()
                    } else {
                        format!("unknown type `{}`", name.text)
                    };
                    self.error(name.span, what);
                    return None;
                };
                let arity = self.enums[enum_id].type_params.len();
                if arity != args.len() {
                    let enum_name = self.enums[enum_id].name.clone();
                    self.error(
                        name.span,
                        format!(
                            "enum `{enum_name}` takes {arity} type argument(s), but {} were supplied",
                            args.len()
                        ),
                    );
                    return None;
                }
                let mut resolved = Vec::with_capacity(args.len());
                for arg in args {
                    resolved.push(self.resolve_type_ref(arg)?);
                }
                let params = self.enums[enum_id].type_params.clone();
                if !self.check_type_argument_kinds(
                    &params,
                    &resolved,
                    name.span,
                    &format!("enum `{}`", self.enums[enum_id].name),
                ) {
                    return None;
                }
                Some(self.enum_application(enum_id, resolved))
            }
            ast::TypeRefKind::Qualified { path, arguments } => {
                self.resolve_qualified_nominal(path, arguments, ty_ref.span)
            }
            ast::TypeRefKind::Named(name) => {
                if let Some(parameter) = self
                    .type_params_in_scope
                    .iter()
                    .find(|param| param.name == name.text)
                {
                    return Some(self.intern_type(Type::Param(parameter.id)));
                }
                if self.outer_type_parameter_owner(&name.text).is_some() {
                    self.error(
                        name.span,
                        if self.current_owner_is_companion() {
                            format!(
                                "companion object cannot use host type parameter `{}`",
                                name.text
                            )
                        } else {
                            format!(
                                "static nested declaration cannot use outer type parameter `{}`",
                                name.text
                            )
                        },
                    );
                    return None;
                }
                if let Some(target) = self.lexical_nested_nominal_target(&name.text) {
                    return self.resolve_nested_nominal_application(
                        target,
                        &[],
                        name.span,
                        &name.text,
                    );
                }
                match self.resolve_type_lookup(name).ok()? {
                    Some(crate::imports::lookup::TypeLookupTarget::Current(
                        crate::namespace::TopLevelTypeTarget::Annotation(_),
                    )) => {
                        self.error(
                            name.span,
                            format!("annotation `{}` cannot be used as a value type", name.text),
                        );
                        return None;
                    }
                    Some(crate::imports::lookup::TypeLookupTarget::Current(
                        crate::namespace::TopLevelTypeTarget::Alias(alias),
                    )) => {
                        return self.resolve_type_alias_id_reference(alias, name, false);
                    }
                    Some(crate::imports::lookup::TypeLookupTarget::Current(
                        crate::namespace::TopLevelTypeTarget::Nominal(target),
                    )) => {
                        return self.resolve_nested_nominal_application(
                            target,
                            &[],
                            name.span,
                            &name.text,
                        );
                    }
                    Some(crate::imports::lookup::TypeLookupTarget::Dependency(binding)) => {
                        return self.resolve_imported_dependency_type_target(&binding, name, false);
                    }
                    None => {}
                }
                match name.text.as_str() {
                    _ if matches!(self.core, crate::CoreLoweringAuthority::Imported(_))
                        && name.text != "Unit" =>
                    {
                        self.error(name.span, format!("unknown type `{}`", name.text));
                        None
                    }
                    "Unit" => Some(self.unit),
                    "Int8" => Some(self.integer_type(hir::IntegerKind::SIGNED_8)),
                    "Int16" => Some(self.integer_type(hir::IntegerKind::SIGNED_16)),
                    "Int" => Some(self.integer_type(hir::IntegerKind::SIGNED_32)),
                    "Long" => Some(self.integer_type(hir::IntegerKind::SIGNED_64)),
                    "UInt8" => Some(self.integer_type(hir::IntegerKind::UNSIGNED_8)),
                    "UInt16" => Some(self.integer_type(hir::IntegerKind::UNSIGNED_16)),
                    "UInt" => Some(self.integer_type(hir::IntegerKind::UNSIGNED_32)),
                    "ULong" => Some(self.integer_type(hir::IntegerKind::UNSIGNED_64)),
                    "Boolean" => Some(self.boolean),
                    "String" => Some(self.string),
                    _ => {
                        self.error(name.span, format!("unknown type `{}`", name.text));
                        None
                    }
                }
            }
            ast::TypeRefKind::Tuple(elements) => {
                let mut resolved = Vec::with_capacity(elements.len());
                for element in elements {
                    resolved.push(self.resolve_type_ref(element)?);
                }
                Some(self.intern_type(Type::Tuple(resolved)))
            }
            ast::TypeRefKind::Function(function) => {
                let mut parameter_types = Vec::with_capacity(function.parameters.len());
                for parameter in &function.parameters {
                    parameter_types.push(self.resolve_type_ref(parameter)?);
                }
                let return_type = self.resolve_type_ref(&function.return_type)?;
                let receiver = match &function.receiver {
                    Some(ty) => Some(self.resolve_type_ref(ty)?),
                    None => None,
                };
                let function = self.intern_receiver_function_type(
                    function.is_suspend,
                    receiver,
                    parameter_types,
                    return_type,
                );
                Some(self.function_types[function].canonical_type)
            }
            // `T?` desugars to `Option<T>` (spec 7.1) — the core
            // library's generic enum since M4. `T??` is
            // `Option<Option<T>>` and deliberately does not collapse.
            ast::TypeRefKind::Nullable(inner) => {
                let inner = self.resolve_type_ref(inner)?;
                match self.has_option_protocol() {
                    true => Some(self.option_type(inner)),
                    false => {
                        self.error(
                            ty_ref.span,
                            "`T?` requires `Option<T>` from scoop.core, which is not defined"
                                .to_string(),
                        );
                        None
                    }
                }
            }
        }
    }
}
