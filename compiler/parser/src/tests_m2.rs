//! Unit tests for the M2 syntax added on top of M1: struct declarations,
//! `val` / `var`, assignment, `if` / `while` / nested blocks, the full
//! expression grammar (precedence, postfix access, unit/tuple/paren
//! disambiguation), and every M2 "not supported" diagnostic.

use scoop_ast::{
    AssignTarget, BinOp, Decl, Expr, FieldSelector, Pattern, Span, StatementKind, TypeRefKind, UnOp,
};

use crate::tests::{block_body, err, ok, only_function};

/// Parses `expr` as the initializer of `val x = <expr>` and returns it.
pub(crate) fn init_expr(expr: &str) -> Expr {
    let file = ok(&format!("fun main() {{\n    val x = {expr}\n}}\n"));
    let function = only_function(&file);
    let StatementKind::ValDecl(decl) = &block_body(function).statements[0].kind else {
        panic!("expected a val declaration");
    };
    decl.init.clone()
}

/// Dumps `fun main() { <statement> }`, stripping the wrapper and the
/// function body's base indentation.
pub(crate) fn stmt_dump(statement: &str) -> String {
    let file = ok(&format!("fun main() {{\n    {statement}\n}}\n"));
    let dump = scoop_ast::dump(&file);
    let body = dump
        .strip_prefix("SourceFile\n  RootPackage\n  fun main()\n")
        .expect("dump starts with the function header");
    let mut out = String::new();
    for line in body.lines() {
        out.push_str(line.strip_prefix("    ").unwrap_or(line));
        out.push('\n');
    }
    out
}

mod declarations;
mod diagnostics;
mod example;
mod expressions;
mod statements;
mod tuples;
