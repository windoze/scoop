use scoop_ast::{Expr, Span};

use crate::tests::err;
use crate::tests_m2::{init_expr, stmt_dump};

#[test]
fn copy_update_is_a_non_empty_dedicated_postfix_node() {
    let expr = init_expr("source.{ second: next(), first: 1 }");
    let Expr::CopyUpdate { base, fields, span } = expr else {
        panic!("expected a copy-update expression");
    };
    assert!(matches!(*base, Expr::Var(ref name) if name.text == "source"));
    assert_eq!(fields.len(), 2);
    assert_eq!(fields.first().field.text, "second");
    assert_eq!(fields.as_slice()[1].field.text, "first");
    assert_eq!(span, Span::new(25, 60));
    assert_eq!(
        stmt_dump("val x = source.{ second: next(), first: 1 }"),
        "val x\n  CopyUpdate\n    base\n      Var source\n    field second\n      Call next\n    field first\n      IntLiteral 1\n"
    );
}

#[test]
fn copy_update_chains_as_an_ordinary_postfix_expression() {
    assert_eq!(
        stmt_dump("val x = source.{ first: 1 }.first"),
        "val x\n  FieldAccess first\n    CopyUpdate\n      base\n        Var source\n      field first\n        IntLiteral 1\n"
    );
}

#[test]
fn empty_copy_update_is_rejected_by_the_parser() {
    let (span, message) = err("fun main() { val x = source.{} }");
    assert_eq!(span, Span::new(29, 30));
    assert_eq!(message, "copy update field list must not be empty");
}

#[test]
fn copy_update_rejects_safe_navigation_rest_and_nested_paths() {
    let (span, message) = err("fun main() { val x = source?.{ first: 1 } }");
    assert_eq!(span, Span::new(27, 29));
    assert_eq!(
        message,
        "copy update does not support safe navigation; unwrap the Option with `when` first"
    );

    let (span, message) = err("fun main() { val x = source.{ .. } }");
    assert_eq!(span, Span::new(30, 32));
    assert_eq!(
        message,
        "copy update field list does not allow rest entries"
    );

    let (span, message) = err("fun main() { val x = source.{ nested.first: 1 } }");
    assert_eq!(span, Span::new(36, 37));
    assert_eq!(
        message,
        "copy update fields must be direct names, not nested paths"
    );
}

#[test]
fn copy_update_rejects_statements_and_trailing_commas() {
    let (span, message) = err("fun main() { val x = source.{ first: val y = 1 } }");
    assert_eq!(span, Span::new(37, 40));
    assert_eq!(
        message,
        "copy update field values must be expressions, not statements"
    );

    for (statement, end) in [("throw error", 42), ("break", 42), ("continue", 45)] {
        let source = format!("fun main() {{ val x = source.{{ first: {statement} }} }}");
        let (span, message) = err(&source);
        assert_eq!(span, Span::new(37, end));
        assert_eq!(
            message,
            "copy update field values must be expressions, not statements"
        );
    }

    let (span, message) = err("fun main() { val x = source.{ first: 1, } }");
    assert_eq!(span, Span::new(40, 41));
    assert_eq!(
        message,
        "copy update field list does not allow a trailing comma"
    );
}
