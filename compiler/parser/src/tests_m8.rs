//! Unit tests for the M8 syntax: `throw` statements, `try` / `catch` /
//! `finally` statements (single and multiple catches, finally-only,
//! nested `try`, catch clauses across newlines), the full type syntax in
//! catch parameter annotations, and every M8 diagnostic (try
//! expressions, missing catch type annotation, `try` without `catch` or
//! `finally`).

use scoop_ast::{Expr, Span, StatementKind, TypeRefKind};

use crate::tests::{block_body, err, ok, only_function};
use crate::tests_m2::stmt_dump;

// --- throw -----------------------------------------------------------------

#[test]
fn throw_statement() {
    let file = ok("fun main() {\n    throw MyError(42)\n}\n");
    let stmt = &block_body(only_function(&file)).statements[0];
    assert_eq!(stmt.span, Span::new(17, 34));
    let StatementKind::Throw(expr) = &stmt.kind else {
        panic!("expected a throw statement");
    };
    let Expr::Call(call) = expr else {
        panic!("expected a call operand");
    };
    assert_eq!(call.callee.text, "MyError");
    assert_eq!(call.args.len(), 1);
}

#[test]
fn throw_operand_runs_to_statement_end() {
    // `throw f(1) + 2` throws the whole `f(1) + 2`, not just `f(1)`.
    let dump = stmt_dump("throw f(1) + 2");
    assert_eq!(
        dump,
        "throw\n  Binary Add\n    Call f\n      IntLiteral 1\n    IntLiteral 2\n"
    );
}

// --- try / catch / finally ---------------------------------------------------

#[test]
fn try_catch_statement() {
    let file = ok(
        "fun main() {\n    try {\n        foo()\n    } catch (e: MyError) {\n        bar()\n    }\n}\n",
    );
    let stmt = &block_body(only_function(&file)).statements[0];
    assert_eq!(stmt.span, Span::new(17, 83));
    let StatementKind::Try(try_) = &stmt.kind else {
        panic!("expected a try statement");
    };
    assert_eq!(try_.span, Span::new(17, 83));
    assert_eq!(try_.body.span, Span::new(21, 42));
    assert_eq!(try_.body.statements.len(), 1);
    assert_eq!(try_.catches.len(), 1);
    let catch = &try_.catches[0];
    assert_eq!(catch.name.text, "e");
    assert_eq!(catch.name.span, Span::new(50, 51));
    assert_eq!(catch.span, Span::new(43, 83));
    let TypeRefKind::Named(ty) = &catch.ty.kind else {
        panic!("expected a named catch type");
    };
    assert_eq!(ty.text, "MyError");
    assert_eq!(catch.body.span, Span::new(62, 83));
    assert_eq!(catch.body.statements.len(), 1);
    assert!(try_.finally_body.is_none());
}

#[test]
fn multiple_catches_parse_in_order() {
    let file = ok(
        "fun main() {\n    try {\n    } catch (a: UnwrapException) {\n    } catch (b: Exception) {\n    }\n}\n",
    );
    let stmt = &block_body(only_function(&file)).statements[0];
    let StatementKind::Try(try_) = &stmt.kind else {
        panic!("expected a try statement");
    };
    assert_eq!(try_.catches.len(), 2);
    assert_eq!(try_.catches[0].name.text, "a");
    let TypeRefKind::Named(ty) = &try_.catches[0].ty.kind else {
        panic!("expected a named catch type");
    };
    assert_eq!(ty.text, "UnwrapException");
    assert_eq!(try_.catches[1].name.text, "b");
    let TypeRefKind::Named(ty) = &try_.catches[1].ty.kind else {
        panic!("expected a named catch type");
    };
    assert_eq!(ty.text, "Exception");
    assert!(try_.finally_body.is_none());
}

#[test]
fn try_finally_without_catch() {
    let file = ok("fun main() {\n    try {\n    } finally {\n        cleanup()\n    }\n}\n");
    let stmt = &block_body(only_function(&file)).statements[0];
    let StatementKind::Try(try_) = &stmt.kind else {
        panic!("expected a try statement");
    };
    assert!(try_.catches.is_empty());
    let finally = try_.finally_body.as_ref().expect("a finally block");
    assert_eq!(finally.statements.len(), 1);
}

