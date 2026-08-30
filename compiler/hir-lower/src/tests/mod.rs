//! Test builders (M4 AST) and the M1 test suite, adapted to the M4
//! AST/HIR contracts and the multi-file `lower` entry point.
//!
//! Every test compiles the user file together with a minimal
//! `scoop.core` (`core_file()`: the `Option<T>` enum, the `Throwable`
//! exception root, the M10 coroutine protocol, plus the M7 `io.scoop`
//! overloads and their backing intrinsic), mirroring the driver's
//! sysroot convention — core files first, the user file last.

mod m10;
mod m11;
mod m2;
mod m3;
mod m4;
mod m5;
mod m6;
mod m7;
mod m8;
mod m9;

use super::*;
use ast::{
    BinOp, Block, CallExpr, Decl, Expr, FieldAccess, FieldDecl, FieldSelector, FunctionBody,
    FunctionDecl, Ident, Param, SourceFile, Statement, StatementKind, StructDecl as AstStructDecl,
    TypeRef, TypeRefKind, UnOp, ValDecl, VariantDecl, VariantDeclKind, VariantFieldDecl,
};

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
        modifier: ast::MethodModifier::Final,
        name: ident(name),
        type_params: type_params.into_iter().map(ident).collect(),
        params: params
            .into_iter()
            .map(|(name, ty)| Param {
                name: ident(name),
                ty,
                span: sp(),
            })
            .collect(),
        return_ty,
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
        modifier: ast::MethodModifier::Final,
        name: ident(name),
        type_params: type_params.into_iter().map(ident).collect(),
        params: params
            .into_iter()
            .map(|(name, ty)| Param {
                name: ident(name),
                ty,
                span: sp(),
            })
            .collect(),
        return_ty,
        body: FunctionBody::Expr(Box::new(expr)),
        span: sp(),
    })
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
            value: Some(intrinsic.to_string()),
            span: sp(),
        }],
        is_suspend: false,
        is_override: false,
        modifier: ast::MethodModifier::Final,
        name: ident(name),
        type_params: type_params.into_iter().map(ident).collect(),
        params: params
            .into_iter()
            .map(|(name, ty)| Param {
                name: ident(name),
                ty,
                span: sp(),
            })
            .collect(),
        return_ty,
        body: FunctionBody::Block(block(vec![])),
        span: sp(),
    })
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
        modifier,
        name: ident(name),
        constructor: ctor
            .into_iter()
            .map(|(mutable, name, ty)| ast::ConstructorProp {
                mutable,
                name: ident(name),
                ty,
                span: sp(),
            })
            .collect(),
        base_class: base.map(|(name, args)| (ident(name), args)),
        interfaces: interfaces.into_iter().map(ty_named).collect(),
        methods,
        span: sp(),
    })
}

