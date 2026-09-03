use super::*;

// --- unit / tuple / paren disambiguation (spec 4.3) ------------------------

#[test]
fn empty_parens_are_the_unit_literal() {
    let Expr::UnitLiteral { span } = init_expr("()") else {
        panic!("expected a unit literal");
    };
    assert_eq!(span, Span::new(25, 27));
}

#[test]
fn unit_identifier_is_the_unit_literal() {
    let Expr::UnitLiteral { span } = init_expr("Unit") else {
        panic!("expected a unit literal");
    };
    assert_eq!(span, Span::new(25, 29));
}

#[test]
fn parens_around_one_expr_produce_no_node() {
    let Expr::Var(ident) = init_expr("(y)") else {
        panic!("expected the parenthesized expression itself");
    };
    assert_eq!(ident.text, "y");
    // Nested parens dissolve the same way.
    assert!(matches!(
        init_expr("((1))"),
        Expr::IntLiteral { value: 1, .. }
    ));
}

#[test]
fn trailing_comma_makes_a_one_tuple() {
    let Expr::TupleLiteral { elements, span } = init_expr("(y,)") else {
        panic!("expected a 1-tuple literal");
    };
    assert_eq!(elements.len(), 1);
    assert_eq!(span, Span::new(25, 29));
}

#[test]
fn comma_makes_a_tuple() {
    let Expr::TupleLiteral { elements, .. } = init_expr("(y, z)") else {
        panic!("expected a tuple literal");
    };
    assert_eq!(elements.len(), 2);
    // A trailing comma is allowed on multi-element tuples too.
    let Expr::TupleLiteral { elements, .. } = init_expr("(y, z,)") else {
        panic!("expected a tuple literal");
    };
    assert_eq!(elements.len(), 2);
}
