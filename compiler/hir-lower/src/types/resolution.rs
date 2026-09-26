use super::*;
use crate::{NominalTarget, Owner};

impl Lowerer {
    pub(crate) fn top_level_type_target_is_accessible(
        &self,
        target: crate::namespace::TopLevelTypeTarget,
    ) -> bool {
        let domain = match target {
            crate::namespace::TopLevelTypeTarget::Alias(alias) => {
                return self.source_type_alias_is_accessible(alias);
            }
            crate::namespace::TopLevelTypeTarget::Nominal(NominalTarget::Struct(id)) => {
                &self.structs[id].access.lookup.0
            }
            crate::namespace::TopLevelTypeTarget::Nominal(NominalTarget::Enum(id)) => {
                &self.enums[id].access.lookup.0
            }
            crate::namespace::TopLevelTypeTarget::Nominal(NominalTarget::Class(id)) => {
                &self.classes[id].access.lookup.0
            }
            crate::namespace::TopLevelTypeTarget::Nominal(NominalTarget::Interface(id)) => {
                &self.interfaces[id].access.lookup.0
            }
            crate::namespace::TopLevelTypeTarget::Nominal(NominalTarget::Object(id)) => {
                &self.objects[id].access.lookup.0
            }
        };
        self.access_domain_allows(domain)
    }

    pub(crate) fn top_level_type_target(
        &self,
        name: &str,
    ) -> Option<crate::namespace::TopLevelTypeTarget> {
        match self.lookup_type(name) {
            crate::imports::lookup::LookupResult::Unique(candidate) => candidate.target.current(),
            crate::imports::lookup::LookupResult::Missing
            | crate::imports::lookup::LookupResult::Ambiguous { .. }
            | crate::imports::lookup::LookupResult::Inaccessible(_) => None,
        }
    }

    pub(crate) fn top_level_type_target_for_reference(
        &self,
        name: &str,
    ) -> Option<crate::namespace::TopLevelTypeTarget> {
        match self.lookup_type(name) {
            crate::imports::lookup::LookupResult::Unique(candidate) => candidate.target.current(),
            crate::imports::lookup::LookupResult::Inaccessible(candidates)
                if candidates.len() == 1 =>
            {
                candidates.first().target.current()
            }
            crate::imports::lookup::LookupResult::Missing
            | crate::imports::lookup::LookupResult::Ambiguous { .. }
            | crate::imports::lookup::LookupResult::Inaccessible(_) => None,
        }
    }

    pub(crate) fn top_level_nominal_target(&self, name: &str) -> Option<NominalTarget> {
        match self.top_level_type_target_for_reference(name)? {
            crate::namespace::TopLevelTypeTarget::Nominal(target) => Some(target),
            crate::namespace::TopLevelTypeTarget::Alias(_) => None,
        }
    }

    pub(crate) fn core_nominal_target(&self, name: &str) -> Option<NominalTarget> {
        let mut declarations = self
            .intrinsic_sources
            .iter()
            .enumerate()
            .filter(|(_, source)| source.kind == crate::SourceKind::Core)
            .flat_map(|(file, _)| self.top_level_namespaces.declared_types_in_file(file, name));
        let target = declarations.next()?;
        if declarations.next().is_some() {
            return None;
        }
        match target {
            crate::namespace::TopLevelTypeTarget::Nominal(target) => Some(target),
            crate::namespace::TopLevelTypeTarget::Alias(_) => None,
        }
    }

    pub(crate) fn top_level_struct_named(&self, name: &str) -> Option<(StructId, TypeId)> {
        let NominalTarget::Struct(id) = self.top_level_nominal_target(name)? else {
            return None;
        };
        let ty = self.struct_applications[self.structs[id].self_application].canonical_type;
        Some((id, ty))
    }

    pub(crate) fn top_level_enum_named(&self, name: &str) -> Option<hir::EnumId> {
        let NominalTarget::Enum(id) = self.top_level_nominal_target(name)? else {
            return None;
        };
        Some(id)
    }

    pub(crate) fn top_level_class_named(&self, name: &str) -> Option<(hir::ClassId, TypeId)> {
        let NominalTarget::Class(id) = self.top_level_nominal_target(name)? else {
            return None;
        };
        let ty = self.class_applications[self.classes[id].self_application].canonical_type;
        Some((id, ty))
    }

    pub(crate) fn top_level_interface_named(
        &self,
        name: &str,
    ) -> Option<(hir::InterfaceId, TypeId)> {
        let NominalTarget::Interface(id) = self.top_level_nominal_target(name)? else {
            return None;
        };
        let ty = self.interface_applications[self.interfaces[id].self_application].canonical_type;
        Some((id, ty))
    }

