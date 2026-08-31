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
//! (`PinHandle<T>` / `GcHandle<T>` from the core GC facilities).
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
                    return Some(self.intern_type(Type::Interface(interface_id, resolved)));
                }
                let Some(&enum_id) = self.enums_by_name.get(&name.text) else {
                    let what = if self.classes_by_name.contains_key(&name.text) {
                        format!("class `{}` is not generic", name.text)
                    } else if name.text == "Any" {
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
                Some(self.intern_type(Type::Enum(enum_id, resolved)))
            }
            ast::TypeRefKind::Named(name) => {
                if let Some(index) = self
                    .type_params_in_scope
                    .iter()
                    .position(|param| param.name == name.text)
                {
                    return Some(
                        self.intern_type(Type::Param(hir::TypeParamId::from_raw(index as u32))),
                    );
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
                            // arguments (`PinHandle<T>` goes through
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
                        if let Some(&(_, ty)) = self.classes_by_name.get(&name.text) {
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
                                    Some(self.intern_type(Type::Enum(id, Vec::new())))
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

    /// Intern a generic struct application (`PinHandle<String>`, M9).
    pub(crate) fn struct_application(&mut self, struct_id: StructId, args: Vec<TypeId>) -> TypeId {
        self.intern_type(Type::Struct(struct_id, args))
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
            Type::Struct(id, args) => {
                let mut substituted = Vec::with_capacity(args.len());
                for arg in args {
                    substituted.push(self.instantiate_ty(arg, type_args));
                }
                self.intern_type(Type::Struct(id, substituted))
            }
            Type::Interface(id, args) => {
                let mut substituted = Vec::with_capacity(args.len());
                for arg in args {
                    substituted.push(self.instantiate_ty(arg, type_args));
                }
                self.intern_type(Type::Interface(id, substituted))
            }
            Type::Array(element) => {
                let element = self.instantiate_ty(element, type_args);
                self.intern_type(Type::Array(element))
            }
            Type::MutableArray(element) => {
                let element = self.instantiate_ty(element, type_args);
                self.intern_type(Type::MutableArray(element))
            }
            Type::Enum(id, args) => {
                let mut substituted = Vec::with_capacity(args.len());
                for arg in args {
                    substituted.push(self.instantiate_ty(arg, type_args));
                }
                self.intern_type(Type::Enum(id, substituted))
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
            _ => ty,
        }
    }

    /// Replace the owner-parameter prefix of a method signature and rebase
    /// its remaining method parameters behind a target owner's prefix. This
    /// is used when an interface method is compared with a concrete
    /// implementation whose own generic host has a distinct parameter
    /// namespace.
    pub(crate) fn instantiate_method_owner_ty(
        &mut self,
        ty: TypeId,
        owner_args: &[TypeId],
        source_owner_count: usize,
        target_owner_count: usize,
    ) -> TypeId {
        match self.types[ty].clone() {
            Type::Param(index) => {
                let index = index.into_raw() as usize;
                if index < source_owner_count {
                    owner_args[index]
                } else {
                    self.intern_type(Type::Param(hir::TypeParamId::from_raw(
                        (target_owner_count + index - source_owner_count) as u32,
                    )))
                }
            }
            Type::Struct(id, args) => {
                let args = args
                    .into_iter()
                    .map(|arg| {
                        self.instantiate_method_owner_ty(
                            arg,
                            owner_args,
                            source_owner_count,
                            target_owner_count,
                        )
                    })
                    .collect();
                self.intern_type(Type::Struct(id, args))
            }
            Type::Interface(id, args) => {
                let args = args
                    .into_iter()
                    .map(|arg| {
                        self.instantiate_method_owner_ty(
                            arg,
                            owner_args,
                            source_owner_count,
                            target_owner_count,
                        )
                    })
                    .collect();
                self.intern_type(Type::Interface(id, args))
            }
            Type::Array(element) => {
                let element = self.instantiate_method_owner_ty(
                    element,
                    owner_args,
                    source_owner_count,
                    target_owner_count,
                );
                self.intern_type(Type::Array(element))
            }
            Type::MutableArray(element) => {
                let element = self.instantiate_method_owner_ty(
                    element,
                    owner_args,
                    source_owner_count,
                    target_owner_count,
                );
                self.intern_type(Type::MutableArray(element))
            }
            Type::Enum(id, args) => {
                let args = args
                    .into_iter()
                    .map(|arg| {
                        self.instantiate_method_owner_ty(
                            arg,
                            owner_args,
                            source_owner_count,
                            target_owner_count,
                        )
                    })
                    .collect();
                self.intern_type(Type::Enum(id, args))
            }
            Type::Tuple(elements) => {
                let elements = elements
                    .into_iter()
                    .map(|element| {
                        self.instantiate_method_owner_ty(
                            element,
                            owner_args,
                            source_owner_count,
                            target_owner_count,
                        )
                    })
                    .collect();
                self.intern_type(Type::Tuple(elements))
            }
            Type::Function(id) => self
                .instantiate_function_type(id, |this, ty| {
                    Some(this.instantiate_method_owner_ty(
                        ty,
                        owner_args,
                        source_owner_count,
                        target_owner_count,
                    ))
                })
                .expect("complete owner substitution"),
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
            Type::Struct(id, args) => {
                let mut substituted = Vec::with_capacity(args.len());
                for arg in args {
                    substituted.push(self.try_substitute(arg, bindings)?);
                }
                Some(self.intern_type(Type::Struct(id, substituted)))
            }
            Type::Interface(id, args) => {
                let mut substituted = Vec::with_capacity(args.len());
                for arg in args {
                    substituted.push(self.try_substitute(arg, bindings)?);
                }
                Some(self.intern_type(Type::Interface(id, substituted)))
            }
            Type::Array(element) => {
                let element = self.try_substitute(element, bindings)?;
                Some(self.intern_type(Type::Array(element)))
            }
            Type::MutableArray(element) => {
                let element = self.try_substitute(element, bindings)?;
                Some(self.intern_type(Type::MutableArray(element)))
            }
            Type::Enum(id, args) => {
                let mut substituted = Vec::with_capacity(args.len());
                for arg in args {
                    substituted.push(self.try_substitute(arg, bindings)?);
                }
                Some(self.intern_type(Type::Enum(id, substituted)))
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
    /// argument list, so `PinHandle<Int>` and `PinHandle<String>` are
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
            (Type::Class(a), Type::Class(b)) => self.class_inherits(a, b),
            (Type::Interface(a, a_args), Type::Interface(b, b_args)) if a == b => {
                let variances: Vec<hir::Variance> = self.interfaces[a]
                    .type_params
                    .iter()
                    .map(|param| param.variance)
                    .collect();
                variances
                    .into_iter()
                    .zip(a_args)
                    .zip(b_args)
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
            (Type::Class(class), Type::Interface(..)) => self
                .class_interfaces_all(class)
                .into_iter()
                .any(|implemented| self.is_subtype(implemented, b)),
            (Type::Struct(id, args), Type::Interface(..)) => {
                let interfaces = self.structs[id].interfaces.clone();
                interfaces.into_iter().any(|implemented| {
                    let implemented = self.instantiate_ty(implemented, &args);
                    self.is_subtype(implemented, b)
                })
            }
            (Type::Enum(id, args), Type::Interface(..)) => {
                let interfaces = self.enums[id].interfaces.clone();
                interfaces.into_iter().any(|implemented| {
                    let implemented = self.instantiate_ty(implemented, &args);
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
            Type::Interface(id, args) => {
                return (id == target).then_some(args);
            }
            Type::Class(class) => self.class_interfaces_all(class),
            Type::Struct(id, args) => self.structs[id]
                .interfaces
                .clone()
                .into_iter()
                .map(|implemented| self.instantiate_ty(implemented, &args))
                .collect(),
            Type::Enum(id, args) => self.enums[id]
                .interfaces
                .clone()
                .into_iter()
                .map(|implemented| self.instantiate_ty(implemented, &args))
                .collect(),
            _ => Vec::new(),
        };
        candidates.into_iter().find_map(|candidate| {
            let Type::Interface(id, args) = self.types[candidate].clone() else {
                return None;
            };
            (id == target).then_some(args)
        })
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
            &Type::Class(id) => {
                self.classes[id].modifier != hir::ClassModifier::Final
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
            | Type::Tuple(_) => true,
            Type::Param(index) => self
                .type_params_in_scope
                .get(index.into_raw() as usize)
                .is_none_or(|param| param.kind != hir::TypeParamKind::Ref),
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
            if self.type_satisfies_kind(arg, param.kind) {
                continue;
            }
            let required = match param.kind {
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
        valid
    }

    pub(crate) fn type_arguments_satisfy_kinds(
        &self,
        params: &[hir::TypeParamDecl],
        args: &[TypeId],
    ) -> bool {
        params
            .iter()
            .zip(args)
            .all(|(param, &arg)| self.type_satisfies_kind(arg, param.kind))
    }

    fn type_satisfies_kind(&self, ty: TypeId, required: hir::TypeParamKind) -> bool {
        if required == hir::TypeParamKind::Any {
            return true;
        }
        if let Type::Param(index) = self.types[ty] {
            let actual = self
                .type_params_in_scope
                .get(index.into_raw() as usize)
                .map(|param| param.kind)
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
        (Type::Struct(x, x_args), Type::Struct(y, y_args)) => {
            x == y
                && x_args.len() == y_args.len()
                && x_args
                    .iter()
                    .zip(y_args.iter())
                    .all(|(&x, &y)| type_value_equal(types, x, y))
        }
        (Type::Class(x), Type::Class(y)) => x == y,
        (Type::Interface(x, x_args), Type::Interface(y, y_args)) => {
            x == y
                && x_args.len() == y_args.len()
                && x_args
                    .iter()
                    .zip(y_args.iter())
                    .all(|(&x, &y)| type_value_equal(types, x, y))
        }
        (Type::Array(x), Type::Array(y)) | (Type::MutableArray(x), Type::MutableArray(y)) => {
            type_value_equal(types, *x, *y)
        }
        (Type::Param(x), Type::Param(y)) => x == y,
        (Type::Enum(x, x_args), Type::Enum(y, y_args)) => {
            x == y
                && x_args.len() == y_args.len()
                && x_args
                    .iter()
                    .zip(y_args.iter())
                    .all(|(&x, &y)| type_value_equal(types, x, y))
        }
        (Type::Tuple(xs), Type::Tuple(ys)) => {
            xs.len() == ys.len()
                && xs
                    .iter()
                    .zip(ys.iter())
                    .all(|(&x, &y)| type_value_equal(types, x, y))
        }
        (Type::Function(x), Type::Function(y)) => x == y,
        _ => false,
    }
}

#[allow(clippy::too_many_arguments)]
fn type_name(
    types: &Arena<Type>,
    function_types: &Arena<hir::FunctionType>,
    structs: &Arena<StructDecl>,
    enums: &Arena<EnumDecl>,
    classes: &Arena<ClassDecl>,
    interfaces: &Arena<InterfaceDecl>,
    type_params: &[hir::TypeParamDecl],
    ty: TypeId,
) -> String {
    match &types[ty] {
        Type::Unit => "Unit".to_string(),
        Type::Int => "Int".to_string(),
        Type::UInt => "UInt".to_string(),
        Type::Boolean => "Boolean".to_string(),
        Type::String => "String".to_string(),
        Type::Struct(id, args) => {
            if args.is_empty() {
                structs[*id].name.clone()
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
                            type_params,
                            *t,
                        )
                    })
                    .collect();
                format!("{}<{}>", structs[*id].name, inner.join(", "))
            }
        }
        Type::Class(id) => classes[*id].name.clone(),
        Type::Interface(id, args) => {
            if args.is_empty() {
                interfaces[*id].name.clone()
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
                            type_params,
                            *t,
                        )
                    })
                    .collect();
                format!("{}<{}>", interfaces[*id].name, inner.join(", "))
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
                type_params,
                *element,
            );
            format!("MutableArray<{inner}>")
        }
        Type::Enum(id, args) => {
            let name = &enums[*id].name;
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
                type_params,
                function.return_type,
            );
            let suspend = if function.is_suspend { "suspend " } else { "" };
            format!("{suspend}({}) -> {return_type}", parameters.join(", "))
        }
        Type::Param(index) => type_params
            .get(index.into_raw() as usize)
            .map(|param| param.name.clone())
            .unwrap_or_else(|| format!("T{}", index.into_raw())),
    }
}
