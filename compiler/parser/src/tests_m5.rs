//! Unit tests for the M5 syntax: array literals (`[e1, e2, ...]`,
//! including empty and nested), the subscript postfix (`a[i]`, chained
//! and on call results), subscript assignment targets (`m[i] = v`), and
//! the M5 "not supported" diagnostics (ranges and slices).

use scoop_ast::{AssignTarget, Expr, Span, StatementKind, TypeRefKind};

use crate::tests::{block_body, err, ok, only_function};
use crate::tests_m2::{init_expr, stmt_dump};

// --- array literals ---------------------------------------------------------

#[test]
fn array_literal() {
    // `fun main() {\n    val x = [1, 2, 3]\n}\n` — `[` is at offset 25.
    let expr = init_expr("[1, 2, 3]");
    let Expr::ArrayLiteral { elements, span } = &expr else {
        panic!("expected an array literal");
    };
    assert_eq!(*span, Span::new(25, 34));
    assert_eq!(elements.len(), 3);
    for (element, (value, start)) in elements.iter().zip([(1, 26), (2, 29), (3, 32)]) {
        let Expr::IntLiteral { value: v, span } = element else {
            panic!("expected an integer literal element");
        };
        assert_eq!(*v, value);
        assert_eq!(*span, Span::new(start, start + 1));
    }
    assert_eq!(
        stmt_dump("val a = [1, 2, 3]"),
        "val a\n  ArrayLiteral\n    IntLiteral 1\n    IntLiteral 2\n    IntLiteral 3\n"
    );
}

#[test]
fn empty_array_literal_parses_without_annotation() {
    // `[]` parses everywhere a literal can appear; HIR rejects it when
    // there is no expected type.
    let expr = init_expr("[]");
    let Expr::ArrayLiteral { elements, span } = &expr else {
        panic!("expected an array literal");
    };
    assert!(elements.is_empty());
    assert_eq!(*span, Span::new(25, 27));
}

#[test]
fn empty_array_literal_with_type_context() {
    let file = ok("fun main() {\n    val a: Array<Int> = []\n}\n");
    let StatementKind::ValDecl(decl) = &block_body(only_function(&file)).statements[0].kind else {
        panic!("expected a val declaration");
    };
    let ty = decl.ty.as_ref().expect("a type annotation");
    let TypeRefKind::Generic(name, args) = &ty.kind else {
        panic!("expected a generic type annotation");
    };
    assert_eq!(name.text, "Array");
    assert_eq!(args.len(), 1);
    assert!(matches!(args[0].kind, TypeRefKind::Named(_)));
    let Expr::ArrayLiteral { elements, span } = &decl.init else {
        panic!("expected an array literal");
    };
    assert!(elements.is_empty());
    assert_eq!(*span, Span::new(37, 39));
}

#[test]
fn nested_array_literal() {
    assert_eq!(
        stmt_dump("val nested = [[1], [2]]"),
        "val nested\n  ArrayLiteral\n    ArrayLiteral\n      IntLiteral 1\n    ArrayLiteral\n      IntLiteral 2\n"
    );
}

#[test]
fn array_literal_elements_are_full_expressions() {
    assert_eq!(
        stmt_dump("val a = [1 + 2, f(3)]"),
        "val a\n  ArrayLiteral\n    Binary Add\n      IntLiteral 1\n      IntLiteral 2\n    Call f\n      IntLiteral 3\n"
    );
}

#[test]
fn array_literal_as_call_argument() {
    assert_eq!(
        stmt_dump("f([1, 2])"),
        "Call f\n  ArrayLiteral\n    IntLiteral 1\n    IntLiteral 2\n"
    );
}

#[test]
fn mutable_array_annotation() {
    // `MutableArray<Int>` uses the M4 `Name<T>` syntax; nothing new here.
    assert_eq!(
        stmt_dump("val m: MutableArray<Int> = [4, 5]"),
        "val m: MutableArray<Int>\n  ArrayLiteral\n    IntLiteral 4\n    IntLiteral 5\n"
    );
}

// --- subscript read ---------------------------------------------------------

#[test]
fn subscript_read() {
    // `fun main() {\n    val x = a[0]\n}\n` — `a` is at offset 25.
    let expr = init_expr("a[0]");
    let Expr::Index {
        receiver,
        indices,
        span,
    } = &expr
    else {
        panic!("expected an index expression");
    };
    assert_eq!(*span, Span::new(25, 29));
    let Expr::Var(name) = receiver.as_ref() else {
        panic!("expected a variable receiver");
    };
    assert_eq!(name.text, "a");
    assert_eq!(name.span, Span::new(25, 26));
    assert!(matches!(
        indices.first(),
        Expr::IntLiteral {
            value: 0,
            span: Span { start: 27, end: 28 }
        }
    ));
}

#[test]
fn subscript_chains_left() {
    let expr = init_expr("a[0][1]");
    let Expr::Index {
        receiver, indices, ..
    } = &expr
    else {
        panic!("expected an index expression");
    };
    assert!(matches!(indices.first(), Expr::IntLiteral { value: 1, .. }));
    let Expr::Index {
        receiver: inner_receiver,
        indices: inner_indices,
        ..
    } = receiver.as_ref()
    else {
        panic!("expected the receiver to be an index expression");
    };
    assert!(matches!(inner_receiver.as_ref(), Expr::Var(name) if name.text == "a"));
    assert!(matches!(
        inner_indices.first(),
        Expr::IntLiteral { value: 0, .. }
    ));
    assert_eq!(
        stmt_dump("val x = a[0][1]"),
        "val x\n  Index\n    Index\n      Var a\n      IntLiteral 0\n    IntLiteral 1\n"
    );
}

