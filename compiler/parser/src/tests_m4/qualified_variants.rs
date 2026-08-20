use super::*;

// --- qualified variant construction `E.V(args)` ------------------------------

#[test]
fn qualified_variant_construction_is_a_method_call() {
    // M4 collapsed `Shape.Circle(5)` into a dotted `Call`; since M6 the
    // same syntax parses as a method call (`expr.name(args)`), and
    // hir-lower resolves enum variant construction from that shape.
    let Expr::MethodCall {
        receiver,
        name,
        args,
        span,
        ..
    } = crate::tests_m2::init_expr("Shape.Circle(5)")
    else {
        panic!("expected a method call");
    };
    assert!(matches!(&*receiver, Expr::Var(head) if head.text == "Shape"));
    assert_eq!(name.text, "Circle");
    assert_eq!(name.span, Span::new(31, 37));
    assert_eq!(args.len(), 1);
    assert_eq!(span, Span::new(25, 40));
}

#[test]
fn qualified_variant_construction_without_arguments() {
    let Expr::MethodCall {
        receiver,
        name,
        args,
        ..
    } = crate::tests_m2::init_expr("Shape.WithDefault()")
    else {
        panic!("expected a method call");
    };
    assert!(matches!(&*receiver, Expr::Var(head) if head.text == "Shape"));
    assert_eq!(name.text, "WithDefault");
    assert!(args.is_empty());
}

#[test]
fn qualified_unit_variant_stays_a_field_access() {
    // Without a call, `Color.Red` is a FieldAccess; hir-lower resolves
    // the enum receiver to a unit variant construction.
    let Expr::FieldAccess(access) = crate::tests_m2::init_expr("Color.Red") else {
        panic!("expected a field access");
    };
    assert!(matches!(&*access.receiver, Expr::Var(name) if name.text == "Color"));
    let FieldSelector::Name(selector) = &access.selector else {
        panic!("expected a named selector");
    };
    assert_eq!(selector.text, "Red");
}

#[test]
fn field_access_without_a_call_is_unchanged() {
    // `p.x + 1` — the dot is not followed by `(`, so nothing collapses.
    let Expr::Binary { lhs, .. } = crate::tests_m2::init_expr("p.x + 1") else {
        panic!("expected a binary expression");
    };
    assert!(matches!(&*lhs, Expr::FieldAccess(_)));
}
