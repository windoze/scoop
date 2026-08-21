//! Parser stage: source text -> AST.
//!
//! Hand-written lexer + recursive-descent parser for the M1 subset; see
//! `docs/milestone1/DESIGN.md` section 2.2 and `docs/specs/SCOOP-IMPL-SPEC.md`
//! section 2.1. Depends only on `scoop-ast` (the output data channel).

mod lexer;
mod parser;
#[cfg(test)]
mod tests;

/// Parses a whole source file into an AST.
///
/// M1 is fail-fast: on failure the returned vector holds exactly one
/// [`scoop_ast::Diagnostic`] whose span points at the offending position.
///
/// ```
/// let file = scoop_parser::parse("fun main() {\n    println(\"hi\")\n}\n").unwrap();
/// assert_eq!(file.functions.len(), 1);
/// assert_eq!(file.functions[0].name.text, "main");
/// ```
pub fn parse(source: &str) -> Result<scoop_ast::SourceFile, Vec<scoop_ast::Diagnostic>> {
    parser::parse_file(source).map_err(|diagnostic| vec![diagnostic])
}
