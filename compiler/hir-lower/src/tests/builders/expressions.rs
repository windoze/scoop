use super::*;

// --- expressions ---

pub(crate) fn str_lit(value: &str) -> Expr {
    Expr::StringLiteral {
        value: value.to_string(),
        span: sp(),
    }
}

pub(crate) fn int_lit(value: i64) -> Expr {
    Expr::IntLiteral(integer_syntax(
        u64::try_from(value).expect("test integer literals are non-negative"),
    ))
}

pub(crate) fn bool_lit(value: bool) -> Expr {
    Expr::BoolLiteral { value, span: sp() }
}

pub(crate) fn unit_lit() -> Expr {
    Expr::UnitLiteral { span: sp() }
}

pub(crate) fn var(name: &str) -> Expr {
    Expr::Var(ident(name))
}

pub(crate) fn tuple_lit(elements: Vec<Expr>) -> Expr {
    Expr::TupleLiteral {
        elements,
        span: sp(),
    }
}

/// Legacy explicit struct syntax; qualified paths use the parser's member shape.
pub(crate) fn struct_init(name: &str, args: Vec<Expr>) -> Expr {
    if name.contains('.') {
        call(name, args)
    } else {
        Expr::StructInit {
            name: ident(name),
            args: call_arguments(args),
            span: sp(),
        }
    }
}

pub(crate) fn call(name: &str, args: Vec<Expr>) -> Expr {
    source_call(name, call_arguments(args))
}

pub(crate) fn super_method_call(name: &str, args: Vec<Expr>) -> Expr {
    Expr::SuperMethodCall {
        super_span: sp(),
        name: ident(name),
        type_args: Vec::new(),
        args: call_arguments(args),
        span: sp(),
    }
}

pub(crate) fn source_call(name: &str, args: Vec<ast::CallArgument>) -> Expr {
    call_with_type_args(name, Vec::new(), args)
}

fn call_with_type_args(
    name: &str,
    type_args: Vec<ast::CallTypeArgument>,
    args: Vec<ast::CallArgument>,
) -> Expr {
    if let Some((owner, name)) = name.split_once('.') {
        Expr::MethodCall {
            receiver: Box::new(var(owner)),
            name: ident(name),
            navigation: ast::Navigation::Direct,
            type_args,
            args,
            span: sp(),
        }
    } else {
        Expr::Call(CallExpr {
            callee: ident(name),
            type_args,
            args,
            span: sp(),
        })
    }
}

pub(crate) fn typed_call(name: &str, type_args: Vec<TypeRef>, args: Vec<Expr>) -> Expr {
    call_with_type_args(
        name,
        type_args
            .into_iter()
            .map(ast::CallTypeArgument::Explicit)
            .collect(),
        call_arguments(args),
    )
}

pub(crate) fn partially_typed_call(
    name: &str,
    type_args: Vec<ast::CallTypeArgument>,
    args: Vec<Expr>,
) -> Expr {
    Expr::Call(CallExpr {
        callee: ident(name),
        type_args,
        args: call_arguments(args),
        span: sp(),
    })
}

pub(crate) fn typed_source_call(
    name: &str,
    type_args: Vec<TypeRef>,
    args: Vec<ast::CallArgument>,
) -> Expr {
    Expr::Call(CallExpr {
        callee: ident(name),
        type_args: type_args
            .into_iter()
            .map(ast::CallTypeArgument::Explicit)
            .collect(),
        args,
        span: sp(),
    })
}

pub(crate) fn partially_typed_source_call(
    name: &str,
    type_args: Vec<ast::CallTypeArgument>,
    args: Vec<ast::CallArgument>,
) -> Expr {
    Expr::Call(CallExpr {
        callee: ident(name),
        type_args,
        args,
        span: sp(),
    })
}

pub(crate) fn call_at(name: &str, args: Vec<Expr>, callee_span: Span) -> Expr {
    Expr::Call(CallExpr {
        callee: Ident {
            text: name.to_string(),
            span: callee_span,
        },
        type_args: Vec::new(),
        args: call_arguments(args),
        span: sp(),
    })
}

