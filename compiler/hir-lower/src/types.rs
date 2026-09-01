//! Type resolution, interning, substitution and comparison on the
//! partially built module.
//!
//! `scoop_hir::types_equal` / `scoop_hir::type_name` operate on a
//! finished `hir::Module`; lowering needs the same operations on the
//! arenas while the module is still being built, so they are
//! reimplemented here over `&Arena<Type>` directly.
//!
//! M3 (milestone3 DESIGN.md 2.2): a generic function's type parameters
//! resolve to `Type::Param(index)` while its signature and body are
//! being lowered.
//!
//! M4 (milestone4 DESIGN.md 3.2): `T?` annotations resolve to the core
//! library's `Option<T>` enum (`Type::Enum(option_enum, [T])`, interned
//! on demand), and enum variant field types resolve in the enum's own
//! type-parameter scope (a generic enum's fields mention
//! `Type::Param(index)` into `EnumDecl::type_params`).
//!
//! M5 (milestone5 DESIGN.md 2.2): `Array<T>` / `MutableArray<T>`
//! annotations resolve to the compiler-built-in array types (spec 10.1;
//! class declarations arrive with M7, DESIGN.md 5.1), interned like
//! every other type.
//!
//! M6 (milestone6 DESIGN.md 2.2): `Any` (a compiler built-in, DESIGN.md
//! 5.5), class and interface type names, and the subtyping relation
//! (`is_subtype`) that replaces plain equality checks at assignment,
//! argument, return, annotation and array-element positions: equal
//! types, a class and its base classes, a class and the interfaces it
//! implements, and everything below `Any`. `adapt_to` performs the
//! accompanying conversion: a value type crossing into `Any` / an
//! interface is boxed (`ExprKind::Box`, spec 4.4.4); a class reference
//! crossing to a base class / interface / `Any` is a zero-cost retype.
//!
//! M9 (milestone9 DESIGN.md section 1): the `UInt` well-known type
//! (spec 11.2; distinct from `Int` — `types_equal` stays strict and
//! there is no implicit conversion) and generic struct applications
//! (`PinnedPtr<T>` / `GcHandle<T>` from the core GC facilities).
//! Struct applications carry their arguments directly in
//! `Type::Struct`, matching generic enum representation.
//!
//! M11 (milestone11 DESIGN.md section 2.1): ordinary and suspend function
//! types are canonical structural signatures referenced through a distinct
//! `FunctionTypeId`. They remain reference types through MIR/LIR.

use la_arena::Arena;
use scoop_ast as ast;
use scoop_hir as hir;
use scoop_hir::{ClassDecl, EnumDecl, InterfaceDecl, StructDecl, StructId, Type, TypeId};

use crate::Lowerer;

impl Lowerer {
    /// Resolve the complete constraint set for one declaration after all
    /// nominal names and arities are known. `params` may begin with an owner
    /// prefix (generic methods/local functions); only names declared in
    /// `declarations` may be constrained by this declaration's `where` clause.
    pub(crate) fn resolve_type_parameter_constraints(
        &mut self,
        mut params: Vec<hir::TypeParamDecl>,
        owner_count: usize,
        declarations: &[ast::TypeParamDecl],
        where_clause: Option<&ast::WhereClause>,
        target: &str,
    ) -> Vec<hir::TypeParamDecl> {
        if target != "interface" {
            for declaration in declarations {
                if declaration.variance != ast::Variance::Invariant {
                    self.error(
                        declaration.span,
                        format!(
                            "type parameter `{}` of {target} must be invariant",
                            declaration.name.text
                        ),
                    );
                }
            }
        }

        self.type_params_in_scope = params.clone();
        let own_indices: std::collections::HashMap<_, _> = params
            .iter()
            .enumerate()
            .skip(owner_count)
            .map(|(index, parameter)| (parameter.name.clone(), index))
            .collect();

        let mut seen_inline = std::collections::HashSet::new();
        for declaration in declarations {
            if !seen_inline.insert(declaration.name.text.as_str()) {
                continue;
            }
            let Some(bound) = declaration.inline_bound.as_ref() else {
                continue;
            };
            let Some(&index) = own_indices.get(&declaration.name.text) else {
                continue;
            };
            self.apply_type_parameter_constraint(
                &mut params,
                index,
                bound,
                declaration.span,
                target,
            );
        }

        if let Some(clause) = where_clause {
            for constraint in &clause.constraints {
                let Some(&index) = own_indices.get(&constraint.parameter.text) else {
                    self.error(
                        constraint.parameter.span,
                        format!(
                            "unknown type parameter `{}` in where clause of {target}",
                            constraint.parameter.text
                        ),
                    );
                    continue;
                };
                self.apply_type_parameter_constraint(
                    &mut params,
                    index,
                    &constraint.bound,
                    constraint.span,
                    target,
                );
            }
        }

        self.type_params_in_scope.clear();
        params
    }

