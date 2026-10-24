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
fn explicit_unit_companion_access_preserves_the_type_qualifier() {
    for source in ["Unit.Companion", "Unit . `Companion`", "Unit\n.Companion"] {
        let Expr::FieldAccess(access) = init_expr(source) else {
            panic!("expected companion access");
        };
        assert!(matches!(&*access.receiver, Expr::Var(name) if name.text == "Unit"));
        assert!(matches!(&access.selector, FieldSelector::Name(name) if name.text == "Companion"));
    }
}

#[test]
fn ordinary_unit_members_and_parenthesized_values_keep_value_receivers() {
    let Expr::MethodCall { receiver, .. } = init_expr("Unit.equals(())") else {
        panic!("expected a Unit value method call");
    };
    assert!(matches!(&*receiver, Expr::UnitLiteral { .. }));
    for source in ["().Companion", "(Unit).Companion", "Unit?.Companion"] {
        let Expr::FieldAccess(access) = init_expr(source) else {
            panic!("expected value field access");
        };
        assert!(matches!(&*access.receiver, Expr::UnitLiteral { .. }));
    }
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
        Expr::IntLiteral(literal) if literal.magnitude == 1
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
