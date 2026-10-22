//! AST definitions: the data channel between parser and HIR.
//!
//! See `docs/specs/SCOOP-IMPL-SPEC.md` section 2.1 and
//! `docs/milestone2/DESIGN.md` for the M2 subset.

mod annotations;
mod context;
mod declarations;
mod documentation;
mod dump;
mod expressions;
mod floating;
mod headers;
pub use floating::*;
mod declaration_source;
mod integer;
mod parsed_sources;
mod patterns;
mod recovered;
mod recovered_body;
pub use recovered_body::*;
mod recovered_when;
pub use recovered_when::*;
mod recovered_call;
pub use recovered_call::*;
mod statements;
mod types;

pub use annotations::*;
pub use context::*;
pub use declaration_source::*;
pub use declarations::*;
pub use documentation::*;
pub use dump::{dump, dump_pattern};
pub use expressions::*;
pub use headers::*;
pub use integer::*;
pub use parsed_sources::*;
pub use patterns::*;
pub use recovered::*;
pub use scoop_source::{Diagnostic, DiagnosticNote, DiagnosticSeverity, Span};
pub use statements::*;
pub use types::*;

mod when;
pub use when::*;
