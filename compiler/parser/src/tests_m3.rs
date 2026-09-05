//! Unit tests for the M3 syntax added on top of M2: generic type
//! parameters, parameter lists, return type annotations, expression
//! bodies, `return`, nullable type annotations (`T?`), and the `?.` /
//! `!!` / `?:` operators.

use scoop_ast::{
    Decl, Expr, FieldSelector, FunctionBody, Navigation, Span, StatementKind, TypeRefKind, UnOp,
};

use crate::tests::{block_body, err, ok, only_function};
use crate::tests_m2::{init_expr, stmt_dump};

// --- function signatures ----------------------------------------------------

#[test]
fn function_with_params_and_return_type() {
    let file = ok("fun add(x: Int, y: Int): Int {\n    return x + y\n}\n");
    let function = only_function(&file);
    assert_eq!(function.name.text, "add");
    assert_eq!(function.name.span, Span::new(4, 7));
    assert_eq!(function.span, Span::new(0, 49));
    assert!(function.type_params.is_empty());
    assert_eq!(function.params.len(), 2);
    assert_eq!(function.params[0].name.text, "x");
    assert_eq!(function.params[0].span, Span::new(8, 14));
    assert!(matches!(&function.params[0].ty.kind, TypeRefKind::Named(name) if name.text == "Int"));
    assert_eq!(function.params[1].name.text, "y");
    assert_eq!(function.params[1].span, Span::new(16, 22));
    let return_ty = function.return_ty.as_ref().expect("return type present");
    assert!(matches!(&return_ty.kind, TypeRefKind::Named(name) if name.text == "Int"));
    assert_eq!(return_ty.span, Span::new(25, 28));
    assert_eq!(
        scoop_ast::dump(&file),
        "SourceFile\n  fun add(x: Int, y: Int): Int\n    return\n      Binary Add\n        Var x\n        Var y\n"
    );
}

#[test]
fn generic_function_single_type_param() {
    let file = ok("fun <T> identity(x: T): T = x");
    let function = only_function(&file);
    assert_eq!(function.type_params.len(), 1);
    assert_eq!(function.type_params[0].name.text, "T");
    assert_eq!(function.type_params[0].span, Span::new(5, 6));
    assert_eq!(function.span, Span::new(0, 29));
    let FunctionBody::Expr(body) = &function.body else {
        panic!("expected an expression body");
    };
    assert!(matches!(**body, Expr::Var(ref ident) if ident.text == "x"));
    assert_eq!(
        scoop_ast::dump(&file),
        "SourceFile\n  fun identity<T>(x: T): T\n    =\n      Var x\n"
    );
}

#[test]
fn generic_function_multiple_type_params() {
    let file = ok("fun <T, U> first(a: T, b: U): T = a");
    let function = only_function(&file);
    let names: Vec<&str> = function
        .type_params
        .iter()
        .map(|param| param.name.text.as_str())
        .collect();
    assert_eq!(names, ["T", "U"]);
    assert_eq!(
        scoop_ast::dump(&file),
        "SourceFile\n  fun first<T, U>(a: T, b: U): T\n    =\n      Var a\n"
    );
}

#[test]
fn empty_type_parameter_list_is_an_error() {
    let (span, message) = err("fun <> f() {}");
    assert_eq!(span, Span::new(5, 6));
    assert_eq!(message, "expected type parameter name, found `>`");
}

#[test]
fn parameter_type_annotation_is_mandatory() {
    let (span, message) = err("fun f(x) {}");
    assert_eq!(span, Span::new(7, 8));
    assert_eq!(message, "expected `:`, found `)`");
}

#[test]
fn trailing_comma_in_parameter_list_is_an_error() {
    let (span, message) = err("fun f(x: Int,) {}");
    assert_eq!(span, Span::new(13, 14));
    assert_eq!(message, "expected parameter name, found `)`");
}

// --- expression bodies --------------------------------------------------------

#[test]
fn expression_body_without_return_type() {
    let file = ok("fun g() = 42");
    let function = only_function(&file);
    assert!(function.return_ty.is_none());
    assert_eq!(function.span, Span::new(0, 12));
    let FunctionBody::Expr(body) = &function.body else {
        panic!("expected an expression body");
    };
    assert!(matches!(
        **body,
        Expr::IntLiteral(literal)
            if literal.magnitude == 42 && literal.span == Span::new(10, 12)
    ));
}

#[test]
fn expression_body_followed_by_another_declaration() {
    let file = ok("fun f(x: Int): Int = x + 1\nfun main() {}\n");
    assert_eq!(file.declarations.len(), 2);
    assert!(matches!(file.declarations[0], Decl::Function(_)));
    assert!(matches!(file.declarations[1], Decl::Function(_)));
}

// --- return statements --------------------------------------------------------