pub(crate) fn call_arguments(args: Vec<Expr>) -> Vec<ast::CallArgument> {
    args.into_iter()
        .map(ast::CallArgument::positional)
        .collect()
}

pub(crate) fn named_argument(name: &str, expression: Expr) -> ast::CallArgument {
    ast::CallArgument {
        name: ast::CallArgumentName::Named(ident(name)),
        spread: ast::SpreadSyntax::Plain,
        expression,
        span: sp(),
    }
}

pub(crate) fn spread_argument(expression: Expr) -> ast::CallArgument {
    ast::CallArgument {
        name: ast::CallArgumentName::Positional,
        spread: ast::SpreadSyntax::Spread(sp()),
        expression,
        span: sp(),
    }
}

pub(crate) fn field(receiver: Expr, name: &str) -> Expr {
    Expr::FieldAccess(FieldAccess {
        receiver: Box::new(receiver),
        selector: FieldSelector::Name(ident(name)),
        navigation: ast::Navigation::Direct,
        span: sp(),
    })
}

pub(crate) fn copy_update(base: Expr, fields: Vec<(&str, Expr)>) -> Expr {
    let mut fields = fields.into_iter().map(|(name, value)| ast::FieldUpdate {
        field: ident(name),
        value,
        span: sp(),
    });
    let first = fields
        .next()
        .expect("copy-update test builders require a non-empty field list");
    Expr::CopyUpdate {
        base: Box::new(base),
        fields: ast::NonEmptyVec::new(first, fields.collect()),
        span: sp(),
    }
}

/// The `?.` safe field access.
pub(crate) fn safe_field(receiver: Expr, name: &str) -> Expr {
    Expr::FieldAccess(FieldAccess {
        receiver: Box::new(receiver),
        selector: FieldSelector::Name(ident(name)),
        navigation: ast::Navigation::Safe,
        span: sp(),
    })
}

pub(crate) fn index(receiver: Expr, n: u32) -> Expr {
    Expr::FieldAccess(FieldAccess {
        receiver: Box::new(receiver),
        selector: FieldSelector::Index(n, sp()),
        navigation: ast::Navigation::Direct,
        span: sp(),
    })
}

pub(crate) fn binary(op: BinOp, lhs: Expr, rhs: Expr) -> Expr {
    Expr::Binary {
        op,
        operator_span: None,
        lhs: Box::new(lhs),
        rhs: Box::new(rhs),
        span: sp(),
    }
}

pub(crate) fn unary(op: UnOp, operand: Expr) -> Expr {
    Expr::Unary {
        op,
        operator_span: None,
        operand: Box::new(operand),
        span: sp(),
    }
}

/// The `None` variant construction (parsed as an identifier, resolved
/// by HIR against the core `Option` enum).
pub(crate) fn none() -> Expr {
    var("None")
}

/// The `Some(x)` variant construction (parsed as a plain call).
pub(crate) fn some(value: Expr) -> Expr {
    call("Some", vec![value])
}

pub(crate) fn elvis(lhs: Expr, rhs: Expr) -> Expr {
    Expr::Elvis {
        lhs: Box::new(lhs),
        rhs: Box::new(rhs),
        span: sp(),
    }
}

pub(crate) fn null_assert(operand: Expr) -> Expr {
    Expr::NullAssert {
        operand: Box::new(operand),
        span: sp(),
    }
}

/// `[e1, e2, ...]` (M5).
pub(crate) fn array_lit(elements: Vec<Expr>) -> Expr {
    Expr::ArrayLiteral {
        elements,
        span: sp(),
    }
}

/// `receiver[index]` (M5).
pub(crate) fn subscript(receiver: Expr, index: Expr) -> Expr {
    Expr::Index {
        bracket_span: None,
        receiver: Box::new(receiver),
        indices: ast::NonEmptyVec::new(index, Vec::new()),
        span: sp(),
    }
}
