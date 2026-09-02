use super::*;

pub(crate) fn sp() -> Span {
    Span::new(0, 0)
}

/// A span for a trailing `..` rest marker: the split between leading
/// and trailing elements is recovered by span comparison, so a
/// trailing rest must sort after the (zero-span) builder patterns.
pub(crate) fn trailing_rest() -> Span {
    Span::new(u32::MAX - 1, u32::MAX)
}

pub(crate) fn ident(text: &str) -> Ident {
    Ident {
        text: text.to_string(),
        span: sp(),
    }
}

pub(crate) fn type_param(name: &str) -> ast::TypeParamDecl {
    ast::TypeParamDecl {
        name: ident(name),
        variance: ast::Variance::Invariant,
        inline_bound: None,
        span: sp(),
    }
}

pub(crate) fn ident_at(text: &str, span: Span) -> Ident {
    Ident {
        text: text.to_string(),
        span,
    }
}

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
        args,
        span: sp(),
    }
}

pub(crate) fn call(name: &str, args: Vec<Expr>) -> Expr {
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
        args,
        span: sp(),
    })
}

pub(crate) fn field(receiver: Expr, name: &str) -> Expr {
    Expr::FieldAccess(FieldAccess {
        receiver: Box::new(receiver),
        selector: FieldSelector::Name(ident(name)),
        safe: false,
        span: sp(),
    })
}

/// The `?.` safe field access.
pub(crate) fn safe_field(receiver: Expr, name: &str) -> Expr {
    Expr::FieldAccess(FieldAccess {
        receiver: Box::new(receiver),
        selector: FieldSelector::Name(ident(name)),
        safe: true,
        span: sp(),
    })
}