#[test]
fn bare_return() {
    let file = ok("fun f() {\n    return\n}\n");
    let stmt = &block_body(only_function(&file)).statements[0];
    assert_eq!(stmt.span, Span::new(14, 20));
    let StatementKind::Return { value } = &stmt.kind else {
        panic!("expected a return statement");
    };
    assert!(value.is_none());
}

#[test]
fn return_value_ends_at_the_newline() {
    let file = ok("fun f() {\n    return\n    x\n}\n");
    let statements = &block_body(only_function(&file)).statements;
    assert_eq!(statements.len(), 2);
    let StatementKind::Return { value } = &statements[0].kind else {
        panic!("expected a return statement");
    };
    assert!(value.is_none());
    let StatementKind::Expr(Expr::Var(ident)) = &statements[1].kind else {
        panic!("expected an expression statement");
    };
    assert_eq!(ident.text, "x");
}

// --- nullable type annotations ------------------------------------------------

#[test]
fn nullable_type_annotations() {
    let file = ok("fun f(a: Int?, b: (Int, String)?, c: Int??) {}");
    let function = only_function(&file);
    let [a, b, c] = &function.params[..] else {
        panic!("expected three parameters");
    };
    let TypeRefKind::Nullable(inner) = &a.ty.kind else {
        panic!("expected a nullable type");
    };
    assert!(matches!(&inner.kind, TypeRefKind::Named(name) if name.text == "Int"));
    assert_eq!(a.ty.span, Span::new(9, 13));
    let TypeRefKind::Nullable(inner) = &b.ty.kind else {
        panic!("expected a nullable type");
    };
    assert!(
        matches!(&inner.kind, TypeRefKind::Tuple(elements) if elements.len() == 2),
        "nullable tuple type"
    );
    // `Int??` is `Option<Option<Int>>`; the two layers do not collapse.
    let TypeRefKind::Nullable(outer) = &c.ty.kind else {
        panic!("expected a nullable type");
    };
    let TypeRefKind::Nullable(inner) = &outer.kind else {
        panic!("expected a nested nullable type");
    };
    assert!(matches!(&inner.kind, TypeRefKind::Named(name) if name.text == "Int"));
}

#[test]
fn nullable_type_parameter_annotation() {
    let file = ok("fun <T> unwrapOr(o: T?, fallback: T): T = o ?: fallback");
    let function = only_function(&file);
    let TypeRefKind::Nullable(inner) = &function.params[0].ty.kind else {
        panic!("expected a nullable type");
    };
    assert!(matches!(&inner.kind, TypeRefKind::Named(name) if name.text == "T"));
}

#[test]
fn nullable_struct_field() {
    let file = ok("struct S(val p: Int?)");
    let Decl::Struct(decl) = &file.declarations[0] else {
        panic!("expected a struct declaration");
    };
    assert!(matches!(decl.fields[0].ty.kind, TypeRefKind::Nullable(_)));
    assert_eq!(
        scoop_ast::dump(&file),
        "SourceFile\n  struct S\n    field p: Int?\n"
    );
}

// --- `?.` ----------------------------------------------------------------------

#[test]
fn safe_field_access() {
    let Expr::FieldAccess(access) = init_expr("p?.x") else {
        panic!("expected a field access");
    };
    assert_eq!(access.navigation, Navigation::Safe);
    assert_eq!(access.span, Span::new(25, 29));
    let FieldSelector::Name(name) = &access.selector else {
        panic!("expected a named selector");
    };
    assert_eq!(name.text, "x");
    assert!(matches!(*access.receiver, Expr::Var(ref ident) if ident.text == "p"));
}

#[test]
fn safe_field_access_chains() {
    let Expr::FieldAccess(outer) = init_expr("a?.b?.c") else {
        panic!("expected a field access");
    };
    assert_eq!(outer.navigation, Navigation::Safe);
    let FieldSelector::Name(name) = &outer.selector else {
        panic!("expected a named selector");
    };
    assert_eq!(name.text, "c");
    let Expr::FieldAccess(inner) = &*outer.receiver else {
        panic!("expected a field access receiver");
    };
    assert_eq!(inner.navigation, Navigation::Safe);
}

#[test]
fn safe_field_access_allows_whitespace_before_the_dot() {
    // `?.` is one token; `a ?. x` still lexes the operator greedily.
    let Expr::FieldAccess(access) = init_expr("a?. x") else {
        panic!("expected a field access");
    };
    assert_eq!(access.navigation, Navigation::Safe);
}

#[test]
fn tuple_index_after_safe_dot_is_rejected() {
    let (span, message) = err("fun main() { val x = a?._1 }");
    assert_eq!(span, Span::new(24, 26));
    assert_eq!(message, "expected field name, found `_1`");
}

// --- `!!` ----------------------------------------------------------------------

#[test]
fn null_assert() {
    let Expr::NullAssert { operand, span } = init_expr("a!!") else {
        panic!("expected a null assertion");
    };
    assert!(matches!(*operand, Expr::Var(ref ident) if ident.text == "a"));
    assert_eq!(span, Span::new(25, 28));
}

