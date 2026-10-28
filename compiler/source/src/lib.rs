//! Shared source text, byte ranges and diagnostics, independent of syntax and HIR.

mod debug;
mod diagnostic;
pub use debug::{DebugInlineSite, DebugLocation, DebugOrigin, DebugRecords};
mod edit;
mod line_index;
mod position;
mod span;
mod text;
mod version;

pub use diagnostic::{Diagnostic, DiagnosticNote, DiagnosticSeverity};
pub use edit::SourceEdit;
pub use line_index::LineIndex;
pub use position::{PositionEncoding, PositionError, TextPosition};
pub use span::{SourceLengthError, Span, source_span};
pub use text::SourceText;
pub use version::{Revision, SourceId, SourceVersion};
