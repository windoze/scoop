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
//! M14: `Array<T>` / `MutableArray<T>` are ordinary applications of the
//! typed intrinsic core classes. Their mutability and element type are read
//! from `ClassApplicationRepresentation`, never from names or a second built-in
//! type identity.
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

use crate::{IntrinsicTypeOwner, Lowerer};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ArrayKind {
    Immutable,
    Mutable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ArrayType {
    pub(crate) kind: ArrayKind,
    pub(crate) element: TypeId,
}

mod adaptation;
mod arrays;
mod constraints;
mod display;
mod fields;
mod interning;
mod invariance;
mod kinds;
mod layout_cycles;
mod nominal;
mod qualified;
mod relations;
mod resolution;
mod substitution;
mod variants;

pub(crate) use qualified::ResolvedTypeName;

fn type_value_equal(types: &Arena<Type>, a: TypeId, b: TypeId) -> bool {
    match (&types[a], &types[b]) {
        (Type::Unit, Type::Unit)
        | (Type::Boolean, Type::Boolean)
        | (Type::String, Type::String)
        | (Type::Any, Type::Any) => true,
        (Type::Integer(x), Type::Integer(y)) => x == y,
        (Type::Struct(x), Type::Struct(y)) => x == y,
        (Type::ImportedClass(x), Type::ImportedClass(y)) => {
            x.declaration.owner() == y.declaration.owner() && x.arguments == y.arguments
        }
        (Type::ImportedInterface(x), Type::ImportedInterface(y)) => {
            x.declaration.owner() == y.declaration.owner() && x.arguments == y.arguments
        }
        (Type::Class(x), Type::Class(y)) => x == y,
        (Type::Interface(x), Type::Interface(y)) => x == y,
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
