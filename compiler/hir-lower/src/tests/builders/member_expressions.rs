use super::*;

/// `this`.
pub(crate) fn this_expr() -> Expr {
    Expr::This { span: sp() }
}

/// `receiver.name(args)`.
pub(crate) fn method_call(receiver: Expr, name: &str, args: Vec<Expr>) -> Expr {
    Expr::MethodCall {
        receiver: Box::new(receiver),
        name: ident(name),
        navigation: ast::Navigation::Direct,
        type_args: Vec::new(),
        args: call_arguments(args),
        span: sp(),
    }
}

pub(crate) fn source_method_call(receiver: Expr, name: &str, args: Vec<ast::CallArgument>) -> Expr {
    Expr::MethodCall {
        receiver: Box::new(receiver),
        name: ident(name),
        navigation: ast::Navigation::Direct,
        type_args: Vec::new(),
        args,
        span: sp(),
    }
}

pub(crate) fn typed_method_call(
    receiver: Expr,
    name: &str,
    type_args: Vec<TypeRef>,
    args: Vec<Expr>,
) -> Expr {
    Expr::MethodCall {
        receiver: Box::new(receiver),
        name: ident(name),
        navigation: ast::Navigation::Direct,
        type_args,
        args: call_arguments(args),
        span: sp(),
    }
}

/// `operand is T` / `operand !is T`.
pub(crate) fn is_ty(operand: Expr, ty: TypeRef, negated: bool) -> Expr {
    Expr::Is {
        operand: Box::new(operand),
        ty,
        negated,
        span: sp(),
    }
}

/// `operand as T` / `operand as? T`.
pub(crate) fn cast_ty(operand: Expr, ty: TypeRef, optional: bool) -> Expr {
    Expr::Cast {
        operand: Box::new(operand),
        ty,
        optional,
        span: sp(),
    }
}