#[test]
fn subscript_on_call_result() {
    assert_eq!(
        stmt_dump("val x = f()[i]"),
        "val x\n  Index\n    Call f\n    Var i\n"
    );
}

#[test]
fn subscript_binds_tighter_than_binary() {
    assert_eq!(
        stmt_dump("val x = a[0] + b"),
        "val x\n  Binary Add\n    Index\n      Var a\n      IntLiteral 0\n    Var b\n"
    );
}

#[test]
fn subscript_and_field_access_chain() {
    assert_eq!(
        stmt_dump("val x = ps[1].x"),
        "val x\n  FieldAccess x\n    Index\n      Var ps\n      IntLiteral 1\n"
    );
    // `.size` stays an ordinary field access; HIR resolves the pseudo
    // property on array types.
    assert_eq!(
        stmt_dump("val n = a.size"),
        "val n\n  FieldAccess size\n    Var a\n"
    );
}

#[test]
fn bracket_at_line_start_is_a_new_literal_expression() {
    // The subscript postfix requires the `[` to touch the receiver; a
    // line-start `[` begins a new array literal statement instead.
    let file = ok("fun main() {\n    val a = [1, 2]\n    [3, 4]\n}\n");
    let statements = &block_body(only_function(&file)).statements;
    assert_eq!(statements.len(), 2);
    let StatementKind::Expr(Expr::ArrayLiteral { elements, .. }) = &statements[1].kind else {
        panic!("expected an array literal statement");
    };
    assert_eq!(elements.len(), 2);
}

#[test]
fn space_before_bracket_is_property_like_infix_not_subscript() {
    let file = ok("fun main() { a [0] }");
    let statement = &block_body(only_function(&file)).statements[0];
    assert!(matches!(
        statement.kind,
        StatementKind::Expr(Expr::InfixCall {
            target: scoop_ast::InfixTarget::Invoke,
            ..
        })
    ));
}

// --- subscript assignment ---------------------------------------------------

#[test]
fn subscript_assignment() {
    // `fun main() {\n    m[0] = 40\n}\n` — `m` is at offset 17.
    let file = ok("fun main() {\n    m[0] = 40\n}\n");
    let stmt = &block_body(only_function(&file)).statements[0];
    assert_eq!(stmt.span, Span::new(17, 26));
    let StatementKind::Assign(assign) = &stmt.kind else {
        panic!("expected an assignment");
    };
    assert_eq!(assign.span, Span::new(17, 26));
    let AssignTarget::Index {
        receiver,
        indices,
        span,
    } = &assign.target
    else {
        panic!("expected an index assignment target");
    };
    assert_eq!(*span, Span::new(17, 21));
    assert!(matches!(receiver.as_ref(), Expr::Var(name) if name.text == "m"));
    assert!(matches!(
        indices.first(),
        Expr::IntLiteral {
            value: 0,
            span: Span { start: 19, end: 20 }
        }
    ));
    assert!(matches!(
        assign.value,
        Expr::IntLiteral {
            value: 40,
            span: Span { start: 24, end: 26 }
        }
    ));
    assert_eq!(
        stmt_dump("m[0] = 40"),
        "assign []\n  Var m\n  IntLiteral 0\n  IntLiteral 40\n"
    );
}

#[test]
fn nested_subscript_assignment() {
    assert_eq!(
        stmt_dump("m[i][j] = v"),
        "assign []\n  Index\n    Var m\n    Var i\n  Var j\n  Var v\n"
    );
}

#[test]
fn call_result_assignment_is_rejected() {
    // Only a plain local or a subscript is a valid assignment target.
    let (span, message) = err("fun main() { f() = 1 }");
    assert_eq!(span, Span::new(13, 16));
    assert_eq!(message, "assignment target must be an assignable place");
}

// --- M5 "not supported" diagnostics ------------------------------------------

#[test]
fn range_is_a_binary_expression() {
    assert_eq!(
        stmt_dump("val r = a..b"),
        "val r\n  Binary RangeTo\n    Var a\n    Var b\n"
    );
}

#[test]
fn range_binds_looser_than_addition() {
    assert_eq!(
        stmt_dump("val r = a + b..c"),
        "val r\n  Binary RangeTo\n    Binary Add\n      Var a\n      Var b\n    Var c\n"
    );
}

#[test]
fn slice_not_supported() {
    let (span, message) = err("fun main() { val x = a[1:2] }");
    assert_eq!(span, Span::new(24, 25));
    assert_eq!(message, "array slices are not supported yet (milestone M5)");
}

// --- literal syntax errors ---------------------------------------------------

#[test]
fn array_literal_trailing_comma_is_an_error() {
    // Same rule as call arguments: no trailing comma.
    let (_, message) = err("fun main() { val a = [1, 2,] }");
    assert_eq!(message, "expected expression, found `]`");
}

#[test]
fn unclosed_array_literal() {
    let (_, message) = err("fun main() { val a = [1, 2 }");
    assert_eq!(message, "expected `]`, found `}`");
}
