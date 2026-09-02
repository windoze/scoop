use crate::Span;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ident {
    pub text: String,
    pub span: Span,
}

/// A type annotation, e.g. in struct fields and `val x: T = ...`.
#[derive(Debug, Clone, PartialEq)]
pub struct TypeRef {
    pub kind: TypeRefKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypeRefKind {
    Named(Ident),
    /// `Name<T1, T2>` — a generic type application (e.g. `Box<Int>`).
    Generic(Ident, Vec<TypeRef>),
    Tuple(Vec<TypeRef>),
    /// The `Unit` type name (also written `()` in type position).
    Unit,
    /// `(P1, P2, ...) -> R` / `suspend (P1, P2, ...) -> R`.
    Function(FunctionTypeRef),
    /// `T?` — desugars to `Option<T>` in HIR (spec 7.1).
    Nullable(Box<TypeRef>),
}

/// The structural source form of a function type. Parameter names,
/// defaults and `vararg` are deliberately absent from function type
/// identity (spec 8.1.1).
#[derive(Debug, Clone, PartialEq)]
pub struct FunctionTypeRef {
    pub is_suspend: bool,
    pub parameters: Vec<TypeRef>,
    pub return_type: Box<TypeRef>,
}

/// Parser-local identity of one lambda expression.  This is deliberately
/// distinct from every named/anonymous callable identity; HIR remaps it into
/// the Cone-wide lambda arena.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LambdaId(pub u32);

/// Parser-local identity of one anonymous-function expression.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AnonymousFunctionId(pub u32);

/// Parser-local identity of one callable-reference expression.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CallableReferenceId(pub u32);