    fn apply_type_parameter_constraint(
        &mut self,
        params: &mut [hir::TypeParamDecl],
        index: usize,
        bound: &ast::TypeBound,
        span: ast::Span,
        target: &str,
    ) {
        match bound {
            ast::TypeBound::Kind(kind) => {
                let next = match kind {
                    ast::TypeParamKindBound::Value => hir::TypeParamBounds::Value { span },
                    ast::TypeParamKindBound::Ref => hir::TypeParamBounds::Ref { span },
                };
                match params[index].bounds {
                    hir::TypeParamBounds::Unconstrained => params[index].bounds = next,
                    hir::TypeParamBounds::Interfaces(_) => self.error(
                        span,
                        format!(
                            "type parameter `{}` of {target} cannot combine a kind bound with interface upper bounds",
                            params[index].name
                        ),
                    ),
                    hir::TypeParamBounds::Value { .. }
                    | hir::TypeParamBounds::Ref { .. } => self.error(
                        span,
                        format!(
                            "duplicate kind bound for type parameter `{}` of {target}",
                            params[index].name
                        ),
                    ),
                }
            }
            ast::TypeBound::Upper(reference) => {
                // Keep the complete parameter namespace visible while the
                // upper application (including F-bound arguments) resolves.
                self.type_params_in_scope = params.to_vec();
                let Some(ty) = self.resolve_type_ref(reference) else {
                    return;
                };
                let Type::Interface(application) = self.types[ty] else {
                    let found = self.type_name(ty);
                    self.error(
                        reference.span,
                        format!(
                            "upper bound of type parameter `{}` must be an interface, found `{found}`",
                            params[index].name
                        ),
                    );
                    return;
                };
                match &mut params[index].bounds {
                    hir::TypeParamBounds::Unconstrained => {
                        params[index].bounds =
                            hir::TypeParamBounds::Interfaces(vec![hir::InterfaceBound {
                                application,
                                span,
                            }]);
                    }
                    hir::TypeParamBounds::Interfaces(bounds) => {
                        if bounds
                            .iter()
                            .any(|existing| existing.application == application)
                        {
                            self.error(
                                span,
                                format!(
                                    "duplicate interface upper bound `{}` for type parameter `{}` of {target}",
                                    self.type_name(ty), params[index].name
                                ),
                            );
                        } else {
                            bounds.push(hir::InterfaceBound { application, span });
                        }
                    }
                    hir::TypeParamBounds::Value { .. }
                    | hir::TypeParamBounds::Ref { .. } => self.error(
                        span,
                        format!(
                            "type parameter `{}` of {target} cannot combine interface upper bounds with a kind bound",
                            params[index].name
                        ),
                    ),
                }
            }
        }
        self.type_params_in_scope = params.to_vec();
    }

    /// Validate dependencies between nominal bound declarations only after all
    /// of them have complete constraint sets. This is order-independent and
    /// permits legal F-bound cycles.
    pub(crate) fn validate_nominal_type_parameter_constraints(&mut self) {
        let declarations: Vec<_> = self
            .structs
            .iter()
            .map(|(id, declaration)| {
                (
                    self.struct_files[&id],
                    "struct",
                    declaration.name.clone(),
                    declaration.type_params.clone(),
                )
            })
            .chain(self.enums.iter().map(|(id, declaration)| {
                (
                    self.enum_files[&id],
                    "enum",
                    declaration.name.clone(),
                    declaration.type_params.clone(),
                )
            }))
            .chain(self.classes.iter().map(|(id, declaration)| {
                (
                    self.class_files[&id],
                    "class",
                    declaration.name.clone(),
                    declaration.type_params.clone(),
                )
            }))
            .chain(self.interfaces.iter().map(|(id, declaration)| {
                (
                    self.interface_files[&id],
                    "interface",
                    declaration.name.clone(),
                    declaration.type_params.clone(),
                )
            }))
            .collect();
        for (file, kind, name, params) in declarations {
            self.current_file = file;
            self.type_params_in_scope = params.clone();
            for parameter in &params {
                for bound in parameter.interface_bounds() {
                    let application = self.interface_applications[bound.application].clone();
                    let target_params = self.interfaces[application.template].type_params.clone();
                    self.check_type_argument_kinds(
                        &target_params,
                        &application.arguments,
                        bound.span,
                        &format!("upper bound of {kind} `{name}`"),
                    );
                }
            }
        }
        self.type_params_in_scope.clear();
    }

