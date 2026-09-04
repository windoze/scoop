use super::*;

impl Lowerer {
    pub(crate) fn resolve_type_ref(&mut self, ty_ref: &ast::TypeRef) -> Option<TypeId> {
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
                // Generic structs (M9, spec 3.2).
                if let Some(&(struct_id, _)) = self.structs_by_name.get(&name.text) {
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
                        if self.type_contains_param(pointee)
                            && self.current_file >= self.user_file_index
                        {
                            self.error(
                                name.span,
                                "`Ptr` pointee must be a concrete GC-free value type".to_string(),
                            );
                            return None;
                        }
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
                        let function = &self.function_types[function_type];
                        if function.is_suspend || self.function_type_contains_param(function_type) {
                            self.error(
                                name.span,
                                "`FunPtr` type argument must be an ordinary concrete function type"
                                    .to_string(),
                            );
                            return None;
                        }
                        let ty = self.intern_type(Type::FunPtr(function_type));
                        self.fun_ptr_type_uses
                            .push((ty, self.current_file, name.span));
                        return Some(ty);
                    }
                    return Some(self.struct_application(struct_id, resolved));
                }
                if let Some(&(interface_id, _)) = self.interfaces_by_name.get(&name.text) {
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
                    return Some(self.intern_interface_application(interface_id, resolved));
                }
                if let Some(&(class_id, _)) = self.classes_by_name.get(&name.text) {
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
                let Some(&enum_id) = self.enums_by_name.get(&name.text) else {
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
            ast::TypeRefKind::Named(name) => {
                if let Some(parameter) = self
                    .type_params_in_scope
                    .iter()
                    .find(|param| param.name == name.text)
                {
                    return Some(self.intern_type(Type::Param(parameter.id)));
                }
                match name.text.as_str() {
                    "Unit" => Some(self.unit),
                    "Int" => Some(self.int),
                    "UInt" => Some(self.uint),
                    "Boolean" => Some(self.boolean),
                    "String" => Some(self.string),
                    // `Any` is a compiler built-in (milestone6 DESIGN.md
                    // 5.5); the core library shape arrives with M7/core.
                    "Any" => Some(self.any),
                    _ => {
                        if let Some(&(struct_id, ty)) = self.structs_by_name.get(&name.text) {
                            // A generic struct needs its type
                            // arguments (`PinnedPtr<T>` goes through
                            // TypeRefKind::Generic), mirroring the
                            // generic enum rule below.
                            let arity = self.structs[struct_id].type_params.len();
                            if arity != 0 {
                                self.error(
                                    name.span,
                                    format!(
                                        "generic struct `{}` requires {arity} type argument(s)",
                                        name.text
                                    ),
                                );
                                return None;
                            }
                            return Some(ty);
                        }
                        if let Some(&(class_id, ty)) = self.classes_by_name.get(&name.text) {
                            let arity = self.classes[class_id].type_params.len();
                            if arity != 0 {
                                self.error(
                                    name.span,
                                    format!(
                                        "generic class `{}` requires {arity} type argument(s)",
                                        name.text
                                    ),
                                );
                                return None;
                            }
                            return Some(ty);
                        }
                        if let Some(&(interface_id, ty)) = self.interfaces_by_name.get(&name.text) {
                            let arity = self.interfaces[interface_id].type_params.len();
                            if arity != 0 {
                                self.error(
                                    name.span,
                                    format!(
                                        "generic interface `{}` requires {arity} type argument(s)",
                                        name.text
                                    ),
                                );
                                return None;
                            }
                            return Some(ty);
                        }
                        match self.enums_by_name.get(&name.text) {
                            Some(&id) => {
                                // Bare name without type arguments:
                                // only non-generic enums are usable
                                // (`Option<Int>` / `Box<Int>` go
                                // through TypeRefKind::Generic).
                                let arity = self.enums[id].type_params.len();
                                if arity == 0 {
                                    Some(self.enum_application(id, Vec::new()))
                                } else {
                                    let enum_name = self.enums[id].name.clone();
                                    self.error(
                                        name.span,
                                        format!(
                                            "generic enum `{enum_name}` requires {arity} type argument(s)"
                                        ),
                                    );
                                    None
                                }
                            }
                            None => {
                                self.error(name.span, format!("unknown type `{}`", name.text));
                                None
                            }
                        }
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
                Some(self.intern_function_type(function.is_suspend, parameter_types, return_type))
            }
            // `T?` desugars to `Option<T>` (spec 7.1) — the core
            // library's generic enum since M4. `T??` is
            // `Option<Option<T>>` and deliberately does not collapse.
            ast::TypeRefKind::Nullable(inner) => {
                let inner = self.resolve_type_ref(inner)?;
                match self.option_enum {
                    Some(_) => Some(self.option_type(inner)),
                    None => {
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
