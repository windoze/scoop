//! Lossless syntax data shared by compiler and editor consumers.

mod declaration;
mod kind;
mod parse;
mod tree;

pub use declaration::{DeclarationIndex, DeclarationName, DeclarationSyntax};
pub use kind::SyntaxKind;
pub use parse::{ExpectedSyntax, MissingSyntax, Parse};
pub use rowan::{GreenNode, GreenNodeBuilder, TextRange, TextSize, TokenAtOffset, WalkEvent};
pub use tree::{ScoopLanguage, SyntaxElement, SyntaxNode, SyntaxToken};