    /// Resolve a type annotation (`Int`, `Point`, `(Int, String)`,
    /// `T?`, ...). Function (or enum) type parameters shadow well-known
    /// and declared types: they only scope over one function's
    /// signature/body or one enum's variants (`type_params_in_scope`
    /// is empty everywhere else).
    pub(crate) fn resolve_type_ref(&mut self, ty_ref: &ast::TypeRef) -> Option<TypeId> {
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
                // The array built-ins resolve before user-declared
                // types (milestone5 DESIGN.md 2.2).
                if name.text == "Array" || name.text == "MutableArray" {
                    if args.len() != 1 {
                        self.error(
                            name.span,
                            format!(
                                "`{}` takes exactly 1 type argument, but {} were supplied",
                                name.text,
                                args.len()
                            ),
                        );
                        return None;
                    }
                    let element = self.resolve_type_ref(&args[0])?;
                    let ty = if name.text == "Array" {
                        Type::Array(element)
                    } else {
                        Type::MutableArray(element)
                    };
                    return Some(self.intern_type(ty));
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
                        // The array built-ins require their type
                        // argument (`Array<T>` goes through
                        // TypeRefKind::Generic).
                        if name.text == "Array" || name.text == "MutableArray" {
                            self.error(
                                name.span,
                                format!("`{}` requires exactly 1 type argument", name.text),
                            );
                            return None;
                        }
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

    /// Intern a type, so structurally equal types share a single
    /// `TypeId` (this makes instantiation dedup a plain id comparison).
    /// On a dedup hit the fresh entry simply stays unreferenced
    /// (nothing iterates the arena semantically).
    pub(crate) fn intern_type(&mut self, candidate: Type) -> TypeId {
        let id = self.types.alloc(candidate);
        for (other, _) in self.types.iter() {
            if other != id && type_value_equal(&self.types, other, id) {
                return other;
            }
        }
        id
    }

    /// Intern one complete function signature and return its ordinary HIR
    /// type. The signature arena and the surrounding `Type` arena are both
    /// canonical, so equal source spellings share both ids.
    pub(crate) fn intern_function_type(
        &mut self,
        is_suspend: bool,
        parameter_types: Vec<TypeId>,
        return_type: TypeId,
    ) -> TypeId {
        let existing = self.function_types.iter().find_map(|(id, candidate)| {
            (candidate.is_suspend == is_suspend
                && candidate.parameter_types == parameter_types
                && candidate.return_type == return_type)
                .then_some(id)
        });
        let function = match existing {
            Some(id) => id,
            None => self.function_types.alloc(hir::FunctionType {
                is_suspend,
                parameter_types,
                return_type,
            }),
        };
        self.intern_type(Type::Function(function))
    }

    fn instantiate_function_type(
        &mut self,
        id: hir::FunctionTypeId,
        mut substitute: impl FnMut(&mut Self, TypeId) -> Option<TypeId>,
    ) -> Option<TypeId> {
        let function = self.function_types[id].clone();
        let mut parameter_types = Vec::with_capacity(function.parameter_types.len());
        for parameter in function.parameter_types {
            parameter_types.push(substitute(self, parameter)?);
        }
        let return_type = substitute(self, function.return_type)?;
        Some(self.intern_function_type(function.is_suspend, parameter_types, return_type))
    }

    pub(crate) fn struct_application_id(
        &mut self,
        template: StructId,
        arguments: Vec<TypeId>,
    ) -> hir::StructApplicationId {
        let key = (template, arguments.clone());
        if let Some(&application) = self.struct_application_by_key.get(&key) {
            return application;
        }
        let canonical_type = hir::TypeId::from_raw((self.types.len() as u32).into());
        let application = self.struct_applications.alloc(hir::StructApplication {
            template,
            arguments,
            canonical_type,
        });
        let allocated_type = self.types.alloc(Type::Struct(application));
        assert_eq!(allocated_type, canonical_type);
        self.struct_application_by_key.insert(key, application);
        application
    }

    /// Intern a complete generic struct application (`PinnedPtr<String>`,
    /// M12). The type contains only the application identity; declaration and
    /// arguments live together in the application arena.
    pub(crate) fn struct_application(&mut self, template: StructId, args: Vec<TypeId>) -> TypeId {
        let application = self.struct_application_id(template, args);
        self.struct_applications[application].canonical_type
    }

    pub(crate) fn enum_application_id(
        &mut self,
        template: hir::EnumId,
        arguments: Vec<TypeId>,
    ) -> hir::EnumApplicationId {
        let key = (template, arguments.clone());
        if let Some(&application) = self.enum_application_by_key.get(&key) {
            return application;
        }
        let canonical_type = hir::TypeId::from_raw((self.types.len() as u32).into());
        let application = self.enum_applications.alloc(hir::EnumApplication {
            template,
            arguments,
            canonical_type,
        });
        let allocated_type = self.types.alloc(Type::Enum(application));
        assert_eq!(allocated_type, canonical_type);
        self.enum_application_by_key.insert(key, application);
        application
    }

    pub(crate) fn enum_application(
        &mut self,
        template: hir::EnumId,
        arguments: Vec<TypeId>,
    ) -> TypeId {
        let application = self.enum_application_id(template, arguments);
        self.enum_applications[application].canonical_type
    }

    pub(crate) fn class_application_id(
        &mut self,
        template: hir::ClassId,
        arguments: Vec<TypeId>,
    ) -> hir::ClassApplicationId {
        let key = (template, arguments.clone());
        if let Some(&application) = self.class_application_by_key.get(&key) {
            return application;
        }
        let canonical_type = hir::TypeId::from_raw((self.types.len() as u32).into());
        let application = self.class_applications.alloc(hir::ClassApplication {
            template,
            arguments,
            canonical_type,
        });
        let allocated_type = self.types.alloc(Type::Class(application));
        assert_eq!(allocated_type, canonical_type);
        self.class_application_by_key.insert(key, application);
        application
    }

    pub(crate) fn class_application(
        &mut self,
        template: hir::ClassId,
        arguments: Vec<TypeId>,
    ) -> TypeId {
        let application = self.class_application_id(template, arguments);
        self.class_applications[application].canonical_type
    }

    pub(crate) fn interface_application_id(
        &mut self,
        template: hir::InterfaceId,
        arguments: Vec<TypeId>,
    ) -> hir::InterfaceApplicationId {
        let key = (template, arguments.clone());
        if let Some(&application) = self.interface_application_by_key.get(&key) {
            return application;
        }
        let canonical_type = hir::TypeId::from_raw((self.types.len() as u32).into());
        let application = self
            .interface_applications
            .alloc(hir::InterfaceApplication {
                template,
                arguments,
                canonical_type,
            });
        let allocated_type = self.types.alloc(Type::Interface(application));
        assert_eq!(allocated_type, canonical_type);
        self.interface_application_by_key.insert(key, application);
        application
    }

    pub(crate) fn intern_interface_application(
        &mut self,
        template: hir::InterfaceId,
        arguments: Vec<TypeId>,
    ) -> TypeId {
        let application = self.interface_application_id(template, arguments);
        self.interface_applications[application].canonical_type
    }

    pub(crate) fn type_contains_param(&self, ty: TypeId) -> bool {
        match &self.types[ty] {
            Type::Param(_) => true,
            Type::Array(element) | Type::MutableArray(element) | Type::Ptr(element) => {
                self.type_contains_param(*element)
            }
            Type::Struct(application) => self.struct_applications[*application]
                .arguments
                .iter()
                .any(|ty| self.type_contains_param(*ty)),
            Type::Class(application) => self.class_applications[*application]
                .arguments
                .iter()
                .any(|ty| self.type_contains_param(*ty)),
            Type::Interface(application) => self.interface_applications[*application]
                .arguments
                .iter()
                .any(|ty| self.type_contains_param(*ty)),
            Type::Enum(application) => self.enum_applications[*application]
                .arguments
                .iter()
                .any(|ty| self.type_contains_param(*ty)),
            Type::Tuple(args) => args.iter().any(|ty| self.type_contains_param(*ty)),
            Type::Function(id) | Type::FunPtr(id) => self.function_type_contains_param(*id),
            _ => false,
        }
    }

    pub(crate) fn function_type_contains_param(&self, id: hir::FunctionTypeId) -> bool {
        let function = &self.function_types[id];
        function
            .parameter_types
            .iter()
            .any(|ty| self.type_contains_param(*ty))
            || self.type_contains_param(function.return_type)
    }

    /// Substitute bound type arguments for `Type::Param`, recursively.
    /// Used both for generic function instantiation (parameters index
    /// `Function::type_params`) and for enum variant field types
    /// (parameters index `EnumDecl::type_params`). Callers guarantee
    /// every parameter is bound (unbound parameters are diagnosed at
    /// the use site first).
    pub(crate) fn instantiate_ty(&mut self, ty: TypeId, type_args: &[TypeId]) -> TypeId {
        match self.types[ty].clone() {
            Type::Param(index) => type_args[index.into_raw() as usize],
            Type::Struct(application) => {
                let application = self.struct_applications[application].clone();
                let id = application.template;
                let args = application.arguments;
                let mut substituted = Vec::with_capacity(args.len());
                for arg in args {
                    substituted.push(self.instantiate_ty(arg, type_args));
                }
                if Some(id) == self.ffi_fun_ptr {
                    let [function] = substituted.as_slice() else {
                        unreachable!("validated FunPtr has one type argument")
                    };
                    let Type::Function(function) = self.types[*function] else {
                        unreachable!("deferred FunPtr resolves to a function type")
                    };
                    return self.intern_type(Type::FunPtr(function));
                }
                self.struct_application(id, substituted)
            }
            Type::Class(application) => {
                let application = self.class_applications[application].clone();
                let substituted = application
                    .arguments
                    .into_iter()
                    .map(|arg| self.instantiate_ty(arg, type_args))
                    .collect();
                self.class_application(application.template, substituted)
            }
            Type::Interface(application) => {
                let application = self.interface_applications[application].clone();
                let mut substituted = Vec::with_capacity(application.arguments.len());
                for arg in application.arguments {
                    substituted.push(self.instantiate_ty(arg, type_args));
                }
                self.intern_interface_application(application.template, substituted)
            }
            Type::Array(element) => {
                let element = self.instantiate_ty(element, type_args);
                self.intern_type(Type::Array(element))
            }
            Type::MutableArray(element) => {
                let element = self.instantiate_ty(element, type_args);
                self.intern_type(Type::MutableArray(element))
            }
            Type::Enum(application) => {
                let application = self.enum_applications[application].clone();
                let mut substituted = Vec::with_capacity(application.arguments.len());
                for arg in application.arguments {
                    substituted.push(self.instantiate_ty(arg, type_args));
                }
                self.enum_application(application.template, substituted)
            }
            Type::Tuple(elements) => {
                let mut substituted = Vec::with_capacity(elements.len());
                for element in elements {
                    substituted.push(self.instantiate_ty(element, type_args));
                }
                self.intern_type(Type::Tuple(substituted))
            }
            Type::Function(id) => self
                .instantiate_function_type(id, |this, ty| Some(this.instantiate_ty(ty, type_args)))
                .expect("complete type argument substitution"),
            Type::Ptr(pointee) => {
                let pointee = self.instantiate_ty(pointee, type_args);
                self.intern_type(Type::Ptr(pointee))
            }
            Type::FunPtr(id) => {
                let function = self
                    .instantiate_function_type(id, |this, ty| {
                        Some(this.instantiate_ty(ty, type_args))
                    })
                    .expect("complete function pointer substitution");
                let Type::Function(id) = self.types[function] else {
                    unreachable!("function type instantiation stays a function type")
                };
                self.intern_type(Type::FunPtr(id))
            }
            _ => ty,
        }
    }

    /// Apply an exact source-produced type-parameter relation while comparing
    /// method signatures. Both owner substitutions and callable-parameter
    /// alpha-renaming are explicit bindings; this routine never derives a
    /// split point from an index.
    pub(crate) fn instantiate_method_ty(
        &mut self,
        ty: TypeId,
        bindings: &[(hir::TypeParamId, TypeId)],
    ) -> TypeId {
        match self.types[ty].clone() {
            Type::Param(parameter) => bindings
                .iter()
                .find_map(|(source, target)| (*source == parameter).then_some(*target))
                .expect("a complete method substitution binds every referenced parameter"),
            Type::Struct(application) => {
                let application = self.struct_applications[application].clone();
                let args = application
                    .arguments
                    .into_iter()
                    .map(|arg| self.instantiate_method_ty(arg, bindings))
                    .collect();
                self.struct_application(application.template, args)
            }
            Type::Class(application) => {
                let application = self.class_applications[application].clone();
                let args = application
                    .arguments
                    .into_iter()
                    .map(|arg| self.instantiate_method_ty(arg, bindings))
                    .collect();
                self.class_application(application.template, args)
            }
            Type::Interface(application) => {
                let application = self.interface_applications[application].clone();
                let args = application
                    .arguments
                    .into_iter()
                    .map(|arg| self.instantiate_method_ty(arg, bindings))
                    .collect();
                self.intern_interface_application(application.template, args)
            }
            Type::Array(element) => {
                let element = self.instantiate_method_ty(element, bindings);
                self.intern_type(Type::Array(element))
            }
            Type::MutableArray(element) => {
                let element = self.instantiate_method_ty(element, bindings);
                self.intern_type(Type::MutableArray(element))
            }
            Type::Enum(application) => {
                let application = self.enum_applications[application].clone();
                let args = application
                    .arguments
                    .into_iter()
                    .map(|arg| self.instantiate_method_ty(arg, bindings))
                    .collect();
                self.enum_application(application.template, args)
            }
            Type::Tuple(elements) => {
                let elements = elements
                    .into_iter()
                    .map(|element| self.instantiate_method_ty(element, bindings))
                    .collect();
                self.intern_type(Type::Tuple(elements))
            }
            Type::Function(id) => self
                .instantiate_function_type(id, |this, ty| {
                    Some(this.instantiate_method_ty(ty, bindings))
                })
                .expect("complete owner substitution"),
            Type::Ptr(pointee) => {
                let pointee = self.instantiate_method_ty(pointee, bindings);
                self.intern_type(Type::Ptr(pointee))
            }
            Type::FunPtr(id) => {
                let function = self
                    .instantiate_function_type(id, |this, ty| {
                        Some(this.instantiate_method_ty(ty, bindings))
                    })
                    .expect("complete function pointer owner substitution");
                let Type::Function(id) = self.types[function] else {
                    unreachable!("function type instantiation stays a function type")
                };
                self.intern_type(Type::FunPtr(id))
            }
            _ => ty,
        }
    }

    /// Best-effort substitution for expected-type hints: `None` when
    /// the type still mentions an unbound type parameter.
    pub(crate) fn try_substitute(
        &mut self,
        ty: TypeId,
        bindings: &[Option<TypeId>],
    ) -> Option<TypeId> {
        match self.types[ty].clone() {
            Type::Param(index) => bindings.get(index.into_raw() as usize).copied().flatten(),
            Type::Struct(application) => {
                let application = self.struct_applications[application].clone();
                let mut substituted = Vec::with_capacity(application.arguments.len());
                for arg in application.arguments {
                    substituted.push(self.try_substitute(arg, bindings)?);
                }
                Some(self.struct_application(application.template, substituted))
            }
            Type::Class(application) => {
                let application = self.class_applications[application].clone();
                let mut substituted = Vec::with_capacity(application.arguments.len());
                for arg in application.arguments {
                    substituted.push(self.try_substitute(arg, bindings)?);
                }
                Some(self.class_application(application.template, substituted))
            }
            Type::Interface(application) => {
                let application = self.interface_applications[application].clone();
                let mut substituted = Vec::with_capacity(application.arguments.len());
                for arg in application.arguments {
                    substituted.push(self.try_substitute(arg, bindings)?);
                }
                Some(self.intern_interface_application(application.template, substituted))
            }
            Type::Array(element) => {
                let element = self.try_substitute(element, bindings)?;
                Some(self.intern_type(Type::Array(element)))
            }
            Type::MutableArray(element) => {
                let element = self.try_substitute(element, bindings)?;
                Some(self.intern_type(Type::MutableArray(element)))
            }
            Type::Enum(application) => {
                let application = self.enum_applications[application].clone();
                let mut substituted = Vec::with_capacity(application.arguments.len());
                for arg in application.arguments {
                    substituted.push(self.try_substitute(arg, bindings)?);
                }
                Some(self.enum_application(application.template, substituted))
            }
            Type::Tuple(elements) => {
                let mut substituted = Vec::with_capacity(elements.len());
                for element in elements {
                    substituted.push(self.try_substitute(element, bindings)?);
                }
                Some(self.intern_type(Type::Tuple(substituted)))
            }
            Type::Function(id) => {
                self.instantiate_function_type(id, |this, ty| this.try_substitute(ty, bindings))
            }
            Type::Ptr(pointee) => {
                let pointee = self.try_substitute(pointee, bindings)?;
                Some(self.intern_type(Type::Ptr(pointee)))
            }
            Type::FunPtr(id) => {
                let function = self
                    .instantiate_function_type(id, |this, ty| this.try_substitute(ty, bindings))?;
                let Type::Function(id) = self.types[function] else {
                    unreachable!("function type substitution stays a function type")
                };
                Some(self.intern_type(Type::FunPtr(id)))
            }
            _ => Some(ty),
        }
    }

    /// Whether `ty` is `Array<T>` or `MutableArray<T>`; returns `T`.
    pub(crate) fn array_element_ty(&self, ty: TypeId) -> Option<TypeId> {
        match &self.types[ty] {
            Type::Array(element) | Type::MutableArray(element) => Some(*element),
            _ => None,
        }
    }

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
            &self.type_params_in_scope,
            ty,
        )
    }

