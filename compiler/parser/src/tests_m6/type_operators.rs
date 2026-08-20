use scoop_ast::{Expr, Navigation, Span, TypeRefKind};

use crate::tests::err;
use crate::tests_m2::{init_expr, stmt_dump};

// --- is / !is / as / as? ---------------------------------------------------------

#[test]
fn is_expression() {
    // `fun main() {\n    val x = a is Int\n}\n` — `a` is at offset 25.
    let expr = init_expr("a is Int");
    let Expr::Is {
        operand,
        ty,
        negated,
        span,
    } = &expr
    else {
        panic!("expected an is expression");
    };
    assert_eq!(*span, Span::new(25, 33));
    assert!(!negated);
    assert!(matches!(operand.as_ref(), Expr::Var(name) if name.text == "a"));
    assert!(matches!(&ty.kind, TypeRefKind::Named(name) if name.text == "Int"));
}

#[test]
fn bang_is_expression() {
    let expr = init_expr("a !is Int");
    let Expr::Is { negated, span, .. } = &expr else {
        panic!("expected an is expression");
    };
    assert!(negated);
    assert_eq!(*span, Span::new(25, 34));
}

#[test]
fn is_binds_tighter_than_equality() {
    // `a is Int == true` is `(a is Int) == true` — the type operators sit
    // at comparison precedence, above `==`.
    let expr = init_expr("a is Int == true");
    let Expr::Binary { op, lhs, rhs, .. } = &expr else {
        panic!("expected a binary expression");
    };
    assert_eq!(*op, scoop_ast::BinOp::Eq);
    assert!(matches!(lhs.as_ref(), Expr::Is { .. }));
    assert!(matches!(
        rhs.as_ref(),
        Expr::BoolLiteral { value: true, .. }
    ));
}

#[test]
fn is_chains_left() {
    let expr = init_expr("a is Int is Any");
    let Expr::Is { operand, ty, .. } = &expr else {
        panic!("expected an is expression");
    };
    assert!(matches!(&ty.kind, TypeRefKind::Named(name) if name.text == "Any"));
    assert!(matches!(operand.as_ref(), Expr::Is { .. }));
}

#[test]
fn as_expression() {
    let expr = init_expr("a as Point");
    let Expr::Cast {
        operand,
        ty,
        optional,
        span,
    } = &expr
    else {
        panic!("expected a cast expression");
    };
    assert_eq!(*span, Span::new(25, 35));
    assert!(!optional);
    assert!(matches!(operand.as_ref(), Expr::Var(name) if name.text == "a"));
    assert!(matches!(&ty.kind, TypeRefKind::Named(name) if name.text == "Point"));
    assert_eq!(
        stmt_dump("val s = a as? S"),
        "val s\n  Cast S optional=true\n    Var a\n"
    );
}

#[test]
fn safe_cast_after_safe_navigation() {
    // `a?.x as? Point` is `(a?.x) as? Point` — postfix binds tighter.
    let expr = init_expr("a?.x as? Point");
    let Expr::Cast {
        operand,
        ty,
        optional,
        ..
    } = &expr
    else {
        panic!("expected a cast expression");
    };
    assert!(optional);
    assert!(matches!(&ty.kind, TypeRefKind::Named(name) if name.text == "Point"));
    let Expr::FieldAccess(access) = operand.as_ref() else {
        panic!("expected a field access operand");
    };
    assert_eq!(access.navigation, Navigation::Safe);
}

#[test]
fn cast_with_nullable_type() {
    // `a as Point?` casts to the nullable type; the `?` belongs to the
    // type, so this is not the safe cast.
    let expr = init_expr("a as Point?");
    let Expr::Cast { ty, optional, .. } = &expr else {
        panic!("expected a cast expression");
    };
    assert!(!optional);
    assert!(matches!(&ty.kind, TypeRefKind::Nullable(_)));
}

#[test]
fn bang_is_requires_adjacent_tokens() {
    // `! is` with a space in between is not the `!is` operator.
    let (span, message) = err("fun main() { a ! is Int }");
    assert_eq!(span, Span::new(15, 16));
    assert_eq!(
        message,
        "expected `;` or newline after statement, found `!`"
    );
}
