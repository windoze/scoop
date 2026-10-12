use super::*;

// --- type annotations ---

pub(crate) fn ty_named(name: &str) -> TypeRef {
    TypeRef {
        annotations: Vec::new(),
        kind: TypeRefKind::Named(ident(name)),
        span: sp(),
    }
}

pub(crate) fn ty_tuple(elements: Vec<TypeRef>) -> TypeRef {
    TypeRef {
        annotations: Vec::new(),
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
        annotations: Vec::new(),
        kind: TypeRefKind::Function(ast::FunctionTypeRef {
            receiver: None,
            is_suspend,
            parameters,
            return_type: Box::new(return_type),
        }),
        span: sp(),
    }
}

pub(crate) fn ty_nullable(inner: TypeRef) -> TypeRef {
    TypeRef {
        annotations: Vec::new(),
        kind: TypeRefKind::Nullable(Box::new(inner)),
        span: sp(),
    }
}

/// `Name<T1, ...>` (M5: `Array<Int>` / `MutableArray<Int>`).
pub(crate) fn ty_generic(name: &str, args: Vec<TypeRef>) -> TypeRef {
    TypeRef {
        annotations: Vec::new(),
        kind: TypeRefKind::Generic(ident(name), args),
        span: sp(),
    }
}