    /// The subtyping relation (milestone6 DESIGN.md 2.2): equal types,
    /// a class below its base classes, a class below the interfaces it
    /// (or a base class) implements, a value type below the interfaces
    /// it implements (spec 4.4.3), and everything below `Any`. Value
    /// types count as subtypes of `Any` (they cross via boxing, spec
    /// 4.4.4); arrays are invariant in the element type (spec 10.4), so
    /// no array is a subtype of another array.
    pub(crate) fn is_subtype(&mut self, a: TypeId, b: TypeId) -> bool {
        if self.types_equal(a, b) {
            return true;
        }
        let a_ty = self.types[a].clone();
        let b_ty = self.types[b].clone();
        match (a_ty, b_ty) {
            (_, Type::Any) => true,
            (Type::Param(parameter), _) => self
                .type_params_in_scope
                .iter()
                .find(|candidate| candidate.id == parameter)
                .map(|parameter| parameter.interface_bounds().to_vec())
                .unwrap_or_default()
                .into_iter()
                .any(|bound| {
                    let bound = self.interface_applications[bound.application].canonical_type;
                    self.is_subtype(bound, b)
                }),
            (Type::Class(application), Type::Class(..)) => {
                let application = self.class_applications[application].clone();
                let Some((base, _)) = self.classes[application.template].base_class.clone() else {
                    return false;
                };
                let base = self.instantiate_ty(base, &application.arguments);
                self.is_subtype(base, b)
            }
            (Type::Interface(a), Type::Interface(b)) => {
                let a = self.interface_applications[a].clone();
                let b = self.interface_applications[b].clone();
                if a.template != b.template {
                    let parents = self.interfaces[a.template].parents.clone();
                    return parents.into_iter().any(|parent| {
                        let parent = self.interface_applications[parent].canonical_type;
                        let parent = self.instantiate_ty(parent, &a.arguments);
                        self.is_subtype(parent, b.canonical_type)
                    });
                }
                let variances: Vec<hir::Variance> = self.interfaces[a.template]
                    .type_params
                    .iter()
                    .map(|param| param.variance)
                    .collect();
                variances
                    .into_iter()
                    .zip(a.arguments)
                    .zip(b.arguments)
                    .all(|((variance, a), b)| match variance {
                        hir::Variance::Invariant => self.types_equal(a, b),
                        hir::Variance::Out => self.is_subtype(a, b),
                        hir::Variance::In => self.is_subtype(b, a),
                    })
            }
            (Type::Function(source), Type::Function(target)) => {
                let source = self.function_types[source].clone();
                let target = self.function_types[target].clone();
                source.is_suspend == target.is_suspend
                    && source.parameter_types.len() == target.parameter_types.len()
                    && target
                        .parameter_types
                        .into_iter()
                        .zip(source.parameter_types)
                        .all(|(target, source)| self.is_subtype(target, source))
                    && self.is_subtype(source.return_type, target.return_type)
            }
            (Type::Class(application), Type::Interface(..)) => self
                .class_interfaces_for_application(application)
                .into_iter()
                .any(|implemented| self.is_subtype(implemented, b)),
            (Type::Struct(application), Type::Interface(..)) => {
                let application = self.struct_applications[application].clone();
                let interfaces = self.structs[application.template].interfaces.clone();
                interfaces.into_iter().any(|implemented| {
                    let implemented = self.instantiate_ty(implemented, &application.arguments);
                    self.is_subtype(implemented, b)
                })
            }
            (Type::Enum(application), Type::Interface(..)) => {
                let application = self.enum_applications[application].clone();
                let interfaces = self.enums[application.template].interfaces.clone();
                interfaces.into_iter().any(|implemented| {
                    let implemented = self.instantiate_ty(implemented, &application.arguments);
                    self.is_subtype(implemented, b)
                })
            }
            _ => false,
        }
    }

