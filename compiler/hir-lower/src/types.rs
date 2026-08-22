//! Type resolution, interning, substitution and comparison on the
//! partially built module.
//!
//! `scoop_hir::types_equal` / `scoop_hir::type_name` operate on a
//! finished `hir::Module`; lowering needs the same operations on the
//! arenas while the module is still being built, so they are
//! reimplemented here over `&Arena<Type>` directly.
//!
//! M3 (milestone3 DESIGN.md 2.2): `T?` annotations resolve to the
//! builtin `Option<T>` (interned on demand — there is no well-known
//! Option type), and a generic function's type parameters resolve to
//! `Type::Param(index)` while its signature and body are being lowered.

use la_arena::Arena;
use scoop_ast as ast;
use scoop_hir::{StructDecl, Type, TypeId};

use crate::Lowerer;

impl Lowerer {
    /// Resolve a type annotation (`Int`, `Point`, `(Int, String)`,
    /// `T?`, ...). Function type parameters shadow well-known and
    /// struct types: they only scope over one function's signature and
    /// body (`type_params_in_scope` is empty everywhere else).
    pub(crate) fn resolve_type_ref(&mut self, ty_ref: &ast::TypeRef) -> Option<TypeId> {
        match &ty_ref.kind {
            ast::TypeRefKind::Unit => Some(self.unit),
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
                    _ => match self.structs_by_name.get(&name.text) {
                        Some(&(_, ty)) => Some(ty),
                        None => {
                            self.error(name.span, format!("unknown type `{}`", name.text));
                            None
                        }
                    },
                }
            }
            ast::TypeRefKind::Tuple(elements) => {
                let mut resolved = Vec::with_capacity(elements.len());
                for element in elements {
                    resolved.push(self.resolve_type_ref(element)?);
                }
                Some(self.intern_type(Type::Tuple(resolved)))
            }
            // `T?` desugars to `Option<T>` (spec 7.1); `T??` is
            // `Option<Option<T>>` and deliberately does not collapse.
            ast::TypeRefKind::Nullable(inner) => {
                let inner = self.resolve_type_ref(inner)?;
                Some(self.intern_type(Type::Option(inner)))
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
    /// Callers guarantee every parameter of the generic function is
    /// bound (unbound parameters are diagnosed at the call site first).
    pub(crate) fn instantiate_ty(&mut self, ty: TypeId, type_args: &[TypeId]) -> TypeId {
        match self.types[ty].clone() {
            Type::Param(index) => type_args[index as usize],
            Type::Option(inner) => {
                let inner = self.instantiate_ty(inner, type_args);
                self.intern_type(Type::Option(inner))
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
            Type::Option(inner) => {
                let inner = self.try_substitute(inner, bindings)?;
                Some(self.intern_type(Type::Option(inner)))
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

    /// Structural type equality (tuple and Option types compare
    /// elementwise).
    pub(crate) fn types_equal(&self, a: TypeId, b: TypeId) -> bool {
        type_value_equal(&self.types, &self.types[a], &self.types[b])
    }

    /// Render a type for diagnostics. Type parameters render with
    /// their declared name while the owning function is in scope.
    pub(crate) fn type_name(&self, ty: TypeId) -> String {
        type_name(&self.types, &self.structs, &self.type_params_in_scope, ty)
    }
}

fn type_value_equal(types: &Arena<Type>, a: &Type, b: &Type) -> bool {
    match (a, b) {
        (Type::Unit, Type::Unit)
        | (Type::Int, Type::Int)
        | (Type::Boolean, Type::Boolean)
        | (Type::String, Type::String) => true,
        (Type::Struct(x), Type::Struct(y)) => x == y,
        (Type::Param(x), Type::Param(y)) => x == y,
        (Type::Option(x), Type::Option(y)) => type_value_equal(types, &types[*x], &types[*y]),
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
    type_params: &[String],
    ty: TypeId,
) -> String {
    match &types[ty] {
        Type::Unit => "Unit".to_string(),
        Type::Int => "Int".to_string(),
        Type::Boolean => "Boolean".to_string(),
        Type::String => "String".to_string(),
        Type::Struct(id) => structs[*id].name.clone(),
        Type::Tuple(elements) => {
            let inner: Vec<String> = elements
                .iter()
                .map(|t| type_name(types, structs, type_params, *t))
                .collect();
            format!("({})", inner.join(", "))
        }
        Type::Option(inner) => {
            format!("Option<{}>", type_name(types, structs, type_params, *inner))
        }
        Type::Param(index) => type_params
            .get(*index as usize)
            .cloned()
            .unwrap_or_else(|| format!("T{index}")),
    }
}
