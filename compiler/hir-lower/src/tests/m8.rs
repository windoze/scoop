//! M8 tests: exceptions (milestone8 DESIGN.md 3.2) — `throw` operands
//! and catch parameter types checked against the core `Throwable`
//! class, catch shadowing (unreachable catch is an error, DESIGN.md
//! 5.1), catch-local scoping, and the core contract that `scoop.core`
//! must define a class `Throwable`.
//!
//! The parser-level rules (catch parameters require a type annotation,
//! a `try` needs at least one `catch` or a `finally`, `try` is not an
//! expression) are covered by `scoop-parser`'s M8 tests; the AST
//! cannot represent their violations.

use super::*;

fn custom_error() -> Decl {
    class_decl(
        ast::ClassModifier::Final,
        "MyError",
        vec![(false, "code", ty_named("Int"))],
        Some(("Exception", vec![some(str_lit("my error"))])),
        vec![],
        vec![],
    )
}

// --- positive: golden dump ---

mod diagnostics;
mod lowering;
mod returns;
