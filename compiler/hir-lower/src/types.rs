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
//! (`PinHandle<T>` / `GcHandle<T>` from the core GC facilities). HIR's
//! `Type::Struct` carries no type arguments, so an application is a
//! fresh `Type::Struct` arena entry whose arguments live in the
//! lowerer-side `generic_struct_args` table; the phantom parameters
//! exist only for HIR-level checking and mir-lower maps every entry
//! of one struct to the same MIR type. Generic struct *declarations*
//! arrive with parser support; until then the two core GC handle
//! structs are recognized by name (like `Option` / `Throwable`).

use std::collections::HashMap;

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
                    .position(|param| param == &name.text)
                {
                    self.error(
                        name.span,
                        format!(
                            "type parameter `{}` takes no type arguments",
                            self.type_params_in_scope[index]
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
                // Generic structs (M9: the core GC handle types, see
                // the module docs); every other struct is not generic.
                if let Some(&(struct_id, _)) = self.structs_by_name.get(&name.text) {
                    let Some(&arity) = self.generic_structs.get(&struct_id) else {
                        self.error(name.span, format!("struct `{}` is not generic", name.text));
                        return None;
                    };
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
                    return Some(self.struct_application(struct_id, resolved));
                }
                let Some(&enum_id) = self.enums_by_name.get(&name.text) else {
                    let what = if self.classes_by_name.contains_key(&name.text) {
                        format!("class `{}` is not generic", name.text)
                    } else if self.interfaces_by_name.contains_key(&name.text) {
                        format!("interface `{}` is not generic", name.text)
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
                Some(self.intern_type(Type::Enum(enum_id, resolved)))
            }
            ast::TypeRefKind::Named(name) => {
                if let Some(index) = self
                    .type_params_in_scope
                    .iter()
                    .position(|param| param == &name.text)
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
                            if let Some(&arity) = self.generic_structs.get(&struct_id) {
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
                        if let Some(&(_, ty)) = self.interfaces_by_name.get(&name.text) {
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
    /// The candidate is allocated first so `type_value_equal` can
    /// consult `generic_struct_args` for nested generic struct
    /// applications; on a dedup hit the fresh entry simply stays
    /// unreferenced (nothing iterates the arena semantically).
    pub(crate) fn intern_type(&mut self, candidate: Type) -> TypeId {
        let id = self.types.alloc(candidate);
        for (other, _) in self.types.iter() {
            if other != id && type_value_equal(&self.types, &self.generic_struct_args, other, id) {
                return other;
            }
        }
        id
    }

    /// Intern a generic struct application (`PinHandle<String>`, M9).
    /// HIR's `Type::Struct` carries no type arguments, so each
    /// distinct application is a fresh `Type::Struct(struct_id)` arena
    /// entry with the arguments recorded in `generic_struct_args`;
    /// equality and rendering consult that table, and mir-lower maps
    /// every entry of one struct to the same MIR type.
    pub(crate) fn struct_application(&mut self, struct_id: StructId, args: Vec<TypeId>) -> TypeId {
        for (id, ty) in self.types.iter() {
            if !matches!(ty, Type::Struct(existing) if *existing == struct_id) {
                continue;
            }
            let Some((_, existing_args)) = self.generic_struct_args.get(&id) else {
                continue;
            };
            if existing_args.len() == args.len()
                && existing_args
                    .iter()
                    .zip(args.iter())
                    .all(|(&x, &y)| self.types_equal(x, y))
            {
                return id;
            }
        }
        let id = self.types.alloc(Type::Struct(struct_id));
        self.generic_struct_args.insert(id, (struct_id, args));
        id
    }

    /// Substitute bound type arguments for `Type::Param`, recursively.
    /// Used both for generic function instantiation (parameters index
    /// `Function::type_params`) and for enum variant field types
    /// (parameters index `EnumDecl::type_params`). Callers guarantee
    /// every parameter is bound (unbound parameters are diagnosed at
    /// the use site first).
    pub(crate) fn instantiate_ty(&mut self, ty: TypeId, type_args: &[TypeId]) -> TypeId {
        // Generic struct applications substitute inside their argument
        // list and re-intern (the arguments live in the side table,
        // not in the `Type` value).
        if let Some((struct_id, args)) = self.generic_struct_args.get(&ty).cloned() {
            let substituted = args
                .iter()
                .map(|&arg| self.instantiate_ty(arg, type_args))
                .collect();
            return self.struct_application(struct_id, substituted);
        }
        match self.types[ty].clone() {
            Type::Param(index) => type_args[index.into_raw() as usize],
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
        if let Some((struct_id, args)) = self.generic_struct_args.get(&ty).cloned() {
            let mut substituted = Vec::with_capacity(args.len());
            for arg in args {
                substituted.push(self.try_substitute(arg, bindings)?);
            }
            return Some(self.struct_application(struct_id, substituted));
        }
        match self.types[ty].clone() {
            Type::Param(index) => bindings.get(index.into_raw() as usize).copied().flatten(),
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
        type_value_equal(&self.types, &self.generic_struct_args, a, b)
    }

    /// Render a type for diagnostics. Type parameters render with
    /// their declared name while the owning function or enum is in
    /// scope.
    pub(crate) fn type_name(&self, ty: TypeId) -> String {
        type_name(
            &self.types,
            &self.generic_struct_args,
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
    pub(crate) fn is_subtype(&self, a: TypeId, b: TypeId) -> bool {
        if self.types_equal(a, b) {
            return true;
        }
        match (&self.types[a], &self.types[b]) {
            (_, Type::Any) => true,
            (&Type::Class(a), &Type::Class(b)) => self.class_inherits(a, b),
            (&Type::Class(a), &Type::Interface(i)) => self.class_implements(a, i),
            (&Type::Struct(s), &Type::Interface(i)) => self.structs[s].interfaces.contains(&i),
            (&Type::Enum(e, _), &Type::Interface(i)) => self.enums[e].interfaces.contains(&i),
            _ => false,
        }
    }

    /// Whether a value of static type `a` could ever hold a `b` at run
    /// time — the static premise of `is` / `as` / `as?` (a check
    /// between unrelated types is diagnosed as useless). Beyond the
    /// subtyping relation in either direction, `Any` and interfaces
    /// can hold anything below them, and an open/abstract class may
    /// gain an interface implementation in a subclass.
    pub(crate) fn could_hold(&self, a: TypeId, b: TypeId) -> bool {
        if self.is_subtype(a, b) || self.is_subtype(b, a) {
            return true;
        }
        match &self.types[a] {
            Type::Any | Type::Interface(_) => true,
            &Type::Class(id) => {
                self.classes[id].modifier != hir::ClassModifier::Final
                    && matches!(self.types[b], Type::Interface(_))
            }
            _ => false,
        }
    }

    /// Value types (spec 3): everything that is not a reference. They
    /// cross into reference types (`Any` / interfaces) only by boxing
    /// (spec 4.4.4).
    pub(crate) fn is_value_ty(&self, ty: TypeId) -> bool {
        matches!(
            self.types[ty],
            Type::Unit
                | Type::Int
                | Type::UInt
                | Type::Boolean
                | Type::Struct(_)
                | Type::Enum(..)
                | Type::Tuple(_)
                | Type::Param(_)
        )
    }

    /// Reference types (spec 3): classes, interfaces, `Any`, strings
    /// and the built-in array types. `===` / `!==` only apply to these
    /// (spec 4.4.2).
    pub(crate) fn is_ref_ty(&self, ty: TypeId) -> bool {
        !self.is_value_ty(ty)
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

fn type_value_equal(
    types: &Arena<Type>,
    struct_args: &HashMap<TypeId, (StructId, Vec<TypeId>)>,
    a: TypeId,
    b: TypeId,
) -> bool {
    match (&types[a], &types[b]) {
        (Type::Unit, Type::Unit)
        | (Type::Int, Type::Int)
        | (Type::UInt, Type::UInt)
        | (Type::Boolean, Type::Boolean)
        | (Type::String, Type::String)
        | (Type::Any, Type::Any) => true,
        (Type::Struct(x), Type::Struct(y)) => {
            x == y
                && match (struct_args.get(&a), struct_args.get(&b)) {
                    (None, None) => true,
                    (Some((_, xs)), Some((_, ys))) => {
                        xs.len() == ys.len()
                            && xs
                                .iter()
                                .zip(ys.iter())
                                .all(|(&x, &y)| type_value_equal(types, struct_args, x, y))
                    }
                    // A bare struct type never equals an application.
                    _ => false,
                }
        }
        (Type::Class(x), Type::Class(y)) => x == y,
        (Type::Interface(x), Type::Interface(y)) => x == y,
        (Type::Array(x), Type::Array(y)) | (Type::MutableArray(x), Type::MutableArray(y)) => {
            type_value_equal(types, struct_args, *x, *y)
        }
        (Type::Param(x), Type::Param(y)) => x == y,
        (Type::Enum(x, x_args), Type::Enum(y, y_args)) => {
            x == y
                && x_args.len() == y_args.len()
                && x_args
                    .iter()
                    .zip(y_args.iter())
                    .all(|(&x, &y)| type_value_equal(types, struct_args, x, y))
        }
        (Type::Tuple(xs), Type::Tuple(ys)) => {
            xs.len() == ys.len()
                && xs
                    .iter()
                    .zip(ys.iter())
                    .all(|(&x, &y)| type_value_equal(types, struct_args, x, y))
        }
        _ => false,
    }
}

#[allow(clippy::too_many_arguments)]
fn type_name(
    types: &Arena<Type>,
    struct_args: &HashMap<TypeId, (StructId, Vec<TypeId>)>,
    structs: &Arena<StructDecl>,
    enums: &Arena<EnumDecl>,
    classes: &Arena<ClassDecl>,
    interfaces: &Arena<InterfaceDecl>,
    type_params: &[String],
    ty: TypeId,
) -> String {
    match &types[ty] {
        Type::Unit => "Unit".to_string(),
        Type::Int => "Int".to_string(),
        Type::UInt => "UInt".to_string(),
        Type::Boolean => "Boolean".to_string(),
        Type::String => "String".to_string(),
        Type::Struct(id) => match struct_args.get(&ty) {
            Some((_, args)) => {
                let inner: Vec<String> = args
                    .iter()
                    .map(|t| {
                        type_name(
                            types,
                            struct_args,
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
            None => structs[*id].name.clone(),
        },
        Type::Class(id) => classes[*id].name.clone(),
        Type::Interface(id) => interfaces[*id].name.clone(),
        Type::Any => "Any".to_string(),
        Type::Array(element) => {
            let inner = type_name(
                types,
                struct_args,
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
                struct_args,
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
                            struct_args,
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
                        struct_args,
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
        Type::Param(index) => type_params
            .get(index.into_raw() as usize)
            .cloned()
            .unwrap_or_else(|| format!("T{}", index.into_raw())),
    }
}