    /// View a concrete argument type through one exact implemented
    /// interface. Generic-call inference uses this before binding the
    /// interface's type arguments, so `C : I<Int>` can constrain a
    /// parameter declared as `I<T>` without an explicit upcast first.
    pub(crate) fn implemented_interface_application(
        &mut self,
        ty: TypeId,
        target: hir::InterfaceId,
    ) -> Option<Vec<TypeId>> {
        let candidates = match self.types[ty].clone() {
            Type::Interface(application) => {
                let application = self.interface_applications[application].clone();
                return (application.template == target).then_some(application.arguments);
            }
            Type::Class(application) => self.class_interfaces_for_application(application),
            Type::Struct(application) => {
                let application = self.struct_applications[application].clone();
                self.structs[application.template]
                    .interfaces
                    .clone()
                    .into_iter()
                    .map(|implemented| self.instantiate_ty(implemented, &application.arguments))
                    .collect()
            }
            Type::Enum(application) => {
                let application = self.enum_applications[application].clone();
                self.enums[application.template]
                    .interfaces
                    .clone()
                    .into_iter()
                    .map(|implemented| self.instantiate_ty(implemented, &application.arguments))
                    .collect()
            }
            _ => Vec::new(),
        };
        candidates.into_iter().find_map(|candidate| {
            let Type::Interface(application) = self.types[candidate] else {
                return None;
            };
            let application = &self.interface_applications[application];
            (application.template == target).then(|| application.arguments.clone())
        })
    }

