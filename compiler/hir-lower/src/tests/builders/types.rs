use super::*;

// --- type annotations ---

pub(crate) fn ty_named(name: &str) -> TypeRef {
    TypeRef {
        kind: TypeRefKind::Named(ident(name)),
        span: sp(),
    }
}

pub(crate) fn ty_tuple(elements: Vec<TypeRef>) -> TypeRef {
    TypeRef {
        kind: TypeRefKind::Tuple(elements),
        span: sp(),
    }
}

pub(crate) fn ty_function(
    is_suspend: bool,
    parameters: Vec<TypeRef>,
    return_type: TypeRef,
) -> TypeRef {
    TypeRef {
        kind: TypeRefKind::Function(ast::FunctionTypeRef {
            is_suspend,
            parameters,
            return_type: Box::new(return_type),
        }),
        span: sp(),
    }
}

pub(crate) fn ty_nullable(inner: TypeRef) -> TypeRef {
    TypeRef {
        kind: TypeRefKind::Nullable(Box::new(inner)),
        span: sp(),
    }
}

/// `Name<T1, ...>` (M5: `Array<Int>` / `MutableArray<Int>`).
pub(crate) fn ty_generic(name: &str, args: Vec<TypeRef>) -> TypeRef {
    TypeRef {
        kind: TypeRefKind::Generic(ident(name), args),
        span: sp(),
    }
}
