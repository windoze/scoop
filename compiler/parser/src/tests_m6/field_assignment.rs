use scoop_ast::{AssignTarget, Expr, Span, StatementKind};

use crate::tests::{block_body, err, ok, only_function};
use crate::tests_m2::stmt_dump;

// --- field assignment -----------------------------------------------------------

#[test]
fn field_assignment() {
    // `fun main() {\n    p.y = 3\n}\n` — `p` is at offset 17.
    let file = ok("fun main() {\n    p.y = 3\n}\n");
    let stmt = &block_body(only_function(&file)).statements[0];
    assert_eq!(stmt.span, Span::new(17, 24));
    let StatementKind::Assign(assign) = &stmt.kind else {
        panic!("expected an assignment");
    };
    assert_eq!(assign.span, Span::new(17, 24));
    let AssignTarget::Field {
        receiver,
        name,
        span,
    } = &assign.target
    else {
        panic!("expected a field assignment target");
    };
    assert_eq!(*span, Span::new(17, 20));
    assert_eq!(name.text, "y");
    assert_eq!(name.span, Span::new(19, 20));
    assert!(matches!(receiver.as_ref(), Expr::Var(name) if name.text == "p"));
    assert!(matches!(assign.value, Expr::IntLiteral { value: 3, .. }));
    assert_eq!(stmt_dump("p.y = 3"), "assign .y\n  Var p\n  IntLiteral 3\n");
}

#[test]
fn field_assignment_nested_receiver() {
    assert_eq!(
        stmt_dump("p.x.f = 1"),
        "assign .f\n  FieldAccess x\n    Var p\n  IntLiteral 1\n"
    );
}

#[test]
fn field_assignment_on_this() {
    assert_eq!(
        stmt_dump("this.x = 1"),
        "assign .x\n  This\n  IntLiteral 1\n"
    );
}

#[test]
fn subscript_of_field_stays_an_index_assignment() {
    // `a.b[i]` is an Index expression, so the assignment target keeps
    // the Index classification (the receiver is the field access).
    assert_eq!(
        stmt_dump("a.b[i] = 1"),
        "assign []\n  FieldAccess b\n    Var a\n  Var i\n  IntLiteral 1\n"
    );
}

#[test]
fn field_assignment_value_is_a_full_expression() {
    assert_eq!(
        stmt_dump("p.y = p.x + 1"),
        "assign .y\n  Var p\n  Binary Add\n    FieldAccess x\n      Var p\n    IntLiteral 1\n"
    );
}

#[test]
fn safe_navigation_assignment_not_allowed() {
    let (span, message) = err("fun main() { a?.b = 1 }");
    assert_eq!(span, Span::new(13, 17));
    assert_eq!(message, "assignments through `?.` are not allowed");
}