#[test]
fn try_with_catches_and_finally_dump() {
    let dump = stmt_dump(
        "try {\n        throw MyError(42)\n    } catch (e: MyError) {\n        bar()\n    } catch (e: Exception) {\n        baz()\n    } finally {\n        cleanup()\n    }",
    );
    assert_eq!(
        dump,
        "try\n  throw\n    Call MyError\n      IntLiteral 42\ncatch e: MyError\n  Call bar\ncatch e: Exception\n  Call baz\nfinally\n  Call cleanup\n"
    );
}

#[test]
fn nested_try_dump() {
    let dump = stmt_dump(
        "try {\n        try {\n            foo()\n        } catch (e: E) {\n            bar()\n        }\n    } finally {\n        cleanup()\n    }",
    );
    assert_eq!(
        dump,
        "try\n  try\n    Call foo\n  catch e: E\n    Call bar\nfinally\n  Call cleanup\n"
    );
}

#[test]
fn catch_attaches_across_newlines() {
    // A `catch` on the line after the try body's closing brace still
    // belongs to the `try` (it is not a separate statement).
    let file = ok("fun main() {\n    try {\n    }\n    catch (e: E) {\n    }\n}\n");
    let body = block_body(only_function(&file));
    assert_eq!(body.statements.len(), 1);
    let StatementKind::Try(try_) = &body.statements[0].kind else {
        panic!("expected a try statement");
    };
    assert_eq!(try_.catches.len(), 1);
}

#[test]
fn catch_type_annotation_uses_full_type_syntax() {
    let file = ok("fun main() {\n    try {\n    } catch (e: Option<Int>) {\n    }\n}\n");
    let stmt = &block_body(only_function(&file)).statements[0];
    let StatementKind::Try(try_) = &stmt.kind else {
        panic!("expected a try statement");
    };
    let TypeRefKind::Generic(name, args) = &try_.catches[0].ty.kind else {
        panic!("expected a generic catch type");
    };
    assert_eq!(name.text, "Option");
    assert_eq!(args.len(), 1);
}

#[test]
fn catch_and_finally_stay_identifiers_elsewhere() {
    // `catch` / `finally` are contextual: outside a `try` tail they are
    // ordinary identifiers.
    let file = ok("fun main() {\n    catch(1)\n    finally(2)\n}\n");
    let body = block_body(only_function(&file));
    assert_eq!(body.statements.len(), 2);
    for (stmt, callee) in body.statements.iter().zip(["catch", "finally"]) {
        let StatementKind::Expr(Expr::Call(call)) = &stmt.kind else {
            panic!("expected a call statement");
        };
        assert_eq!(call.callee.text, callee);
    }
}

// --- diagnostics -------------------------------------------------------------

#[test]
fn try_expression_is_not_supported() {
    let (span, message) = err("fun main() {\n    val x = try {\n    }\n}\n");
    assert_eq!(span, Span::new(25, 28));
    assert_eq!(
        message,
        "try expressions are not supported yet (milestone M8)"
    );
}

#[test]
fn catch_parameter_requires_a_type_annotation() {
    let (span, message) = err("fun main() {\n    try {\n    } catch (e) {\n    }\n}\n");
    assert_eq!(span, Span::new(37, 38));
    assert_eq!(message, "catch parameter requires a type annotation");
}

#[test]
fn catch_parameter_requires_a_name() {
    let (span, message) = err("fun main() {\n    try {\n    } catch (: E) {\n    }\n}\n");
    assert_eq!(span, Span::new(36, 37));
    assert_eq!(message, "expected catch parameter name, found `:`");
}

#[test]
fn try_without_catch_or_finally() {
    let (_, message) = err("fun main() {\n    try {\n    }\n}\n");
    assert_eq!(message, "expected `catch` or `finally`, found `}`");
}

#[test]
fn throw_without_an_operand() {
    let (_, message) = err("fun main() {\n    throw\n}\n");
    assert_eq!(message, "expected expression, found `}`");
}
