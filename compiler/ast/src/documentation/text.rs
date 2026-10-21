//! Normalized Markdown with a byte-exact map back to the source comment.

use crate::Span;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocTextMapping {
    pub normalized: Span,
    pub source: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocText {
    pub text: String,
    pub span: Span,
    pub mapping: Vec<DocTextMapping>,
}

impl DocText {
    /// Maps a normalized range, including a collapsed CRLF or inserted gap.
    pub fn source_span(&self, range: Span) -> Span {
        let start = self
            .mapping
            .iter()
            .find(|part| part.normalized.start <= range.start && range.start < part.normalized.end);
        let end = self
            .mapping
            .iter()
            .rev()
            .find(|part| part.normalized.start < range.end && range.end <= part.normalized.end);
        let start = start
            .map(|part| part.source_offset(range.start, false))
            .unwrap_or(self.span.end);
        let end = end
            .map(|part| part.source_offset(range.end, true))
            .unwrap_or(start);
        Span::new(start, end.max(start))
    }

    pub fn slice(&self, range: Span) -> Self {
        let mapping = self
            .mapping
            .iter()
            .filter_map(|part| {
                let start = part.normalized.start.max(range.start);
                let end = part.normalized.end.min(range.end);
                (start < end).then(|| DocTextMapping {
                    normalized: Span::new(start - range.start, end - range.start),
                    source: Span::new(
                        part.source_offset(start, false),
                        part.source_offset(end, true),
                    ),
                })
            })
            .collect();
        Self {
            text: self.text[range.start as usize..range.end as usize].into(),
            span: self.source_span(range),
            mapping,
        }
    }
}

impl DocTextMapping {
    fn source_offset(&self, offset: u32, end: bool) -> u32 {
        if self.normalized.end - self.normalized.start == self.source.end - self.source.start {
            self.source.start + offset - self.normalized.start
        } else if end {
            self.source.end
        } else {
            self.source.start
        }
    }
}
