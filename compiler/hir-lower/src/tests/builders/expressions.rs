use super::*;

// --- expressions ---

pub(crate) fn str_lit(value: &str) -> Expr {
    Expr::StringLiteral {
        value: value.to_string(),
        span: sp(),
    }
}

pub(crate) fn int_lit(value: i64) -> Expr {
    Expr::IntLiteral { value, span: sp() }
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

/// The `Name(...)` construction node (parser output when `Name` is
/// known to be a struct or an enum variant path).
pub(crate) fn struct_init(name: &str, args: Vec<Expr>) -> Expr {
    Expr::StructInit {
        name: ident(name),
        args: call_arguments(args),
        span: sp(),
    }
}

pub(crate) fn call(name: &str, args: Vec<Expr>) -> Expr {
    Expr::Call(CallExpr {
        callee: ident(name),
        type_args: Vec::new(),
        args: call_arguments(args),
        span: sp(),
    })
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
    Expr::Call(CallExpr {
        callee: ident(name),
        type_args: Vec::new(),
        args,
        span: sp(),
    })
}

pub(crate) fn typed_call(name: &str, type_args: Vec<TypeRef>, args: Vec<Expr>) -> Expr {
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
        lhs: Box::new(lhs),
        rhs: Box::new(rhs),
        span: sp(),
    }
}

pub(crate) fn unary(op: UnOp, operand: Expr) -> Expr {
    Expr::Unary {
        op,
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
        receiver: Box::new(receiver),
        indices: ast::NonEmptyVec::new(index, Vec::new()),
        span: sp(),
    }
}
