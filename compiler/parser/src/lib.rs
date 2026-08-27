//! Parser stage: source text -> AST.
//!
//! Hand-written lexer + recursive-descent parser for the M6 subset; see
//! `docs/milestone6/DESIGN.md` section 2.1 and `docs/specs/SCOOP-IMPL-SPEC.md`
//! section 2.1. Depends only on `scoop-ast` (the output data channel).

mod decl;
mod expr;
mod lexer;
mod parser;
mod pattern;
#[cfg(test)]
mod tests;
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

/// Parses a whole source file into an AST.
///
/// The parser is fail-fast: on failure the returned vector holds exactly
/// one [`scoop_ast::Diagnostic`] whose span points at the offending
/// position.
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
    parser::parse_file(source).map_err(|diagnostic| vec![diagnostic])
}
