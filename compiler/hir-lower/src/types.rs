//! Type resolution and comparison on the partially built module.
//!
//! `scoop_hir::types_equal` / `scoop_hir::type_name` operate on a
//! finished `hir::Module`; lowering needs the same operations on the
//! arenas while the module is still being built, so they are
//! reimplemented here over `&Arena<Type>` directly.

use la_arena::Arena;
use scoop_ast as ast;
use scoop_hir::{StructDecl, Type, TypeId};

use crate::Lowerer;

impl Lowerer {
    /// Resolve a type annotation (`Int`, `Point`, `(Int, String)`, ...).
    pub(crate) fn resolve_type_ref(&mut self, ty_ref: &ast::TypeRef) -> Option<TypeId> {
        match &ty_ref.kind {
            ast::TypeRefKind::Unit => Some(self.unit),
            ast::TypeRefKind::Named(name) => match name.text.as_str() {
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
            },
            ast::TypeRefKind::Tuple(elements) => {
                let mut resolved = Vec::with_capacity(elements.len());
                for element in elements {
                    resolved.push(self.resolve_type_ref(element)?);
                }
                Some(self.intern_tuple(resolved))
            }
        }
    }

    /// Intern a tuple type, so structurally equal tuple types share a
    /// single `TypeId`.
    pub(crate) fn intern_tuple(&mut self, elements: Vec<TypeId>) -> TypeId {
        let candidate = Type::Tuple(elements);
        for (id, ty) in self.types.iter() {
            if type_value_equal(&self.types, ty, &candidate) {
                return id;
            }
        }
        self.types.alloc(candidate)
    }

    /// Structural type equality (tuple types compare by elements).
    pub(crate) fn types_equal(&self, a: TypeId, b: TypeId) -> bool {
        type_value_equal(&self.types, &self.types[a], &self.types[b])
    }

    /// Render a type for diagnostics.
    pub(crate) fn type_name(&self, ty: TypeId) -> String {
        type_name(&self.types, &self.structs, ty)
    }
}

fn type_value_equal(types: &Arena<Type>, a: &Type, b: &Type) -> bool {
    match (a, b) {
        (Type::Unit, Type::Unit)
        | (Type::Int, Type::Int)
        | (Type::Boolean, Type::Boolean)
        | (Type::String, Type::String) => true,
        (Type::Struct(x), Type::Struct(y)) => x == y,
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

fn type_name(types: &Arena<Type>, structs: &Arena<StructDecl>, ty: TypeId) -> String {
    match &types[ty] {
        Type::Unit => "Unit".to_string(),
        Type::Int => "Int".to_string(),
        Type::Boolean => "Boolean".to_string(),
        Type::String => "String".to_string(),
        Type::Struct(id) => structs[*id].name.clone(),
        Type::Tuple(elements) => {
            let inner: Vec<String> = elements
                .iter()
                .map(|t| type_name(types, structs, *t))
                .collect();
            format!("({})", inner.join(", "))
        }
    }
}
