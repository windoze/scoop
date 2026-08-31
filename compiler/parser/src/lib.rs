//! Parser stage: source text -> AST.
//!
//! Hand-written lexer + recursive-descent parser through the M11 subset; see
//! `docs/milestone11/DESIGN.md` (function types and values) and
//! `docs/specs/SCOOP-IMPL-SPEC.md` section 2.1. Depends only on
//! `scoop-ast` (the output data channel).

mod decl;
mod expr;
mod lexer;
mod parser;
mod pattern;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_m10;
#[cfg(test)]
mod tests_m11;
#[cfg(test)]
mod tests_m12;
#[cfg(test)]
mod tests_m2;
#[cfg(test)]
mod tests_m3;
#[cfg(test)]
mod tests_m4;
#[cfg(test)]
mod tests_m5;
#[cfg(test)]
mod tests_m6;
#[cfg(test)]
mod tests_m8;
#[cfg(test)]
mod tests_m9;

/// Parses a whole source file into an AST.
///
/// Syntax and lexical recovery collect independent diagnostics in source
/// order. If any error is found, the partial AST is discarded and the
/// returned vector contains every diagnostic recovered from this file.
///
/// ```
/// let file = scoop_parser::parse("fun main() {\n    println(\"hi\")\n}\n").unwrap();
/// assert_eq!(file.declarations.len(), 1);
/// let scoop_ast::Decl::Function(main) = &file.declarations[0] else {
///     panic!("expected a function");
/// };
/// assert_eq!(main.name.text, "main");
/// ```
pub fn parse(source: &str) -> Result<scoop_ast::SourceFile, Vec<scoop_ast::Diagnostic>> {
    parser::parse_file(source)
}
