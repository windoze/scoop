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

use la_arena::Arena;
use scoop_ast as ast;
use scoop_hir::{EnumDecl, StructDecl, Type, TypeId};

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
                // generic enums (structs are not generic yet). M5: the
                // built-in `Array<T>` / `MutableArray<T>`.
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
                let Some(&enum_id) = self.enums_by_name.get(&name.text) else {
                    let what = if self.structs_by_name.contains_key(&name.text) {
                        format!("struct `{}` is not generic", name.text)
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
                    return Some(self.intern_type(Type::Param(index as u32)));
                }
                match name.text.as_str() {
                    "Unit" => Some(self.unit),
                    "Int" => Some(self.int),
                    "Boolean" => Some(self.boolean),
                    "String" => Some(self.string),
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
                        if let Some(&(_, ty)) = self.structs_by_name.get(&name.text) {
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
    pub(crate) fn intern_type(&mut self, candidate: Type) -> TypeId {
        for (id, ty) in self.types.iter() {
            if type_value_equal(&self.types, ty, &candidate) {
                return id;
            }
        }
        self.types.alloc(candidate)
    }

    /// Substitute bound type arguments for `Type::Param`, recursively.
    /// Used both for generic function instantiation (parameters index
    /// `Function::type_params`) and for enum variant field types
    /// (parameters index `EnumDecl::type_params`). Callers guarantee
    /// every parameter is bound (unbound parameters are diagnosed at
    /// the use site first).
    pub(crate) fn instantiate_ty(&mut self, ty: TypeId, type_args: &[TypeId]) -> TypeId {
        match self.types[ty].clone() {
            Type::Param(index) => type_args[index as usize],
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
        match self.types[ty].clone() {
            Type::Param(index) => bindings.get(index as usize).copied().flatten(),
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
    /// elementwise).
    pub(crate) fn types_equal(&self, a: TypeId, b: TypeId) -> bool {
        type_value_equal(&self.types, &self.types[a], &self.types[b])
    }

    /// Render a type for diagnostics. Type parameters render with
    /// their declared name while the owning function or enum is in
    /// scope.
    pub(crate) fn type_name(&self, ty: TypeId) -> String {
        type_name(
            &self.types,
            &self.structs,
            &self.enums,
            &self.type_params_in_scope,
            ty,
        )
    }
}

fn type_value_equal(types: &Arena<Type>, a: &Type, b: &Type) -> bool {
    match (a, b) {
        (Type::Unit, Type::Unit)
        | (Type::Int, Type::Int)
        | (Type::Boolean, Type::Boolean)
        | (Type::String, Type::String) => true,
        (Type::Struct(x), Type::Struct(y)) => x == y,
        (Type::Array(x), Type::Array(y)) | (Type::MutableArray(x), Type::MutableArray(y)) => {
            type_value_equal(types, &types[*x], &types[*y])
        }
        (Type::Param(x), Type::Param(y)) => x == y,
        (Type::Enum(x, x_args), Type::Enum(y, y_args)) => {
            x == y
                && x_args.len() == y_args.len()
                && x_args
                    .iter()
                    .zip(y_args.iter())
                    .all(|(&x, &y)| type_value_equal(types, &types[x], &types[y]))
        }
        (Type::Tuple(xs), Type::Tuple(ys)) => {
            xs.len() == ys.len()
                && xs
                    .iter()
                    .zip(ys.iter())
                    .all(|(&x, &y)| type_value_equal(types, &types[x], &types[y]))
        }
        _ => false,
    }
}

fn type_name(
    types: &Arena<Type>,
    structs: &Arena<StructDecl>,
    enums: &Arena<EnumDecl>,
    type_params: &[String],
    ty: TypeId,
) -> String {
    match &types[ty] {
        Type::Unit => "Unit".to_string(),
        Type::Int => "Int".to_string(),
        Type::Boolean => "Boolean".to_string(),
        Type::String => "String".to_string(),
        Type::Struct(id) => structs[*id].name.clone(),
        Type::Array(element) => {
            let inner = type_name(types, structs, enums, type_params, *element);
            format!("Array<{inner}>")
        }
        Type::MutableArray(element) => {
            let inner = type_name(types, structs, enums, type_params, *element);
            format!("MutableArray<{inner}>")
        }
        Type::Enum(id, args) => {
            let name = &enums[*id].name;
            if args.is_empty() {
                name.clone()
            } else {
                let inner: Vec<String> = args
                    .iter()
                    .map(|t| type_name(types, structs, enums, type_params, *t))
                    .collect();
                format!("{}<{}>", name, inner.join(", "))
            }
        }
        Type::Tuple(elements) => {
            let inner: Vec<String> = elements
                .iter()
                .map(|t| type_name(types, structs, enums, type_params, *t))
                .collect();
            format!("({})", inner.join(", "))
        }
        Type::Param(index) => type_params
            .get(*index as usize)
            .cloned()
            .unwrap_or_else(|| format!("T{index}")),
    }
}
