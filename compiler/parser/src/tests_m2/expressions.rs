use super::*;

// --- expressions ----------------------------------------------------------

#[test]
fn int_and_bool_literals() {
    assert!(
        matches!(init_expr("42"), Expr::IntLiteral { value: 42, span } if span == Span::new(25, 27))
    );
    assert!(matches!(
        init_expr("true"),
        Expr::BoolLiteral { value: true, .. }
    ));
    assert!(matches!(
        init_expr("false"),
        Expr::BoolLiteral { value: false, .. }
    ));
}

#[test]
fn multiplicative_binds_tighter_than_additive() {
    assert_eq!(
        stmt_dump("val x = 1 + 2 * 3"),
        "val x\n  Binary Add\n    IntLiteral 1\n    Binary Mul\n      IntLiteral 2\n      IntLiteral 3\n"
    );
}

#[test]
fn binary_operators_are_left_associative() {
    assert_eq!(
        stmt_dump("val x = 1 - 2 - 3"),
        "val x\n  Binary Sub\n    Binary Sub\n      IntLiteral 1\n      IntLiteral 2\n    IntLiteral 3\n"
    );
}

#[test]
fn and_binds_tighter_than_or() {
    assert_eq!(
        stmt_dump("val x = a || b && c"),
        "val x\n  Binary Or\n    Var a\n    Binary And\n      Var b\n      Var c\n"
    );
}

#[test]
fn comparison_binds_tighter_than_equality() {
    assert_eq!(
        stmt_dump("val x = 1 < 2 == true"),
        "val x\n  Binary Eq\n    Binary Lt\n      IntLiteral 1\n      IntLiteral 2\n    BoolLiteral true\n"
    );
}

#[test]
fn unary_binds_tighter_than_equality() {
    assert_eq!(
        stmt_dump("val x = !a == b"),
        "val x\n  Binary Eq\n    Unary Not\n      Var a\n    Var b\n"
    );
}

#[test]
fn postfix_binds_tighter_than_unary() {
    let expr = init_expr("-p.x");
    let Expr::Unary {
        op: UnOp::Neg,
        operand,
        span,
    } = &expr
    else {
        panic!("expected a unary expression");
    };
    assert!(matches!(**operand, Expr::FieldAccess(_)));
    assert_eq!(*span, Span::new(25, 29));
    assert_eq!(
        stmt_dump("val x = !!flag"),
        "val x\n  Unary Not\n    Unary Not\n      Var flag\n"
    );
}

#[test]
fn field_access_by_name_and_index() {
    assert_eq!(
        stmt_dump("val y = t._1._2"),
        "val y\n  FieldAccess _2\n    FieldAccess _1\n      Var t\n"
    );
    let expr = init_expr("p.x");
    let Expr::FieldAccess(access) = &expr else {
        panic!("expected a field access");
    };
    assert_eq!(access.span, Span::new(25, 28));
    let FieldSelector::Name(name) = &access.selector else {
        panic!("expected a named selector");
    };
    assert_eq!(name.text, "x");
    let expr = init_expr("q._12");
    let Expr::FieldAccess(access) = &expr else {
        panic!("expected a field access");
    };
    assert!(matches!(access.selector, FieldSelector::Index(12, _)));
}

#[test]
fn field_access_on_call_result() {
    // Struct construction is plain call syntax in M2 (parsed as `Call`).
    assert_eq!(
        stmt_dump("val x = Point(1, 2).x"),
        "val x\n  FieldAccess x\n    Call Point\n      IntLiteral 1\n      IntLiteral 2\n"
    );
}

#[test]
fn division_operator_coexists_with_comments() {
    assert_eq!(
        stmt_dump("val x = 6 / 2 // half"),
        "val x\n  Binary Div\n    IntLiteral 6\n    IntLiteral 2\n"
    );
    assert_eq!(
        stmt_dump("val x = a /* not a comment error */ / b"),
        "val x\n  Binary Div\n    Var a\n    Var b\n"
    );
}
