use std::sync::{Arc, OnceLock};

use crate::{LineIndex, SourceLengthError, Span, source_span};

/// One immutable text value. Clones share both text and lazily built indices.
#[derive(Debug, Clone)]
pub struct SourceText(Arc<TextData>);

#[derive(Debug)]
struct TextData {
    text: String,
    editor_lines: OnceLock<LineIndex>,
    compiler_lines: OnceLock<LineIndex>,
}

impl SourceText {
    pub fn new(text: String) -> Self {
        Self(Arc::new(TextData {
            text,
            editor_lines: OnceLock::new(),
            compiler_lines: OnceLock::new(),
        }))
    }

    pub fn as_str(&self) -> &str {
        &self.0.text
    }

    pub fn len(&self) -> usize {
        self.0.text.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.text.is_empty()
    }

    pub fn span(&self) -> Result<Span, SourceLengthError> {
        source_span(self.len())
    }

    /// LSP line boundaries include LF, CRLF and CR; raw bytes stay unchanged.
    pub fn line_index(&self) -> &LineIndex {
        self.0
            .editor_lines
            .get_or_init(|| LineIndex::new(self.as_str(), true))
    }

    /// Compiler diagnostics retain their original LF-only line convention.
    pub(crate) fn diagnostic_position(&self, offset: u32) -> (usize, usize) {
        self.0
            .compiler_lines
            .get_or_init(|| LineIndex::new(self.as_str(), false))
            .diagnostic_position(offset as usize)
    }
}

impl PartialEq for SourceText {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0) || self.as_str() == other.as_str()
    }
}

impl Eq for SourceText {}

impl From<String> for SourceText {
    fn from(text: String) -> Self {
        Self::new(text)
    }
}