    /// Fully substitute every interface reached from one class application,
    /// including interfaces inherited through its concrete generic base
    /// application. Applications, rather than declaration ids, are the
    /// deduplication identity.
    pub(crate) fn class_interfaces_for_application(
        &mut self,
        application: hir::ClassApplicationId,
    ) -> Vec<TypeId> {
        let mut result = Vec::new();
        let mut pending = vec![application];
        let mut seen = Vec::<hir::ClassApplicationId>::new();
        while let Some(application) = pending.pop() {
            let application_value = self.class_applications[application].clone();
            for interface in self.classes[application_value.template].interfaces.clone() {
                let interface = self.instantiate_ty(interface, &application_value.arguments);
                self.append_interface_closure(interface, &mut result);
            }
            if let Some((base, _)) = self.classes[application_value.template].base_class.clone() {
                let base = self.instantiate_ty(base, &application_value.arguments);
                let Type::Class(base_application) = self.types[base] else {
                    unreachable!("class bases are resolved class applications")
                };
                if seen.contains(&base_application) {
                    continue;
                }
                seen.push(base_application);
                pending.push(base_application);
            }
        }
        result
    }

    pub(crate) fn append_interface_closure(&mut self, interface: TypeId, result: &mut Vec<TypeId>) {
        if result
            .iter()
            .any(|&other| self.types_equal(other, interface))
        {
            return;
        }
        result.push(interface);
        let Type::Interface(application) = self.types[interface] else {
            unreachable!("interface closure starts from an interface application")
        };
        let application = self.interface_applications[application].clone();
        for parent in self.interfaces[application.template].parents.clone() {
            let parent = self.interface_applications[parent].canonical_type;
            let parent = self.instantiate_ty(parent, &application.arguments);
            self.append_interface_closure(parent, result);
        }
    }

