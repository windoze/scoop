//! Unit tests for the M4 syntax: enum declarations (four variant forms,
//! type parameters, constant default values), `@Intrinsic` annotations,
//! the statement-level `when` (patterns, guards, `..`, `else`), and
//! destructuring `val` / `var` declarations.

use scoop_ast::{
    Decl, Expr, FieldSelector, FunctionBody, Pattern, Span, StatementKind, TypeRefKind,
    VariantDeclKind, When,
};

use crate::tests::{block_body, err, ok, only_function};
use crate::tests_m2::stmt_dump;

/// Parses `fun main() { when (s) { <arms> } }` and returns the `When`.
fn when_with_arms(arms: &str) -> When {
    let file = ok(&format!(
        "fun main() {{\n    when (s) {{\n{arms}    }}\n}}\n"
    ));
    let StatementKind::When(when) = &block_body(only_function(&file)).statements[0].kind else {
        panic!("expected a when statement");
    };
    when.clone()
}

/// Parses `fun main() { val <target> = init }`-style source and returns
/// the declared pattern.
fn val_target(target: &str) -> scoop_ast::ValDecl {
    let file = ok(&format!("fun main() {{\n    val {target} = init\n}}\n"));
    let StatementKind::ValDecl(decl) = &block_body(only_function(&file)).statements[0].kind else {
        panic!("expected a val declaration");
    };
    decl.clone()
}

mod annotations;
mod destructuring;
mod enum_declarations;
mod enum_diagnostics;
mod qualified_variants;
mod when_bodies;
mod when_diagnostics;
mod when_patterns;
