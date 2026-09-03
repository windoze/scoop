use scoop_ast::{Expr, Span};

use crate::tests::err;
use crate::tests_m2::{init_expr, stmt_dump};

// --- this / method calls ---------------------------------------------------------

#[test]
fn this_expression() {
    // `fun main() {\n    val x = this\n}\n` — `this` is at offset 25.
    let expr = init_expr("this");
    let Expr::This { span } = &expr else {
        panic!("expected a this expression");
    };
    assert_eq!(*span, Span::new(25, 29));
}

#[test]
fn method_call() {
    // `fun main() {\n    val x = p.moveTo(1, 2)\n}\n` — `p` is at offset 25.
    let expr = init_expr("p.moveTo(1, 2)");
    let Expr::MethodCall {
        receiver,
        name,
        args,
        span,
        ..
    } = &expr
    else {
        panic!("expected a method call");
    };
    assert_eq!(*span, Span::new(25, 39));
    assert_eq!(name.text, "moveTo");
    assert_eq!(name.span, Span::new(27, 33));
    assert!(matches!(receiver.as_ref(), Expr::Var(name) if name.text == "p"));
    assert_eq!(args.len(), 2);
    assert_eq!(
        stmt_dump("p.moveTo(1, 2)"),
        "MethodCall moveTo\n  Var p\n  IntLiteral 1\n  IntLiteral 2\n"
    );
}

#[test]
fn method_call_without_args() {
    let expr = init_expr("p.describe()");
    let Expr::MethodCall { name, args, .. } = &expr else {
        panic!("expected a method call");
    };
    assert_eq!(name.text, "describe");
    assert!(args.is_empty());
}

#[test]
fn method_call_on_field_access_chains() {
    let expr = init_expr("a.b.c(1)");
    let Expr::MethodCall { receiver, name, .. } = &expr else {
        panic!("expected a method call");
    };
    assert_eq!(name.text, "c");
    assert!(matches!(receiver.as_ref(), Expr::FieldAccess(_)));
}

#[test]
fn dot_name_without_parens_stays_a_field_access() {
    // `.name` without `(` is a field access, not a method call.
    let expr = init_expr("p.x");
    assert!(matches!(expr, Expr::FieldAccess(_)));
}

#[test]
fn method_call_after_safe_navigation_is_preserved() {
    let expr = init_expr("a?.m()");
    let Expr::MethodCall {
        navigation, name, ..
    } = expr
    else {
        panic!("expected a method call");
    };
    assert_eq!(navigation, scoop_ast::Navigation::Safe);
    assert_eq!(name.text, "m");
}

#[test]
fn super_call_is_a_dedicated_expression() {
    let expr = init_expr("super.foo()");
    assert!(matches!(
        expr,
        Expr::SuperMethodCall { name, .. } if name.text == "foo"
    ));
}

#[test]
fn bare_super_not_supported() {
    let (_, message) = err("fun main() { super }");
    assert_eq!(
        message,
        "`super` is only valid as the receiver of a direct base method call"
    );
}
