use super::*;

// --- declarations ---

/// A plain `fun name() { ... }` (no type parameters, no parameters, no
/// return type annotation).
pub(crate) fn fun(name: &str, statements: Vec<Statement>) -> Decl {
    fun_sig(name, vec![], vec![], None, statements)
}

pub(crate) fn suspend_fun(name: &str, statements: Vec<Statement>) -> Decl {
    let Decl::Function(mut function) = fun(name, statements) else {
        unreachable!("fun always builds a function declaration")
    };
    function.is_suspend = true;
    Decl::Function(function)
}

pub(crate) fn with_suspend(mut method: FunctionDecl) -> FunctionDecl {
    method.is_suspend = true;
    method
}

/// A block-bodied function with a full signature.
pub(crate) fn fun_sig(
    name: &str,
    type_params: Vec<&str>,
    params: Vec<(&str, TypeRef)>,
    return_ty: Option<TypeRef>,
    statements: Vec<Statement>,
) -> Decl {
    Decl::Function(FunctionDecl {
        annotations: Vec::new(),
        is_suspend: false,
        is_override: false,
        operator: None,
        modifier: ast::MethodModifier::Final,
        receiver_ty: None,
        name: ident(name),
        type_params: type_params.into_iter().map(type_param).collect(),
        params: params
            .into_iter()
            .map(|(name, ty)| Param {
                name: ident(name),
                ty,
                syntax: ast::ParameterSyntax::Required,
                span: sp(),
            })
            .collect(),
        return_ty,
        where_clause: None,
        body: FunctionBody::Block(block(statements)),
        span: sp(),
    })
}

pub(crate) fn local_fun_sig(
    name: &str,
    type_params: Vec<&str>,
    params: Vec<(&str, TypeRef)>,
    return_ty: Option<TypeRef>,
    statements: Vec<Statement>,
) -> Statement {
    let Decl::Function(function) = fun_sig(name, type_params, params, return_ty, statements) else {
        unreachable!("fun_sig always builds a function declaration")
    };
    Statement {
        span: function.span,
        kind: StatementKind::LocalFunction(function),
    }
}

/// An expression-bodied function: `fun f(...) [: T] = expr`.
pub(crate) fn fun_expr(
    name: &str,
    type_params: Vec<&str>,
    params: Vec<(&str, TypeRef)>,
    return_ty: Option<TypeRef>,
    expr: Expr,
) -> Decl {
    Decl::Function(FunctionDecl {
        annotations: Vec::new(),
        is_suspend: false,
        is_override: false,
        operator: None,
        modifier: ast::MethodModifier::Final,
        receiver_ty: None,
        name: ident(name),
        type_params: type_params.into_iter().map(type_param).collect(),
        params: params
            .into_iter()
            .map(|(name, ty)| Param {
                name: ident(name),
                ty,
                syntax: ast::ParameterSyntax::Required,
                span: sp(),
            })
            .collect(),
        return_ty,
        where_clause: None,
        body: FunctionBody::Expr(Box::new(expr)),
        span: sp(),
    })
}

pub(crate) fn extension_expr(
    receiver_ty: TypeRef,
    name: &str,
    type_params: Vec<&str>,
    params: Vec<(&str, TypeRef)>,
    return_ty: Option<TypeRef>,
    expr: Expr,
) -> Decl {
    let Decl::Function(mut function) = fun_expr(name, type_params, params, return_ty, expr) else {
        unreachable!("fun_expr always builds a function declaration")
    };
    function.receiver_ty = Some(receiver_ty);
    Decl::Function(function)
}

/// `@Intrinsic("...") fun name(params) [: T]` (core library only).
pub(crate) fn intrinsic_fun(
    name: &str,
    intrinsic: &str,
    params: Vec<(&str, TypeRef)>,
    return_ty: Option<TypeRef>,
) -> Decl {
    intrinsic_generic_fun(name, intrinsic, vec![], params, return_ty)
}

/// `@Intrinsic("...") fun <T, ...> name(params) [: T]` (core library
/// only) — the M9 GC intrinsics are generic.
pub(crate) fn intrinsic_generic_fun(
    name: &str,
    intrinsic: &str,
    type_params: Vec<&str>,
    params: Vec<(&str, TypeRef)>,
    return_ty: Option<TypeRef>,
) -> Decl {
    Decl::Function(FunctionDecl {
        annotations: vec![ast::Annotation {
            name: ident("Intrinsic"),
            args: vec![ast::AnnotationArg {
                name: None,
                value: ast::AnnotationLiteral::String(intrinsic.to_string()),
                span: sp(),
            }],
            span: sp(),
        }],
        is_suspend: false,
        is_override: false,
        operator: None,
        modifier: ast::MethodModifier::Final,
        receiver_ty: None,
        name: ident(name),
        type_params: type_params.into_iter().map(type_param).collect(),
        params: params
            .into_iter()
            .map(|(name, ty)| Param {
                name: ident(name),
                ty,
                syntax: ast::ParameterSyntax::Required,
                span: sp(),
            })
            .collect(),
        return_ty,
        where_clause: None,
        body: FunctionBody::None,
        span: sp(),
    })
}

pub(crate) fn scoop_extern_fun(
    name: &str,
    native_symbol: &str,
    params: Vec<(&str, TypeRef)>,
    return_ty: Option<TypeRef>,
) -> Decl {
    let Decl::Function(mut function) = fun_sig(name, vec![], params, return_ty, vec![]) else {
        unreachable!()
    };
    function.annotations = vec![ast::Annotation {
        name: ident("Extern"),
        args: vec![
            ast::AnnotationArg {
                name: Some(ident("name")),
                value: ast::AnnotationLiteral::String(native_symbol.to_string()),
                span: sp(),
            },
            ast::AnnotationArg {
                name: Some(ident("abi")),
                value: ast::AnnotationLiteral::String("scoop".to_string()),
                span: sp(),
            },
        ],
        span: sp(),
    }];
    function.body = FunctionBody::None;
    Decl::Function(function)
}
