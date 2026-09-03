use scoop_ast::{Expr, Span};

use crate::tests_m2::init_expr;

// --- reference equality -----------------------------------------------------------

#[test]
fn reference_equality() {
    let expr = init_expr("x === y");
    let Expr::Binary { op, span, .. } = &expr else {
        panic!("expected a binary expression");
    };
    assert_eq!(*op, scoop_ast::BinOp::RefEq);
    assert_eq!(*span, Span::new(25, 32));
    let expr = init_expr("x !== y");
    let Expr::Binary { op, .. } = &expr else {
        panic!("expected a binary expression");
    };
    assert_eq!(*op, scoop_ast::BinOp::RefNe);
}

#[test]
fn reference_equality_is_left_associative() {
    // `x === y !== z` is `(x === y) !== z`.
    let expr = init_expr("x === y !== z");
    let Expr::Binary { op, lhs, rhs, .. } = &expr else {
        panic!("expected a binary expression");
    };
    assert_eq!(*op, scoop_ast::BinOp::RefNe);
    assert!(matches!(rhs.as_ref(), Expr::Var(name) if name.text == "z"));
    let Expr::Binary {
        op: inner_op,
        lhs: inner_lhs,
        rhs: inner_rhs,
        ..
    } = lhs.as_ref()
    else {
        panic!("expected the left operand to be `x === y`");
    };
    assert_eq!(*inner_op, scoop_ast::BinOp::RefEq);
    assert!(matches!(inner_lhs.as_ref(), Expr::Var(name) if name.text == "x"));
    assert!(matches!(inner_rhs.as_ref(), Expr::Var(name) if name.text == "y"));
}

#[test]
fn reference_equality_at_equality_precedence() {
    // `a === b == c` is `(a === b) == c` — same tier, left associative.
    let expr = init_expr("a === b == c");
    let Expr::Binary { op, lhs, .. } = &expr else {
        panic!("expected a binary expression");
    };
    assert_eq!(*op, scoop_ast::BinOp::Eq);
    assert!(matches!(
        lhs.as_ref(),
        Expr::Binary {
            op: scoop_ast::BinOp::RefEq,
            ..
        }
    ));
}
