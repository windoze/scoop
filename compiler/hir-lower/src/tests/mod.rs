//! Test builders (M4 AST) and the M1 test suite, adapted to the M4
//! AST/HIR contracts and the multi-file `lower` entry point.
//!
//! Every test compiles the user file together with a minimal
//! `scoop.core` (`core_file()`: the `Option<T>` enum plus the
//! `rt_print` / `rt_println` intrinsics), mirroring the driver's
//! sysroot convention — core files first, the user file last.

mod m2;
mod m3;
mod m4;
mod m5;

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

/// `@Intrinsic("...") fun name(params)` (core library only).
pub(crate) fn intrinsic_fun(name: &str, intrinsic: &str, params: Vec<(&str, TypeRef)>) -> Decl {
    Decl::Function(FunctionDecl {
        annotations: vec![ast::Annotation {
            name: ident("Intrinsic"),
            value: Some(intrinsic.to_string()),
            span: sp(),
        }],
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
        return_ty: None,
        body: FunctionBody::Block(block(vec![])),
        span: sp(),
    })
}

pub(crate) fn struct_decl(name: &str, fields: Vec<(&str, TypeRef)>) -> Decl {
    Decl::Struct(AstStructDecl {
        name: ident(name),
        fields: fields
            .into_iter()
            .map(|(name, ty)| FieldDecl {
                name: ident(name),
                ty,
                span: sp(),
            })
            .collect(),
        span: sp(),
    })
}

pub(crate) fn enum_decl(name: &str, type_params: Vec<&str>, variants: Vec<VariantDecl>) -> Decl {
    Decl::Enum(ast::EnumDecl {
        name: ident(name),
        type_params: type_params.into_iter().map(ident).collect(),
        variants,
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

/// The minimal `scoop.core` (sysroot): the `Option<T>` enum (spec 7.2)
/// and the `rt_print` / `rt_println` intrinsics (milestone4 DESIGN.md
/// 1.3).
pub(crate) fn core_file() -> SourceFile {
    file(vec![
        enum_decl(
            "Option",
            vec!["T"],
            vec![
                variant_positional("Some", vec![ty_named("T")]),
                variant_unit("None"),
            ],
        ),
        intrinsic_fun("print", "rt_print", vec![("message", ty_named("String"))]),
        intrinsic_fun(
            "println",
            "rt_println",
            vec![("message", ty_named("String"))],
        ),
    ])
}

/// Lower a user file together with the minimal `scoop.core`, mirroring
/// the driver's sysroot convention (core files first, user file last).
pub(crate) fn lower_user(user: SourceFile) -> Result<hir::Module, Vec<Diagnostic>> {
    lower(&[core_file(), user])
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
  fun print(): Unit <intrinsic rt_print>
  fun println(): Unit <intrinsic rt_println>
  fun main(): Unit
    Call println : Unit
      StringLiteral \"hello, world\" : String
    Call helper : Unit
  fun helper(): Unit
    Call print : Unit
      StringLiteral \"!\" : String
  entry main
";
    assert_eq!(hir::dump(&module), expected);
}

#[test]
fn duplicate_function_is_an_error() {
    let file = file(vec![fun("main", vec![]), fun("main", vec![])]);
    let errors = lower_user(file).expect_err("duplicate `main` must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "duplicate function `main`");
    // The duplicate is in the user file (index 1; core is index 0).
    assert_eq!(errors[0].file, 1);
}

#[test]
fn redeclaring_a_core_function_is_an_error() {
    let file = file(vec![fun("main", vec![]), fun("print", vec![])]);
    let errors = lower_user(file).expect_err("redeclaring `print` must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "duplicate function `print`");
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
            format!("`print` takes exactly 1 argument, but {supplied} were supplied")
        );
    }
}

#[test]
fn print_argument_must_be_printable() {
    // `helper()` has type `Unit`, which is not printable.
    let file = file(vec![
        fun(
            "main",
            vec![stmt(call("println", vec![call("helper", vec![])]))],
        ),
        fun("helper", vec![]),
    ]);
    let errors = lower_user(file).expect_err("non-printable argument must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "argument of `println` must be String, Int or Boolean, found Unit"
    );
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