    /// Whether a value of static type `a` could ever hold a `b` at run
    /// time — the static premise of `is` / `as` / `as?` (a check
    /// between unrelated types is diagnosed as useless). Beyond the
    /// subtyping relation in either direction, `Any` and interfaces
    /// can hold anything below them, and an open/abstract class may
    /// gain an interface implementation in a subclass.
    pub(crate) fn could_hold(&mut self, a: TypeId, b: TypeId) -> bool {
        if self.is_subtype(a, b) || self.is_subtype(b, a) {
            return true;
        }
        match &self.types[a] {
            Type::Any | Type::Interface(..) => true,
            &Type::Class(application) => {
                let template = self.class_applications[application].template;
                self.classes[template].modifier != hir::ClassModifier::Final
                    && matches!(self.types[b], Type::Interface(..))
            }
            _ => false,
        }
    }

    /// Value types (spec 3): everything that is not a reference. They
    /// cross into reference types (`Any` / interfaces) only by boxing
    /// (spec 4.4.4).
    pub(crate) fn is_value_ty(&self, ty: TypeId) -> bool {
        match self.types[ty] {
            Type::Unit
            | Type::Int
            | Type::UInt
            | Type::Boolean
            | Type::Struct(..)
            | Type::Enum(..)
            | Type::Tuple(_)
            | Type::Ptr(_)
            | Type::FunPtr(_) => true,
            Type::Param(index) => self
                .type_params_in_scope
                .iter()
                .find(|parameter| parameter.id == index)
                .is_none_or(|param| param.kind() != hir::TypeParamKind::Ref),
            _ => false,
        }
    }

    /// Reference types (spec 3): classes, interfaces, `Any`, strings
    /// and the built-in array types. `===` / `!==` only apply to these
    /// (spec 4.4.2).
    pub(crate) fn is_ref_ty(&self, ty: TypeId) -> bool {
        !self.is_value_ty(ty)
    }

    pub(crate) fn check_type_argument_kinds(
        &mut self,
        params: &[hir::TypeParamDecl],
        args: &[TypeId],
        span: ast::Span,
        target: &str,
    ) -> bool {
        let mut valid = true;
        for (param, &arg) in params.iter().zip(args) {
            if !self.type_satisfies_kind(arg, param.kind()) {
                let required = match param.kind() {
                    hir::TypeParamKind::Any => continue,
                    hir::TypeParamKind::Value => "value",
                    hir::TypeParamKind::Ref => "ref",
                };
                let found = self.type_name(arg);
                self.error(
                    span,
                    format!(
                        "type argument `{found}` for `{}` of {target} must satisfy `{required}`",
                        param.name
                    ),
                );
                valid = false;
            }
        }
        for (param, &arg) in params.iter().zip(args) {
            for bound in param.interface_bounds() {
                let bound = self.interface_applications[bound.application].canonical_type;
                let required = self.instantiate_ty(bound, args);
                if self.is_subtype(arg, required) {
                    continue;
                }
                let found = self.type_name(arg);
                let required = self.type_name(required);
                self.error(
                    span,
                    format!(
                        "type argument `{found}` for `{}` of {target} must satisfy interface upper bound `{required}`",
                        param.name
                    ),
                );
                valid = false;
            }
        }
        valid
    }

    pub(crate) fn type_arguments_satisfy_kinds(
        &mut self,
        params: &[hir::TypeParamDecl],
        args: &[TypeId],
    ) -> bool {
        for (param, &arg) in params.iter().zip(args) {
            if !self.type_satisfies_kind(arg, param.kind()) {
                return false;
            }
            for bound in param.interface_bounds() {
                let bound = self.interface_applications[bound.application].canonical_type;
                let required = self.instantiate_ty(bound, args);
                if !self.is_subtype(arg, required) {
                    return false;
                }
            }
        }
        true
    }