/// `interface I { fun m(...): T ... }`.
pub(crate) fn interface_decl(name: &str, methods: Vec<FunctionDecl>) -> Decl {
    Decl::Interface(ast::InterfaceDecl {
        name: ident(name),
        type_params: Vec::new(),
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
        name: ident(name),
        type_params: type_params
            .into_iter()
            .map(|(variance, name)| ast::TypeParamDecl {
                name: ident(name),
                variance,
                span: sp(),
            })
            .collect(),
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
        modifier: if is_abstract {
            ast::MethodModifier::Abstract
        } else if is_override {
            ast::MethodModifier::Open
        } else {
            ast::MethodModifier::Final
        },
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
        name: ident(name),
        type_params: type_params.into_iter().map(ident).collect(),
        fields: fields
            .into_iter()
            .map(|(name, ty)| FieldDecl {
                name: ident(name),
                ty,
                span: sp(),
            })
            .collect(),
        interfaces: interfaces.into_iter().map(ty_named).collect(),
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
        name: ident(name),
        type_params: type_params.into_iter().map(ident).collect(),
        variants,
        interfaces: interfaces.into_iter().map(ty_named).collect(),
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
        name: ident(name),
        type_params: type_params.into_iter().map(ident).collect(),
        variants,
        interfaces: Vec::new(),
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

/// The minimal `scoop.core` (sysroot): the `Option<T>` enum (spec 7.2),
/// the `Throwable` exception root (spec 11.7; most subclasses live in
/// `throwable_core()`), the M10 coroutine protocol, plus the M7
/// `io.scoop` final shape
/// (docs/milestone7/DESIGN.md section 2) — the single `rt_write`
/// intrinsic and `print` / `println` as ordinary `Any`-parameter
/// functions dispatching `toString()`.
pub(crate) fn core_file() -> SourceFile {
    let mut declarations = vec![
        enum_decl(
            "Option",
            vec!["T"],
            vec![
                variant_positional("Some", vec![ty_named("T")]),
                variant_unit("None"),
            ],
        ),
        class_decl(
            ast::ClassModifier::Open,
            "Throwable",
            vec![],
            None,
            vec![],
            vec![],
        ),
    ];
    declarations.extend(coroutine_core_declarations());
    declarations.extend([
        intrinsic_fun(
            "write",
            "rt_write",
            vec![("message", ty_named("String"))],
            None,
        ),
        fun_expr(
            "print",
            vec![],
            vec![("message", ty_named("Any"))],
            None,
            call(
                "write",
                vec![method_call(var("message"), "toString", vec![])],
            ),
        ),
        fun_sig(
            "println",
            vec![],
            vec![("message", ty_named("Any"))],
            None,
            vec![
                stmt(call(
                    "write",
                    vec![method_call(var("message"), "toString", vec![])],
                )),
                stmt(call("write", vec![str_lit("\n")])),
            ],
        ),
    ]);
    file(declarations)
}

fn coroutine_core_declarations() -> Vec<Decl> {
    let continuation = generic_interface_decl(
        "Continuation",
        vec![(ast::Variance::In, "T")],
        vec![
            bodyless_method(false, "resume", vec![("value", ty_named("T"))], None),
            bodyless_method(
                false,
                "resumeWithException",
                vec![("exception", ty_named("Throwable"))],
                None,
            ),
        ],
    );
    let task = generic_interface_decl(
        "SuspendTask",
        vec![(ast::Variance::Out, "T")],
        vec![with_suspend(bodyless_method(
            false,
            "run",
            vec![],
            Some(ty_named("T")),
        ))],
    );
    let registration = generic_interface_decl(
        "SuspendRegistration",
        vec![(ast::Variance::Out, "T")],
        vec![bodyless_method(
            false,
            "register",
            vec![(
                "continuation",
                ty_generic("Continuation", vec![ty_named("T")]),
            )],
            None,
        )],
    );
    let illegal_state = class_decl(
        ast::ClassModifier::Final,
        "IllegalStateException",
        vec![],
        Some(("Throwable", vec![])),
        vec![],
        vec![],
    );
    let start = intrinsic_generic_fun(
        "startCoroutine",
        "coroutine_start",
        vec!["T"],
        vec![
            ("task", ty_generic("SuspendTask", vec![ty_named("T")])),
            (
                "completion",
                ty_generic("Continuation", vec![ty_named("T")]),
            ),
        ],
        None,
    );
    let Decl::Function(mut suspend) = intrinsic_generic_fun(
        "suspendCoroutine",
        "coroutine_suspend",
        vec!["T"],
        vec![(
            "registration",
            ty_generic("SuspendRegistration", vec![ty_named("T")]),
        )],
        Some(ty_named("T")),
    ) else {
        unreachable!("intrinsic_generic_fun always builds a function declaration")
    };
    suspend.is_suspend = true;

    vec![
        continuation,
        task,
        registration,
        illegal_state,
        start,
        Decl::Function(suspend),
    ]
}

/// Lower a user file together with the minimal `scoop.core`, mirroring
/// the driver's sysroot convention (core files first, user file last).
pub(crate) fn lower_user(user: SourceFile) -> Result<hir::Module, Vec<Diagnostic>> {
    lower(&[core_file(), user])
}

// --- M8: exceptions ---

/// `throw expr` (M8).
pub(crate) fn throw_stmt(value: Expr) -> Statement {
    Statement {
        kind: StatementKind::Throw(value),
        span: sp(),
    }
}

/// `try { body } catch... finally...` (M8).
pub(crate) fn try_stmt(
    body: Vec<Statement>,
    catches: Vec<ast::CatchClause>,
    finally_body: Option<Vec<Statement>>,
) -> Statement {
    Statement {
        kind: StatementKind::Try(ast::Try {
            body: block(body),
            catches,
            finally_body: finally_body.map(block),
            span: sp(),
        }),
        span: sp(),
    }
}

/// `catch (name: T) { body }`.
pub(crate) fn catch_clause(name: &str, ty: TypeRef, body: Vec<Statement>) -> ast::CatchClause {
    ast::CatchClause {
        name: ident(name),
        ty,
        body: block(body),
        span: sp(),
    }
}

/// `catch (name: T) { body }` with an explicit clause span (diagnostic
/// position assertions).
pub(crate) fn catch_clause_at(
    name: &str,
    ty: TypeRef,
    body: Vec<Statement>,
    span: Span,
) -> ast::CatchClause {
    ast::CatchClause {
        name: ident(name),
        ty,
        body: block(body),
        span,
    }
}

/// The exception subclasses of `scoop.core`
/// (sysroot/lib/scoop.core/src/throwable.scoop) as a second core file
/// for tests that exercise `throw` / `catch`; the `Throwable` root
/// lives in `core_file()`.
pub(crate) fn throwable_core() -> SourceFile {
    let subclass = |name: &str, message: &str| {
        class_decl(
            ast::ClassModifier::Final,
            name,
            vec![],
            Some(("Exception", vec![some(str_lit(message))])),
            vec![],
            vec![],
        )
    };
    file(vec![
        class_decl(
            ast::ClassModifier::Open,
            "Exception",
            vec![(false, "message", ty_nullable(ty_named("String")))],
            Some(("Throwable", vec![])),
            vec![],
            vec![],
        ),
        subclass("UnwrapException", "unwrap on None"),
        subclass("ClassCastException", "invalid cast"),
        subclass("ArithmeticException", "arithmetic error"),
        subclass("IndexOutOfBoundsException", "array index out of bounds"),
    ])
}

/// Lower a user file with the full core exception hierarchy available
/// (M8 tests).
pub(crate) fn lower_user_with_exceptions(user: SourceFile) -> Result<hir::Module, Vec<Diagnostic>> {
    lower(&[core_file(), throwable_core(), user])
}

/// The GC facilities of `scoop.core`
/// (sysroot/lib/scoop.core/src/gc.scoop, M9) as another core file:
/// the `PinHandle` / `GcHandle` structs, the four generic GC
/// intrinsics and the test-only `gcCollect` / `gcStats` hooks. The
/// surface declarations spell the handle types `PinHandle<T>`.
pub(crate) fn gc_core_file() -> SourceFile {
    let handle = |name: &str| ty_generic(name, vec![ty_named("T")]);
    file(vec![
        generic_struct_decl("PinHandle", vec!["T"], vec![("raw", ty_named("UInt"))]),
        generic_struct_decl("GcHandle", vec!["T"], vec![("raw", ty_named("UInt"))]),
        intrinsic_generic_fun(
            "pin",
            "rt_pin",
            vec!["T"],
            vec![("v", ty_named("T"))],
            Some(handle("PinHandle")),
        ),
        intrinsic_generic_fun(
            "unpin",
            "rt_unpin",
            vec!["T"],
            vec![("h", handle("PinHandle"))],
            Some(ty_named("T")),
        ),
        intrinsic_generic_fun(
            "getGcHandle",
            "rt_get_handle",
            vec!["T"],
            vec![("v", ty_named("T"))],
            Some(handle("GcHandle")),
        ),
        intrinsic_generic_fun(
            "releaseGcHandle",
            "rt_release_handle",
            vec!["T"],
            vec![("h", handle("GcHandle"))],
            Some(ty_named("T")),
        ),
        intrinsic_fun("gcCollect", "rt_gc_collect", vec![], None),
        intrinsic_fun("gcStats", "rt_gc_stats", vec![], Some(ty_named("UInt"))),
    ])
}

/// Lower a user file with the core GC facilities available (M9 tests).
pub(crate) fn lower_user_with_gc(user: SourceFile) -> Result<hir::Module, Vec<Diagnostic>> {
    lower(&[core_file(), gc_core_file(), user])
}

/// `main` calls `println("hello, world")` then `helper()`, which
/// calls `print("!")` — the M1 hello world shape (milestone1
/// DESIGN.md 1).
fn hello_world() -> SourceFile {
    file(vec![
        fun(
            "main",
            vec![
                stmt(call("println", vec![str_lit("hello, world")])),
                stmt(call("helper", vec![])),
            ],
        ),
        fun("helper", vec![stmt(call("print", vec![str_lit("!")]))]),
    ])
}

#[test]
fn lowers_hello_world() {
    let module = lower_user(hello_world()).expect("hello world must lower");

    // Well-known types are allocated first, in a fixed order.
    assert_eq!(module.types[module.unit], Type::Unit);
    assert_eq!(module.types[module.int], Type::Int);
    assert_eq!(module.types[module.boolean], Type::Boolean);
    assert_eq!(module.types[module.string], Type::String);
    // `Option` comes from the core library.
    assert_eq!(module.enums[module.option_enum].name, "Option");

    // Entry point is `main`.
    assert_eq!(module.functions[module.entry].name, "main");

    // Golden dump locks the output structure.
    let expected = "\
Module
  enum Option<T>
    Some(_1: T0)
    None()
  open class Throwable()
  class IllegalStateException()
  interface Continuation<in T>
    fun resume(value: T0): Unit
    fun resumeWithException(exception: Throwable): Unit
  interface SuspendTask<out T>
    suspend fun run(): T0
  interface SuspendRegistration<out T>
    fun register(continuation: Continuation<T0>): Unit
  fun startCoroutine<T>(): Unit <intrinsic coroutine_start>
  suspend fun suspendCoroutine<T>(): T0 <intrinsic coroutine_suspend>
  fun write(): Unit <intrinsic rt_write>
  fun print(message: Any): Unit
    Call write : Unit
      MethodCall Any.toString : String
        Local message : Any
    return
  fun println(message: Any): Unit
    Call write : Unit
      MethodCall Any.toString : String
        Local message : Any
    Call write : Unit
      StringLiteral \"\\n\" : String
  fun main(): Unit
    Call println : Unit
      StringLiteral \"hello, world\" : Any
    Call helper : Unit
  fun helper(): Unit
    Call print : Unit
      StringLiteral \"!\" : Any
  entry main
";
    assert_eq!(hir::dump(&module), expected);
}

#[test]
fn duplicate_function_is_an_error() {
    let file = file(vec![fun("main", vec![]), fun("main", vec![])]);
    let errors = lower_user(file).expect_err("duplicate `main` must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "function `main` is already declared with the same signature"
    );
    // The duplicate is in the user file (index 1; core is index 0).
    assert_eq!(errors[0].file, 1);
}

#[test]
fn redeclaring_a_core_function_is_an_error() {
    // The user file's `print(Any)` duplicates the core declaration
    // exactly (overloads with different signatures would be legal).
    let file = file(vec![
        fun("main", vec![]),
        fun_expr(
            "print",
            vec![],
            vec![("message", ty_named("Any"))],
            None,
            call(
                "write",
                vec![method_call(var("message"), "toString", vec![])],
            ),
        ),
    ]);
    let errors = lower_user(file).expect_err("redeclaring `print` must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "function `print` is already declared with the same signature"
    );
}

#[test]
fn unknown_function_is_an_error_with_callee_span() {
    let callee_span = Span::new(10, 15);
    let file = file(vec![fun(
        "main",
        vec![stmt(call_at("hello", vec![], callee_span))],
    )]);
    let errors = lower_user(file).expect_err("unknown callee must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "unknown function `hello`");
    assert_eq!(errors[0].span, Some(callee_span));
}

#[test]
fn print_requires_exactly_one_argument() {
    for args in [vec![], vec![str_lit("a"), str_lit("b")]] {
        let supplied = args.len();
        let file = file(vec![fun("main", vec![stmt(call("print", args))])]);
        let errors = lower_user(file).expect_err("wrong arity must fail");
        assert_eq!(errors.len(), 1);
        assert_eq!(
            errors[0].message,
            format!("function `print` takes exactly 1 argument, but {supplied} were supplied")
        );
    }
}

#[test]
fn user_function_arity_is_an_error() {
    let file = file(vec![
        fun("main", vec![stmt(call("helper", vec![str_lit("x")]))]),
        fun("helper", vec![]),
    ]);
    let errors = lower_user(file).expect_err("argument to `helper` must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "function `helper` takes exactly 0 arguments, but 1 were supplied"
    );
}

#[test]
fn missing_main_is_an_error_with_file_span() {
    let file = file(vec![fun("helper", vec![])]);
    let errors = lower_user(file.clone()).expect_err("missing `main` must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "missing entry point: declare `fun main()`"
    );
    assert_eq!(errors[0].span, Some(file.span));
}

#[test]
fn bare_literal_statement_is_an_error() {
    let file = file(vec![fun("main", vec![stmt(str_lit("dangling"))])]);
    let errors = lower_user(file).expect_err("literal statement must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "statement must be a function call");
}

#[test]
fn collects_multiple_diagnostics() {
    let file = file(vec![fun(
        "main",
        vec![
            stmt(call("missing_one", vec![])),
            stmt(call("missing_two", vec![])),
        ],
    )]);
    let errors = lower_user(file).expect_err("unknown callees must fail");
    assert_eq!(errors.len(), 2);
    assert_eq!(errors[0].message, "unknown function `missing_one`");
    assert_eq!(errors[1].message, "unknown function `missing_two`");
}
