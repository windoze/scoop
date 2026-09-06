use super::*;
use crate::{NominalTarget, Owner};

impl Lowerer {
    pub(crate) fn top_level_nominal_target(&self, name: &str) -> Option<NominalTarget> {
        self.structs_by_name
            .get(name)
            .map(|(id, _)| NominalTarget::Struct(*id))
            .or_else(|| {
                self.enums_by_name
                    .get(name)
                    .copied()
                    .map(NominalTarget::Enum)
            })
            .or_else(|| {
                self.classes_by_name
                    .get(name)
                    .map(|(id, _)| NominalTarget::Class(*id))
            })
            .or_else(|| {
                self.interfaces_by_name
                    .get(name)
                    .map(|(id, _)| NominalTarget::Interface(*id))
            })
            .or_else(|| {
                self.objects_by_name
                    .get(name)
                    .copied()
                    .map(NominalTarget::Object)
            })
    }

    pub(crate) fn nested_nominal_target(&self, owner: Owner, name: &str) -> Option<NominalTarget> {
        self.nested_nominals_by_owner
            .get(&(owner, name.to_string()))
            .copied()
    }

    fn nominal_parent(&self, owner: Owner) -> Option<Owner> {
        let parent = match owner {
            Owner::Class(id) => self.classes[id].owner,
            Owner::Interface(id) => self.interfaces[id].owner,
            Owner::Struct(id) => self.structs[id].owner,
            Owner::Enum(id) => self.enums[id].owner,
            Owner::Object(id) => self.objects[id].owner,
        }?;
        Some(Owner::from_nominal_owner(parent))
    }

    pub(crate) fn lexical_nested_nominal_target(&self, name: &str) -> Option<NominalTarget> {
        let mut owner = self.current_owner?;
        loop {
            if let Some(target) = self
                .nested_nominals_by_owner
                .get(&(owner, name.to_string()))
            {
                return Some(*target);
            }
            owner = self.nominal_parent(owner)?;
        }
    }

    fn outer_type_parameter_owner(&self, name: &str) -> Option<Owner> {
        let mut owner = self.nominal_parent(self.current_owner?)?;
        loop {
            let params: &[hir::TypeParamDecl] = match owner {
                Owner::Class(id) => &self.classes[id].type_params,
                Owner::Interface(id) => &self.interfaces[id].type_params,
                Owner::Struct(id) => &self.structs[id].type_params,
                Owner::Enum(id) => &self.enums[id].type_params,
                Owner::Object(_) => &[],
            };
            if params.iter().any(|parameter| parameter.name == name) {
                return Some(owner);
            }
            owner = self.nominal_parent(owner)?;
        }
    }

    fn resolve_nested_nominal_application(
        &mut self,
        target: NominalTarget,
        arguments: &[ast::TypeRef],
        span: ast::Span,
        display_name: &str,
    ) -> Option<TypeId> {
        let (kind, params) = match target {
            NominalTarget::Struct(id) => ("struct", self.structs[id].type_params.clone()),
            NominalTarget::Enum(id) => ("enum", self.enums[id].type_params.clone()),
            NominalTarget::Class(id) => ("class", self.classes[id].type_params.clone()),
            NominalTarget::Interface(id) => ("interface", self.interfaces[id].type_params.clone()),
            NominalTarget::Object(_) => ("object", Vec::new()),
        };
        let arity = params.len();
        if arity == 0 && !arguments.is_empty() {
            self.error(span, format!("{kind} `{display_name}` is not generic"));
            return None;
        }
        if arity != arguments.len() {
            self.error(
                span,
                if arguments.is_empty() {
                    format!("generic {kind} `{display_name}` requires {arity} type argument(s)")
                } else {
                    format!(
                        "{kind} `{display_name}` takes {arity} type argument(s), but {} were supplied",
                        arguments.len()
                    )
                },
            );
            return None;
        }
        let mut resolved = Vec::with_capacity(arguments.len());
        for argument in arguments {
            resolved.push(self.resolve_type_ref(argument)?);
        }
        if !self.check_type_argument_kinds(
            &params,
            &resolved,
            span,
            &format!("{kind} `{display_name}`"),
        ) {
            return None;
        }
        Some(match target {
            NominalTarget::Struct(id) => self.struct_application(id, resolved),
            NominalTarget::Enum(id) => self.enum_application(id, resolved),
            NominalTarget::Class(id) => self.class_application(id, resolved),
            NominalTarget::Interface(id) => self.intern_interface_application(id, resolved),
            NominalTarget::Object(id) => {
                self.object_types[self.objects[id].object_type].canonical_type
            }
        })
    }

    fn resolve_qualified_nominal(
        &mut self,
        path: &[ast::Ident],
        arguments: &[ast::TypeRef],
        span: ast::Span,
    ) -> Option<TypeId> {
        let first = path.first().expect("a qualified type path is non-empty");
        let mut target = if let Some(target) = self.lexical_nested_nominal_target(&first.text) {
            target
        } else if self.source_type_alias_named(&first.text).is_some() {
            let alias = self.resolve_type_alias_reference(first, false)?;
            let Some(target) = self.nominal_target_for_type(alias) else {
                self.error(
                    first.span,
                    format!("typealias `{}` does not name a type qualifier", first.text),
                );
                return None;
            };
            target
        } else {
            let Some(target) = self.top_level_nominal_target(&first.text) else {
                self.error(first.span, format!("unknown type `{}`", first.text));
                return None;
            };
            target
        };
        for segment in &path[1..] {
            let owner = target.owner();
            let Some(nested) = self
                .nested_nominals_by_owner
                .get(&(owner, segment.text.clone()))
                .copied()
            else {
                let owner_name = path
                    .iter()
                    .take_while(|candidate| candidate.span.end <= segment.span.start)
                    .map(|candidate| candidate.text.as_str())
                    .collect::<Vec<_>>()
                    .join(".");
                self.error(
                    segment.span,
                    format!("type `{owner_name}` has no nested type `{}`", segment.text),
                );
                return None;
            };
            target = nested;
        }
        let display_name = path
            .iter()
            .map(|segment| segment.text.as_str())
            .collect::<Vec<_>>()
            .join(".");
        self.resolve_nested_nominal_application(target, arguments, span, &display_name)
    }

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
                if self.source_type_alias_named(&name.text).is_some() {
                    return self.resolve_type_alias_reference(name, true);
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
                if self.objects_by_name.contains_key(&name.text) {
                    self.error(name.span, format!("object `{}` is not generic", name.text));
                    return None;
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
                if self.source_type_alias_named(&name.text).is_some() {
                    return self.resolve_type_alias_reference(name, false);
                }
                match name.text.as_str() {
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
                        if let Some(&object) = self.objects_by_name.get(&name.text) {
                            return Some(
                                self.object_types[self.objects[object].object_type].canonical_type,
                            );
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
                match self.option_enumeration() {
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
