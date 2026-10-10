//! Parser stage: source text -> AST.
//!
//! Hand-written lexer + recursive-descent parser for the implemented language
//! surface, including the M21 declaration syntax; see
//! `docs/milestone21/DESIGN.md` and
//! `docs/specs/SCOOP-IMPL-SPEC.md` section 2.1. Depends only on
//! `scoop-ast` (the output data channel).

mod decl;
mod expr;
mod header;
mod lexer;
mod parser;
mod pattern;
mod source_input;
mod stmt;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_m10;
#[cfg(test)]
mod tests_m11;
#[cfg(test)]
mod tests_m12;
#[cfg(test)]
mod tests_m14;
#[cfg(test)]
mod tests_m17;
#[cfg(test)]
mod tests_m18;
#[cfg(test)]
mod tests_m19;
#[cfg(test)]
mod tests_m2;
#[cfg(test)]
mod tests_m20;
#[cfg(test)]
mod tests_m21;
#[cfg(test)]
mod tests_m22_control_flow;
#[cfg(test)]
mod tests_m22_copy_update;
#[cfg(test)]
mod tests_m22_integer_literals;
#[cfg(test)]
mod tests_m22_type_aliases;
#[cfg(test)]
mod tests_m23_header_contract;
#[cfg(test)]
mod tests_m23_headers;
#[cfg(test)]
mod tests_m24;
#[cfg(test)]
mod tests_m26_interpolation;
#[cfg(test)]
mod tests_m27_context;
#[cfg(test)]
mod tests_m3;
#[cfg(test)]
mod tests_m30_floating;
#[cfg(test)]
mod tests_m35;
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
mod ty;

pub use source_input::{
    CurrentConeSourceInput, IdentifiedSourceInput, ParseAllDiagnostic, ParseCurrentConeError,
    ParseCurrentConeInputError, ParserDiagnosticContext, parse_all, parse_current_cone,
};

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
