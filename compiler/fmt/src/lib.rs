//! Source-preserving formatting of a successfully parsed syntax tree.

mod layout;
mod render;
mod spacing;
mod syntax;

use scoop_syntax::SyntaxNode;

/// Format the lossless tree from a successful parse of a complete source file.
/// The caller owns parsing, diagnostics, and file I/O. No tokens are rewritten.
pub fn format(syntax: &SyntaxNode) -> String {
    let input = syntax::Input::new(syntax);
    if input.tokens.is_empty() {
        return String::new();
    }
    let gaps = spacing::gaps(&input);
    let document = layout::document(&input, &gaps);
    render::render(document, input.crlf)
}
