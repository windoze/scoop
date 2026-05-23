//! AST, syntax tokens, lexer, and parser for the Scoop frontend.
//!
//! This stage crate owns the source-level syntax product and depends only on
//! stage-independent base crates plus diagnostic helpers.

#![forbid(unsafe_code)]

pub mod ast;
pub mod parser;
pub mod syntax;

pub use ast::*;

pub mod source {
    pub use scoopc_source::*;
}

pub mod span {
    pub use scoopc_span::*;
}

pub mod ty {
    pub use scoopc_types::*;
}
