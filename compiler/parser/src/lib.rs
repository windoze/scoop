//! Parser stage: source text -> AST.
//!
//! Hand-written lexer + recursive-descent parser for the implemented language
//! surface, including the M21 declaration syntax; see
//! `docs/milestone21/DESIGN.md` and
//! `docs/specs/SCOOP-IMPL-SPEC.md` section 2.1. Depends only on
//! the source and AST data channels.

mod decl;
mod declaration_source;
pub use declaration_source::declaration_source;
mod names;
pub use names::identifier_spelling;
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
mod tests_m36_modifiers;
#[cfg(test)]
mod tests_m37_control_bodies;
#[cfg(test)]
mod tests_m37_interface_delegation;
#[cfg(test)]
mod tests_m37_local_types;
#[cfg(test)]
mod tests_m37_loop_jumps;
#[cfg(test)]
mod tests_m37_modifiers;
#[cfg(test)]
mod tests_m37_names;
#[cfg(test)]
mod tests_m37_receivers;
#[cfg(test)]
mod tests_m37_return_labels;
#[cfg(test)]
mod tests_m37_type_annotations;
#[cfg(test)]
mod tests_m4;
#[cfg(test)]
mod tests_m5;
#[cfg(test)]
mod tests_m55_requirements;
#[cfg(test)]
mod tests_m6;
#[cfg(test)]
mod tests_m60_docs;
#[cfg(test)]
mod tests_m8;
#[cfg(test)]
mod tests_m9;
#[cfg(test)]
mod tests_operator_sources;
#[cfg(test)]
mod tests_shebang;
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
    strict(parse_recovering(source))
}

/// Parses exactly one ordinary function body, including its outer braces.
///
/// Leading and trailing trivia are accepted. All spans refer to the original
/// fragment. Errors inside the body and input remaining outside it prevent a
/// successful AST, just as they do for a complete source file.
pub fn parse_function_body(source: &str) -> Result<scoop_ast::Block, Vec<scoop_ast::Diagnostic>> {
    strict(parse_function_body_recovering(source))
}

/// Parses exactly one complete type using the ordinary type grammar.
///
/// This only parses syntax: names and constraints are resolved by HIR lowering
/// in the caller's semantic context. Leading and trailing trivia are accepted.
pub fn parse_type(source: &str) -> Result<scoop_ast::TypeRef, Vec<scoop_ast::Diagnostic>> {
    strict(parse_type_recovering(source))
}

/// Complete function headers paired with declarations from the same grammar run.
pub type FunctionHeaders = Vec<scoop_syntax::DeclarationData<scoop_ast::FunctionHeader>>;

/// One grammar result, with complete headers independent of strict AST success.
pub type ParsedSyntax<T> = scoop_syntax::Parse<T, FunctionHeaders>;

pub type FileProjections = scoop_syntax::FileProjections<
    Option<scoop_ast::RecoveredFileHeader>,
    scoop_ast::DeclarationFragment,
>;

/// A file projection keeps successful declarations when its strict AST fails.
pub type ParsedFile = scoop_syntax::Parse<scoop_ast::SourceFile, FileProjections>;

/// Parses a file while retaining concrete syntax, including erroneous input.
/// Only an unrepresentable input length prevents a syntax result.
pub fn parse_recovering(source: &str) -> Result<ParsedFile, scoop_source::SourceLengthError> {
    parser::parse_file(source)
}

/// Retains concrete syntax and diagnostics for a function body fragment.
pub fn parse_function_body_recovering(
    source: &str,
) -> Result<ParsedSyntax<scoop_ast::Block>, scoop_source::SourceLengthError> {
    parser::parse_function_body(source)
}

/// Retains concrete syntax and diagnostics for a type fragment.
pub fn parse_type_recovering(
    source: &str,
) -> Result<ParsedSyntax<scoop_ast::TypeRef>, scoop_source::SourceLengthError> {
    parser::parse_type(source)
}

fn strict<T, D>(
    result: Result<scoop_syntax::Parse<T, D>, scoop_source::SourceLengthError>,
) -> Result<T, Vec<scoop_ast::Diagnostic>> {
    result
        .map_err(|error| {
            vec![scoop_ast::Diagnostic::without_span(
                scoop_ast::DiagnosticSeverity::Error,
                0,
                error.to_string(),
            )]
        })?
        .into_ast()
}
