//! AST definitions: the data channel between parser and HIR.
//!
//! See `docs/specs/SCOOP-IMPL-SPEC.md` section 2.1 and
//! `docs/milestone2/DESIGN.md` for the M2 subset.

mod annotations;
mod context;
mod declarations;
mod dump;
mod expressions;
mod floating;
mod headers;
pub use floating::*;
mod integer;
mod parsed_sources;
mod source;
mod statements;
mod types;

pub use annotations::*;
pub use context::*;
pub use declarations::*;
pub use dump::{dump, dump_pattern};
pub use expressions::*;
pub use headers::*;
pub use integer::*;
pub use parsed_sources::*;
pub use source::{Diagnostic, DiagnosticNote, DiagnosticSeverity, Span};
pub use statements::*;
pub use types::*;

mod when;
pub use when::*;
