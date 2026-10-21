//! Shared source text, byte ranges and diagnostics, independent of syntax and HIR.

mod diagnostic;
mod line_index;
mod position;
mod span;
mod text;
mod version;

pub use diagnostic::{Diagnostic, DiagnosticNote, DiagnosticSeverity};
pub use line_index::LineIndex;
pub use position::{PositionEncoding, PositionError, TextPosition};
pub use span::{SourceLengthError, Span, source_span};
pub use text::SourceText;
pub use version::{Revision, SourceId, SourceVersion};