    fn type_satisfies_kind(&self, ty: TypeId, required: hir::TypeParamKind) -> bool {
        if required == hir::TypeParamKind::Any {
            return true;
        }
        if let Type::Param(index) = self.types[ty] {
            let actual = self
                .type_params_in_scope
                .iter()
                .find(|parameter| parameter.id == index)
                .map(|param| param.kind())
                .unwrap_or(hir::TypeParamKind::Any);
            return actual == required;
        }
        match required {
            hir::TypeParamKind::Any => true,
            hir::TypeParamKind::Value => self.is_value_ty(ty),
            hir::TypeParamKind::Ref => self.is_ref_ty(ty),
        }
    }

    /// The least representable upper bound of a non-empty set of
    /// reference types (spec 10.3). The current type system has no
    /// intersection types: if several incomparable minimal common
    /// supertypes remain, `Any` is the only representable result.
    pub(crate) fn reference_lob(&mut self, element_types: &[TypeId]) -> TypeId {
        debug_assert!(!element_types.is_empty());
        debug_assert!(element_types.iter().all(|&ty| self.is_ref_ty(ty)));

        self.least_upper_bound(element_types)
    }

    /// The least representable upper bound used by structured expression
    /// branches. Unlike array inference this also accepts value types;
    /// `is_subtype` includes their boxing conversions to interfaces/`Any`.
    /// With no intersection types, incomparable minimal bounds fall back to
    /// `Any`.
    pub(crate) fn least_upper_bound(&mut self, types: &[TypeId]) -> TypeId {
        debug_assert!(!types.is_empty());

        let mut common_supertypes = Vec::new();
        let candidates: Vec<TypeId> = self.types.iter().map(|(id, _)| id).collect();
        for candidate in candidates {
            if !types
                .iter()
                .all(|&element| self.is_subtype(element, candidate))
                || common_supertypes
                    .iter()
                    .any(|&existing| self.types_equal(existing, candidate))
            {
                continue;
            }
            common_supertypes.push(candidate);
        }

        let minimal: Vec<TypeId> = common_supertypes
            .iter()
            .copied()
            .filter(|&candidate| {
                !common_supertypes.iter().copied().any(|other| {
                    !self.types_equal(other, candidate) && self.is_subtype(other, candidate)
                })
            })
            .collect();
        if minimal.len() == 1 {
            minimal[0]
        } else {
            self.any
        }
    }

    /// Adapt an expression to a target type it is a subtype of (callers
    /// check `is_subtype` first and diagnose otherwise): a value type
    /// crossing into a reference target is boxed (`ExprKind::Box`,
    /// target in `Expr::ty`); a reference crossing to a supertype is a
    /// zero-cost retype. Equal types pass through unchanged.
    pub(crate) fn adapt_to(&mut self, expr: hir::Expr, target: TypeId) -> hir::Expr {
        if self.types_equal(expr.ty, target) {
            return expr;
        }
        let span = expr.span;
        if let (Type::Function(source), Type::Function(target_type)) =
            (self.types[expr.ty].clone(), self.types[target].clone())
        {
            let key = (source, target_type);
            let coercion = self
                .function_coercion_by_types
                .get(&key)
                .copied()
                .unwrap_or_else(|| {
                    let id = self.function_coercions.alloc(hir::FunctionCoercion {
                        source,
                        target: target_type,
                    });
                    self.function_coercion_by_types.insert(key, id);
                    id
                });
            return hir::Expr {
                kind: hir::ExprKind::FunctionCoercion {
                    source: Box::new(expr),
                    coercion,
                    target_type,
                },
                ty: target,
                span,
            };
        }
        if self.is_value_ty(expr.ty) {
            hir::Expr {
                kind: hir::ExprKind::Box(Box::new(expr)),
                ty: target,
                span,
            }
        } else {
            hir::Expr {
                kind: expr.kind,
                ty: target,
                span,
            }
        }
    }
}

fn type_value_equal(types: &Arena<Type>, a: TypeId, b: TypeId) -> bool {
    match (&types[a], &types[b]) {
        (Type::Unit, Type::Unit)
        | (Type::Int, Type::Int)
        | (Type::UInt, Type::UInt)
        | (Type::Boolean, Type::Boolean)
        | (Type::String, Type::String)
        | (Type::Any, Type::Any) => true,
        (Type::Struct(x), Type::Struct(y)) => x == y,
        (Type::Class(x), Type::Class(y)) => x == y,
        (Type::Interface(x), Type::Interface(y)) => x == y,
        (Type::Array(x), Type::Array(y)) | (Type::MutableArray(x), Type::MutableArray(y)) => {
            type_value_equal(types, *x, *y)
        }
        (Type::Param(x), Type::Param(y)) => x == y,
        (Type::Enum(x), Type::Enum(y)) => x == y,
        (Type::Tuple(xs), Type::Tuple(ys)) => {
            xs.len() == ys.len()
                && xs
                    .iter()
                    .zip(ys.iter())
                    .all(|(&x, &y)| type_value_equal(types, x, y))
        }
        (Type::Function(x), Type::Function(y)) => x == y,
        (Type::Ptr(x), Type::Ptr(y)) => type_value_equal(types, *x, *y),
        (Type::FunPtr(x), Type::FunPtr(y)) => x == y,
        _ => false,
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
        Type::Array(element) => {
            let inner = type_name(
                types,
                function_types,
                structs,
                enums,
                classes,
                interfaces,
                applications,
                type_params,
                *element,
            );
            format!("Array<{inner}>")
        }
        Type::MutableArray(element) => {
            let inner = type_name(
                types,
                function_types,
                structs,
                enums,
                classes,
                interfaces,
                applications,
                type_params,
                *element,
            );
            format!("MutableArray<{inner}>")
        }
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