pub(crate) fn index(receiver: Expr, n: u32) -> Expr {
    Expr::FieldAccess(FieldAccess {
        receiver: Box::new(receiver),
        selector: FieldSelector::Index(n, sp()),
        safe: false,
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
        index: Box::new(index),
        span: sp(),
    }
}

// --- patterns ---

pub(crate) fn pat_bind(name: &str) -> ast::Pattern {
    ast::Pattern::Binding(ident(name))
}

pub(crate) fn pat_bind_at(name: &str, span: Span) -> ast::Pattern {
    ast::Pattern::Binding(ident_at(name, span))
}

pub(crate) fn pat_wild() -> ast::Pattern {
    ast::Pattern::Wildcard { span: sp() }
}

pub(crate) fn pat_lit(expr: Expr) -> ast::Pattern {
    ast::Pattern::Literal {
        expr: Box::new(expr),
        span: sp(),
    }
}

/// `Path?(p1, p2, ..)` — enum positional variant or struct positional.
pub(crate) fn pat_pos(
    path: &[&str],
    elements: Vec<ast::Pattern>,
    rest: Option<Span>,
) -> ast::Pattern {
    ast::Pattern::Positional {
        path: path.iter().map(|p| ident(p)).collect(),
        elements,
        rest,
        span: sp(),
    }
}

/// `Path?{ f1, f2: renamed, .. }` — enum named-field variant or
/// struct field pattern.
pub(crate) fn pat_named(
    path: &[&str],
    fields: Vec<(&str, Option<&str>)>,
    rest: Option<Span>,
) -> ast::Pattern {
    ast::Pattern::Named {
        path: path.iter().map(|p| ident(p)).collect(),
        fields: fields
            .into_iter()
            .map(|(name, rename)| ast::FieldPattern {
                name: ident(name),
                rename: rename.map(ident),
                span: sp(),
            })
            .collect(),
        rest,
        span: sp(),
    }
}

pub(crate) fn pat_tuple(elements: Vec<ast::Pattern>, rest: Option<Span>) -> ast::Pattern {
    ast::Pattern::Tuple {
        elements,
        rest,
        span: sp(),
    }
}

// --- statements ---

pub(crate) fn stmt(expr: Expr) -> Statement {
    Statement {
        kind: StatementKind::Expr(expr),
        span: sp(),
    }
}

pub(crate) fn ret(value: Option<Expr>) -> Statement {
    Statement {
        kind: StatementKind::Return { value },
        span: sp(),
    }
}

pub(crate) fn val(name: &str, init: Expr) -> Statement {
    val_ty(name, None, init)
}

pub(crate) fn var_(name: &str, init: Expr) -> Statement {
    Statement {
        kind: StatementKind::ValDecl(ValDecl {
            mutable: true,
            target: pat_bind(name),
            ty: None,
            init,
            span: sp(),
        }),
        span: sp(),
    }
}

pub(crate) fn val_ty(name: &str, ty: Option<TypeRef>, init: Expr) -> Statement {
    Statement {
        kind: StatementKind::ValDecl(ValDecl {
            mutable: false,
            target: pat_bind(name),
            ty,
            init,
            span: sp(),
        }),
        span: sp(),
    }
}

/// A destructuring `val` / `var` declaration (spec 4.6).
pub(crate) fn val_pat(
    mutable: bool,
    target: ast::Pattern,
    ty: Option<TypeRef>,
    init: Expr,
) -> Statement {
    Statement {
        kind: StatementKind::ValDecl(ValDecl {
            mutable,
            target,
            ty,
            init,
            span: sp(),
        }),
        span: sp(),
    }
}

pub(crate) fn when_stmt(
    subject: Expr,
    arms: Vec<ast::WhenArm>,
    else_body: Option<Vec<Statement>>,
) -> Statement {
    Statement {
        kind: StatementKind::When(ast::When {
            subject,
            arms,
            else_body: else_body.map(block),
            span: sp(),
        }),
        span: sp(),
    }
}

pub(crate) fn arm(
    pattern: ast::Pattern,
    guard: Option<Expr>,
    body: Vec<Statement>,
) -> ast::WhenArm {
    ast::WhenArm {
        pattern,
        guard,
        body: block(body),
        span: sp(),
    }
}

pub(crate) fn assign(target: &str, value: Expr) -> Statement {
    Statement {
        kind: StatementKind::Assign(ast::Assign {
            target: ast::AssignTarget::Local(ident(target)),
            value,
            span: sp(),
        }),
        span: sp(),
    }
}

/// `receiver.field = value` (M6).
pub(crate) fn assign_field(receiver: Expr, name: &str, value: Expr) -> Statement {
    Statement {
        kind: StatementKind::Assign(ast::Assign {
            target: ast::AssignTarget::Field {
                receiver: Box::new(receiver),
                name: ident(name),
                span: sp(),
            },
            value,
            span: sp(),
        }),
        span: sp(),
    }
}

/// `receiver[index] = value` (M5).
pub(crate) fn assign_index(receiver: Expr, index: Expr, value: Expr) -> Statement {
    Statement {
        kind: StatementKind::Assign(ast::Assign {
            target: ast::AssignTarget::Index {
                receiver: Box::new(receiver),
                index: Box::new(index),
                span: sp(),
            },
            value,
            span: sp(),
        }),
        span: sp(),
    }
}

pub(crate) fn if_stmt(
    cond: Expr,
    then_body: Vec<Statement>,
    else_body: Option<Vec<Statement>>,
) -> Statement {
    Statement {
        kind: StatementKind::If(ast::If {
            cond,
            then_block: block(then_body),
            else_block: else_body.map(block),
            span: sp(),
        }),
        span: sp(),
    }
}

pub(crate) fn while_stmt(cond: Expr, body: Vec<Statement>) -> Statement {
    Statement {
        kind: StatementKind::While(ast::While {
            cond,
            body: block(body),
            span: sp(),
        }),
        span: sp(),
    }
}

pub(crate) fn block_stmt(statements: Vec<Statement>) -> Statement {
    Statement {
        kind: StatementKind::Block(block(statements)),
        span: sp(),
    }
}

pub(crate) fn unsafe_block(statements: Vec<Statement>) -> Statement {
    Statement {
        kind: StatementKind::SafetyBlock {
            mode: ast::SafetyMode::Unsafe,
            block: block(statements),
        },
        span: sp(),
    }
}

pub(crate) fn block(statements: Vec<Statement>) -> Block {
    Block {
        statements,
        span: sp(),
    }
}

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

pub(crate) fn struct_decl(name: &str, fields: Vec<(&str, TypeRef)>) -> Decl {
    generic_struct_decl(name, Vec::new(), fields)
}

pub(crate) fn generic_struct_decl(
    name: &str,
    type_params: Vec<&str>,
    fields: Vec<(&str, TypeRef)>,
) -> Decl {
    generic_struct_decl_full(name, type_params, fields, Vec::new(), Vec::new())
}

// --- M6: classes, interfaces, member functions ---

/// `class Name(props) : Base(args), I1, I2 { methods }`.
pub(crate) fn class_decl(
    modifier: ast::ClassModifier,
    name: &str,
    ctor: Vec<(bool, &str, TypeRef)>,
    base: Option<(&str, Vec<Expr>)>,
    interfaces: Vec<&str>,
    methods: Vec<FunctionDecl>,
) -> Decl {
    Decl::Class(ast::ClassDecl {
        annotations: vec![],
        modifier,
        name: ident(name),
        type_params: Vec::new(),
        constructor: ctor
            .into_iter()
            .map(|(mutable, name, ty)| ast::ConstructorProp {
                mutable,
                name: ident(name),
                ty,
                span: sp(),
            })
            .collect(),
        base_class: base.map(|(name, args)| (ty_named(name), args)),
        interfaces: interfaces.into_iter().map(ty_named).collect(),
        where_clause: None,
        methods,
        span: sp(),
    })
}

/// `interface I { fun m(...): T ... }`.
pub(crate) fn interface_decl(name: &str, methods: Vec<FunctionDecl>) -> Decl {
    Decl::Interface(ast::InterfaceDecl {
        annotations: vec![],
        name: ident(name),
        type_params: Vec::new(),
        parents: Vec::new(),
        where_clause: None,
        methods,
        span: sp(),
    })
}

pub(crate) fn generic_interface_decl(
    name: &str,
    type_params: Vec<(ast::Variance, &str)>,
    methods: Vec<FunctionDecl>,
) -> Decl {
    Decl::Interface(ast::InterfaceDecl {
        annotations: vec![],
        name: ident(name),
        type_params: type_params
            .into_iter()
            .map(|(variance, name)| ast::TypeParamDecl {
                name: ident(name),
                variance,
                inline_bound: None,
                span: sp(),
            })
            .collect(),
        parents: Vec::new(),
        where_clause: None,
        methods,
        span: sp(),
    })
}

/// A member function with full control over flags and body shape.
pub(crate) fn method_full(
    is_override: bool,
    is_abstract: bool,
    name: &str,
    params: Vec<(&str, TypeRef)>,
    return_ty: Option<TypeRef>,
    body: FunctionBody,
) -> FunctionDecl {
    FunctionDecl {
        annotations: Vec::new(),
        is_suspend: false,
        is_override,
        operator: None,
        modifier: if is_abstract {
            ast::MethodModifier::Abstract
        } else if is_override {
            ast::MethodModifier::Open
        } else {
            ast::MethodModifier::Final
        },
        receiver_ty: None,
        name: ident(name),
        type_params: Vec::new(),
        params: params
            .into_iter()
            .map(|(name, ty)| Param {
                name: ident(name),
                ty,
                span: sp(),
            })
            .collect(),
        return_ty,
        where_clause: None,
        body,
        span: sp(),
    }
}

/// A plain block-bodied member function (no flags).
pub(crate) fn method(
    name: &str,
    params: Vec<(&str, TypeRef)>,
    return_ty: Option<TypeRef>,
    statements: Vec<Statement>,
) -> FunctionDecl {
    method_full(
        false,
        false,
        name,
        params,
        return_ty,
        FunctionBody::Block(block(statements)),
    )
}

/// A plain expression-bodied member function (no flags).
pub(crate) fn method_expr(
    name: &str,
    params: Vec<(&str, TypeRef)>,
    return_ty: Option<TypeRef>,
    expr: Expr,
) -> FunctionDecl {
    method_full(
        false,
        false,
        name,
        params,
        return_ty,
        FunctionBody::Expr(Box::new(expr)),
    )
}

/// Set the effective modality of a member declaration built by the
/// helpers above.
pub(crate) fn with_method_modifier(
    mut method: FunctionDecl,
    modifier: ast::MethodModifier,
) -> FunctionDecl {
    method.modifier = modifier;
    method
}

/// An `override` expression-bodied member function.
pub(crate) fn override_method_expr(
    name: &str,
    params: Vec<(&str, TypeRef)>,
    return_ty: Option<TypeRef>,
    expr: Expr,
) -> FunctionDecl {
    method_full(
        true,
        false,
        name,
        params,
        return_ty,
        FunctionBody::Expr(Box::new(expr)),
    )
}

/// A bodyless member declaration (interface signatures, `abstract`).
pub(crate) fn bodyless_method(
    is_abstract: bool,
    name: &str,
    params: Vec<(&str, TypeRef)>,
    return_ty: Option<TypeRef>,
) -> FunctionDecl {
    method_full(
        false,
        is_abstract,
        name,
        params,
        return_ty,
        FunctionBody::None,
    )
}

/// A struct with member functions.
pub(crate) fn struct_decl_methods(
    name: &str,
    fields: Vec<(&str, TypeRef)>,
    methods: Vec<FunctionDecl>,
) -> Decl {
    struct_decl_full(name, fields, vec![], methods)
}

/// A struct with an interface list and member functions (spec 4.4.3).
pub(crate) fn struct_decl_full(
    name: &str,
    fields: Vec<(&str, TypeRef)>,
    interfaces: Vec<&str>,
    methods: Vec<FunctionDecl>,
) -> Decl {
    generic_struct_decl_full(name, Vec::new(), fields, interfaces, methods)
}

pub(crate) fn generic_struct_decl_full(
    name: &str,
    type_params: Vec<&str>,
    fields: Vec<(&str, TypeRef)>,
    interfaces: Vec<&str>,
    methods: Vec<FunctionDecl>,
) -> Decl {
    Decl::Struct(AstStructDecl {
        annotations: vec![],
        name: ident(name),
        type_params: type_params.into_iter().map(type_param).collect(),
        fields: fields
            .into_iter()
            .map(|(name, ty)| FieldDecl {
                name: ident(name),
                ty,
                span: sp(),
            })
            .collect(),
        interfaces: interfaces.into_iter().map(ty_named).collect(),
        where_clause: None,
        methods,
        span: sp(),
    })
}

/// An enum with member functions.
pub(crate) fn enum_decl_methods(
    name: &str,
    type_params: Vec<&str>,
    variants: Vec<VariantDecl>,
    methods: Vec<FunctionDecl>,
) -> Decl {
    enum_decl_full(name, type_params, variants, vec![], methods)
}

/// An enum with an interface list and member functions (spec 4.4.3).
pub(crate) fn enum_decl_full(
    name: &str,
    type_params: Vec<&str>,
    variants: Vec<VariantDecl>,
    interfaces: Vec<&str>,
    methods: Vec<FunctionDecl>,
) -> Decl {
    Decl::Enum(ast::EnumDecl {
        annotations: vec![],
        name: ident(name),
        type_params: type_params.into_iter().map(type_param).collect(),
        variants,
        interfaces: interfaces.into_iter().map(ty_named).collect(),
        where_clause: None,
        methods,
        span: sp(),
    })
}

/// `this`.
pub(crate) fn this_expr() -> Expr {
    Expr::This { span: sp() }
}

/// `receiver.name(args)`.
pub(crate) fn method_call(receiver: Expr, name: &str, args: Vec<Expr>) -> Expr {
    Expr::MethodCall {
        receiver: Box::new(receiver),
        name: ident(name),
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
        type_args,
        args,
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

pub(crate) fn enum_decl(name: &str, type_params: Vec<&str>, variants: Vec<VariantDecl>) -> Decl {
    Decl::Enum(ast::EnumDecl {
        annotations: vec![],
        name: ident(name),
        type_params: type_params.into_iter().map(type_param).collect(),
        variants,
        interfaces: Vec::new(),
        where_clause: None,
        methods: Vec::new(),
        span: sp(),
    })
}

pub(crate) fn variant_unit(name: &str) -> VariantDecl {
    VariantDecl {
        name: ident(name),
        kind: VariantDeclKind::Unit,
        span: sp(),
    }
}

pub(crate) fn variant_positional(name: &str, types: Vec<TypeRef>) -> VariantDecl {
    VariantDecl {
        name: ident(name),
        kind: VariantDeclKind::Positional(types),
        span: sp(),
    }
}

pub(crate) fn variant_named(name: &str, fields: Vec<(&str, TypeRef)>) -> VariantDecl {
    VariantDecl {
        name: ident(name),
        kind: VariantDeclKind::Named(
            fields
                .into_iter()
                .map(|(name, ty)| VariantFieldDecl {
                    name: ident(name),
                    ty,
                    default: None,
                    span: sp(),
                })
                .collect(),
        ),
        span: sp(),
    }
}

pub(crate) fn variant_constructor(
    name: &str,
    fields: Vec<(&str, TypeRef, Option<Expr>)>,
) -> VariantDecl {
    VariantDecl {
        name: ident(name),
        kind: VariantDeclKind::Constructor(
            fields
                .into_iter()
                .map(|(name, ty, default)| VariantFieldDecl {
                    name: ident(name),
                    ty,
                    default,
                    span: sp(),
                })
                .collect(),
        ),
        span: sp(),
    }
}

pub(crate) fn file(declarations: Vec<Decl>) -> SourceFile {
    SourceFile {
        declarations,
        span: Span::new(0, 100),
    }
}