#[test]
fn null_assert_allows_whitespace_around_but_not_between_the_bangs() {
    assert!(matches!(init_expr("a !!"), Expr::NullAssert { .. }));
    assert!(matches!(init_expr("a!! "), Expr::NullAssert { .. }));
    let (span, message) = err("fun main() { val x = a ! ! }");
    assert_eq!(span, Span::new(23, 24));
    assert_eq!(
        message,
        "expected `;` or newline after statement, found `!`"
    );
}

#[test]
fn postfix_operators_share_one_tier_and_chain_left_to_right() {
    // `a?.x!!` unwraps the safe access.
    assert_eq!(
        stmt_dump("val x = a?.x!!"),
        "val x\n  NullAssert\n    FieldAccess ?x\n      Var a\n"
    );
    // `a!!.x` accesses a field of the unwrapped value.
    assert_eq!(
        stmt_dump("val x = a!!.x"),
        "val x\n  FieldAccess x\n    NullAssert\n      Var a\n"
    );
}

#[test]
fn null_assert_binds_tighter_than_unary() {
    let Expr::Unary {
        op: UnOp::Neg,
        operand,
        ..
    } = init_expr("-a!!")
    else {
        panic!("expected a unary expression");
    };
    assert!(matches!(*operand, Expr::NullAssert { .. }));
    let Expr::Unary {
        op: UnOp::Not,
        operand,
        ..
    } = init_expr("!a!!")
    else {
        panic!("expected a unary expression");
    };
    assert!(matches!(*operand, Expr::NullAssert { .. }));
    // The M1 double negation keeps parsing as two prefix `!`.
    let Expr::Unary {
        op: UnOp::Not,
        operand,
        ..
    } = init_expr("!!flag")
    else {
        panic!("expected a unary expression");
    };
    assert!(matches!(*operand, Expr::Unary { op: UnOp::Not, .. }));
}

// --- `?:` ----------------------------------------------------------------------

#[test]
fn elvis_is_right_associative() {
    assert_eq!(
        stmt_dump("val x = a ?: b ?: c"),
        "val x\n  Elvis\n    Var a\n    Elvis\n      Var b\n      Var c\n"
    );
}

#[test]
fn elvis_binds_tighter_than_boolean_operators() {
    assert_eq!(
        stmt_dump("val x = a || b ?: c"),
        "val x\n  Binary Or\n    Var a\n    Elvis\n      Var b\n      Var c\n"
    );
    assert_eq!(
        stmt_dump("val x = a ?: b || c"),
        "val x\n  Binary Or\n    Elvis\n      Var a\n      Var b\n    Var c\n"
    );
}

#[test]
fn elvis_combines_with_safe_field_access() {
    assert_eq!(
        stmt_dump("val x = p?.x ?: 0"),
        "val x\n  Elvis\n    FieldAccess ?x\n      Var p\n    IntLiteral 0\n"
    );
}

// --- Option construction is ordinary syntax -------------------------------------

#[test]
fn some_and_none_are_plain_call_and_var() {
    let Expr::Call(call) = init_expr("Some(41)") else {
        panic!("expected a call");
    };
    assert_eq!(call.callee.text, "Some");
    let Expr::Var(ident) = init_expr("None") else {
        panic!("expected a variable");
    };
    assert_eq!(ident.text, "None");
}

// --- the M3 design example end to end -------------------------------------------

#[test]
fn design_example() {
    // `Option<T>` cannot be written in annotations yet (type references
    // have no type arguments); the design example's `Option<T>` parameter
    // is written with `T?` here.
    let source = "fun <T> identity(x: T): T = x\n\
\n\
fun <T> unwrapOr(o: T?, fallback: T): T {\n\
\u{20}   return o ?: fallback\n\
}\n\
\n\
fun describe(p: Point?): String {\n\
\u{20}   val x = p?.x\n\
\u{20}   if (x == None) {\n\
\u{20}       return \"empty\"\n\
\u{20}   }\n\
\u{20}   return \"x=\" + \"!\"\n\
}\n\
\n\
fun main() {\n\
\u{20}   val a: Int? = Some(41)\n\
\u{20}   val b = a ?: 0\n\
\u{20}   println(identity(b + 1))\n\
\u{20}   println(identity(\"hi\"))\n\
\u{20}   val p: Point? = Some(Point(1, 2))\n\
\u{20}   println(p?.x ?: 0)\n\
\u{20}   val q: Point? = None\n\
\u{20}   println(q?.x ?: 0)\n\
\u{20}   println(unwrapOr(a, 0)!! + 1)\n\
}\n";
    let file = ok(source);
    assert_eq!(file.declarations.len(), 4);
    let Decl::Function(identity) = &file.declarations[0] else {
        panic!("expected a function declaration");
    };
    assert_eq!(identity.type_params.len(), 1);
    assert!(matches!(identity.body, FunctionBody::Expr(_)));
}