    pub(crate) fn top_level_object_named(&self, name: &str) -> Option<hir::ObjectId> {
        let NominalTarget::Object(id) = self.top_level_nominal_target(name)? else {
            return None;
        };
        Some(id)
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
        let (package, package_length) = self.top_level_namespaces.longest_package_prefix(path);
        let (mut target, owner_start) = if package_length != 0 {
            let package_name = self
                .top_level_namespaces
                .package_segments(package)
                .join(".");
            let Some(binding_name) = path.get(package_length) else {
                self.error(
                    path.last().expect("the path is non-empty").span,
                    format!("`{package_name}` names a package, not a type"),
                );
                return None;
            };
            let result = self.lookup_package_type(package, &binding_name.text);
            let Some(binding) = self.commit_type_lookup(binding_name, result).ok()? else {
                self.error(
                    binding_name.span,
                    format!(
                        "package `{package_name}` has no accessible type `{}`",
                        binding_name.text
                    ),
                );
                return None;
            };
            let target = match binding {
                crate::imports::lookup::TypeLookupTarget::Current(
                    crate::namespace::TopLevelTypeTarget::Nominal(target),
                ) => target,
                crate::imports::lookup::TypeLookupTarget::Current(
                    crate::namespace::TopLevelTypeTarget::Alias(alias),
                ) => {
                    let final_segment = package_length + 1 == path.len();
                    let ty = self.resolve_type_alias_id_reference(
                        alias,
                        binding_name,
                        final_segment && !arguments.is_empty(),
                    )?;
                    if final_segment {
                        return Some(ty);
                    }
                    let Some(target) = self.nominal_target_for_type(ty) else {
                        self.error(
                            binding_name.span,
                            format!(
                                "typealias `{}` does not name a type qualifier",
                                binding_name.text
                            ),
                        );
                        return None;
                    };
                    target
                }
                crate::imports::lookup::TypeLookupTarget::Dependency(binding) => {
                    let final_segment = package_length + 1 == path.len();
                    let ty = self.resolve_imported_dependency_type_target(
                        &binding,
                        binding_name,
                        final_segment && !arguments.is_empty(),
                    )?;
                    if final_segment {
                        return Some(ty);
                    }
                    let Some(target) = self.nominal_target_for_type(ty) else {
                        self.error(
                            binding_name.span,
                            format!(
                                "typealias `{}` does not name a type qualifier",
                                binding_name.text
                            ),
                        );
                        return None;
                    };
                    target
                }
            };
            (target, package_length + 1)
        } else {
            let target = if let Some(target) = self.lexical_nested_nominal_target(&first.text) {
                target
            } else {
                match self.resolve_type_lookup(first).ok()? {
                    Some(crate::imports::lookup::TypeLookupTarget::Current(
                        crate::namespace::TopLevelTypeTarget::Nominal(target),
                    )) => target,
                    Some(crate::imports::lookup::TypeLookupTarget::Current(
                        crate::namespace::TopLevelTypeTarget::Alias(alias),
                    )) => {
                        let ty = self.resolve_type_alias_id_reference(alias, first, false)?;
                        let Some(target) = self.nominal_target_for_type(ty) else {
                            self.error(
                                first.span,
                                format!(
                                    "typealias `{}` does not name a type qualifier",
                                    first.text
                                ),
                            );
                            return None;
                        };
                        target
                    }
                    Some(crate::imports::lookup::TypeLookupTarget::Dependency(binding)) => {
                        let ty =
                            self.resolve_imported_dependency_type_target(&binding, first, false)?;
                        let Some(target) = self.nominal_target_for_type(ty) else {
                            self.error(
                                first.span,
                                format!(
                                    "typealias `{}` does not name a type qualifier",
                                    first.text
                                ),
                            );
                            return None;
                        };
                        target
                    }
                    None => {
                        self.error(first.span, format!("unknown type `{}`", first.text));
                        return None;
                    }
                }
            };
            (target, 1)
        };
        for segment in &path[owner_start..] {
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
            ast::TypeRefKind::Unit => {
                if let Some(declaration) = self.imported_nominal_declaration(self.unit)
                    && !self.retain_builtin_alias_target_binding("Unit", declaration, ty_ref.span)
                {
                    return None;
                }
                Some(self.unit)
            }
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
                        crate::namespace::TopLevelTypeTarget::Alias(alias),
                    )) => return self.resolve_type_alias_id_reference(alias, name, true),
                    Some(crate::imports::lookup::TypeLookupTarget::Dependency(binding)) => {
                        return self.resolve_imported_dependency_type_target(&binding, name, true);
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
                    return Some(self.intern_interface_application(interface_id, resolved));
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
                let resolved = match name.text.as_str() {
                    _ if matches!(self.core, crate::CoreLoweringAuthority::Imported(_))
                        && !matches!(name.text.as_str(), "Unit" | "Any") =>
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
                    // `Any` is a compiler built-in (milestone6 DESIGN.md
                    // 5.5); the core library shape arrives with M7/core.
                    "Any" => Some(self.any),
                    _ => {
                        self.error(name.span, format!("unknown type `{}`", name.text));
                        None
                    }
                };
                if let Some(ty) = resolved
                    && let Some(declaration) = self.imported_nominal_declaration(ty)
                    && !self.retain_builtin_alias_target_binding(&name.text, declaration, name.span)
                {
                    return None;
                }
                resolved
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
