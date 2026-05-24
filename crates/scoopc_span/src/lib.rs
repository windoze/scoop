//! Stage-independent span and diagnostic coordinate foundations.
//!
//! This base crate owns source span primitives that every compiler stage and
//! fact crate may share. It must not depend on `scoopc`, stage crates, fact
//! crates, backend crates, or repository tools.

#![forbid(unsafe_code)]

use std::fmt;

/// UTF-8 byte-offset range within one source buffer.
#[derive(Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    pub fn synthetic_prelude() -> Self {
        Self::new(0, 0)
    }

    pub fn len(self) -> usize {
        self.end.saturating_sub(self.start)
    }

    pub fn is_empty(self) -> bool {
        self.start >= self.end
    }
}

impl fmt::Debug for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}..{}", self.start, self.end)
    }
}

impl From<Span> for miette::SourceSpan {
    fn from(span: Span) -> Self {
        (span.start, span.len()).into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn span_len_saturates_when_reversed() {
        assert_eq!(Span::new(2, 5).len(), 3);
        assert_eq!(Span::new(5, 2).len(), 0);
    }

    #[test]
    fn span_debug_matches_byte_range() {
        assert_eq!(format!("{:?}", Span::new(4, 9)), "4..9");
    }
}
