//! AST definitions: the data channel between parser and HIR.
//!
//! See `docs/specs/SCOOP-IMPL-SPEC.md` section 2.1 and
//! `docs/milestone2/DESIGN.md` for the M2 subset.

mod declarations;
mod dump;
mod expressions;
mod headers;
mod integer;
mod parsed_sources;
mod source;
mod statements;
mod types;

pub use declarations::*;
pub use dump::{dump, dump_pattern};
pub use expressions::*;
pub use headers::*;
pub use integer::*;
pub use parsed_sources::*;
pub use source::{Diagnostic, DiagnosticSeverity, Span};
pub use statements::*;
pub use types::*;
