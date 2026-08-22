//! Test builders (M3 AST) and the M1 test suite, adapted to the M3
//! AST/HIR contracts.

mod m2;
mod m3;

use super::*;
use ast::{
    BinOp, Block, CallExpr, Decl, Expr, FieldAccess, FieldDecl, FieldSelector, FunctionBody,
    FunctionDecl, Ident, Param, SourceFile, Statement, StatementKind, StructDecl as AstStructDecl,
    TypeRef, TypeRefKind, UnOp, ValDecl,
};

pub(crate) fn sp() -> Span {
    Span::new(0, 0)
}

pub(crate) fn ident(text: &str) -> Ident {
    Ident {
        text: text.to_string(),
        span: sp(),
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
/// known to be a struct).
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

/// The `None` literal (parsed as an identifier, recognized by HIR).
pub(crate) fn none() -> Expr {
    var("None")
}

/// The builtin `Some(x)` constructor (parsed as a plain call).
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
            name: ident(name),
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
            name: ident(name),
            ty,
            init,
            span: sp(),
        }),
        span: sp(),
    }
}

pub(crate) fn assign(target: &str, value: Expr) -> Statement {
    Statement {
        kind: StatementKind::Assign(ast::Assign {
            target: ident(target),
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

pub(crate) fn file(declarations: Vec<Decl>) -> SourceFile {
    SourceFile {
        declarations,
        span: Span::new(0, 100),
    }
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
    let module = lower(&hello_world()).expect("hello world must lower");

    // Well-known types are allocated first, in a fixed order.
    assert_eq!(module.types[module.unit], Type::Unit);
    assert_eq!(module.types[module.int], Type::Int);
    assert_eq!(module.types[module.boolean], Type::Boolean);
    assert_eq!(module.types[module.string], Type::String);
    assert!(matches!(
        module.functions[module.print].kind,
        FunctionKind::Builtin(Builtin::Print)
    ));
    assert!(matches!(
        module.functions[module.println].kind,
        FunctionKind::Builtin(Builtin::Println)
    ));

    // Entry point is `main`.
    assert_eq!(module.functions[module.entry].name, "main");

    // Golden dump locks the output structure.
    let expected = "\
Module
  fun print(): Unit <builtin Print>
  fun println(): Unit <builtin Println>
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
    let errors = lower(&file).expect_err("duplicate `main` must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "duplicate function `main`");
}

#[test]
fn redeclaring_a_builtin_is_an_error() {
    let file = file(vec![fun("main", vec![]), fun("print", vec![])]);
    let errors = lower(&file).expect_err("redeclaring `print` must fail");
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
    let errors = lower(&file).expect_err("unknown callee must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "unknown function `hello`");
    assert_eq!(errors[0].span, Some(callee_span));
}

#[test]
fn print_requires_exactly_one_argument() {
    for args in [vec![], vec![str_lit("a"), str_lit("b")]] {
        let supplied = args.len();
        let file = file(vec![fun("main", vec![stmt(call("print", args))])]);
        let errors = lower(&file).expect_err("wrong arity must fail");
        assert_eq!(errors.len(), 1);
        assert_eq!(
            errors[0].message,
            format!("`print` takes exactly 1 argument, but {supplied} were supplied")
        );
    }
}

#[test]
fn print_argument_must_be_printable() {
    // `helper()` has type `Unit`, which is not printable in M2.
    let file = file(vec![
        fun(
            "main",
            vec![stmt(call("println", vec![call("helper", vec![])]))],
        ),
        fun("helper", vec![]),
    ]);
    let errors = lower(&file).expect_err("non-printable argument must fail");
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
    let errors = lower(&file).expect_err("argument to `helper` must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "function `helper` takes exactly 0 arguments, but 1 were supplied"
    );
}

#[test]
fn missing_main_is_an_error_with_file_span() {
    let file = file(vec![fun("helper", vec![])]);
    let errors = lower(&file).expect_err("missing `main` must fail");
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
    let errors = lower(&file).expect_err("literal statement must fail");
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
    let errors = lower(&file).expect_err("unknown callees must fail");
    assert_eq!(errors.len(), 2);
    assert_eq!(errors[0].message, "unknown function `missing_one`");
    assert_eq!(errors[1].message, "unknown function `missing_two`");
}
