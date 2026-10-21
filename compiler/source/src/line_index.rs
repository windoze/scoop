use std::ops::Range;

use crate::{PositionEncoding, PositionError, TextPosition};

#[derive(Debug)]
struct WideChar {
    start: usize,
    end: usize,
    utf16: usize,
}

impl WideChar {
    fn width(&self, encoding: PositionEncoding) -> usize {
        match encoding {
            PositionEncoding::Utf8 => self.end - self.start,
            PositionEncoding::Utf16 => self.utf16,
            PositionEncoding::Utf32 => 1,
        }
    }

    fn excess_bytes(&self, encoding: PositionEncoding) -> usize {
        self.end - self.start - self.width(encoding)
    }
}

/// A reusable index for one immutable source, including non-ASCII boundaries.
#[derive(Debug)]
pub struct LineIndex {
    length: usize,
    starts: Vec<usize>,
    ends: Vec<usize>,
    wide: Vec<WideChar>,
}

impl LineIndex {
    pub(crate) fn new(source: &str, universal_newlines: bool) -> Self {
        let mut starts = vec![0];
        let mut ends = Vec::new();
        let mut wide = Vec::new();
        let mut chars = source.char_indices().peekable();
        while let Some((offset, ch)) = chars.next() {
            if ch == '\n' || (universal_newlines && ch == '\r') {
                ends.push(offset);
                let mut next = offset + 1;
                if ch == '\r' && chars.peek().is_some_and(|(_, ch)| *ch == '\n') {
                    chars.next();
                    next += 1;
                }
                starts.push(next);
            } else if !ch.is_ascii() {
                wide.push(WideChar {
                    start: offset,
                    end: offset + ch.len_utf8(),
                    utf16: ch.len_utf16(),
                });
            }
        }
        ends.push(source.len());
        Self {
            length: source.len(),
            starts,
            ends,
            wide,
        }
    }

    pub fn line_count(&self) -> usize {
        self.starts.len()
    }

    /// Excludes the terminator; a trailing terminator creates an empty line.
    pub fn line_range(&self, line: usize) -> Result<Range<usize>, PositionError> {
        match (self.starts.get(line), self.ends.get(line)) {
            (Some(start), Some(end)) => Ok(*start..*end),
            _ => Err(PositionError::LineOutOfBounds {
                line,
                line_count: self.line_count(),
            }),
        }
    }

    pub fn position(
        &self,
        offset: usize,
        encoding: PositionEncoding,
    ) -> Result<TextPosition, PositionError> {
        if offset > self.length {
            return Err(PositionError::OffsetOutOfBounds {
                offset,
                length: self.length,
            });
        }
        let line = self.starts.partition_point(|start| *start <= offset) - 1;
        if offset > self.ends[line] {
            return Err(PositionError::InsideLineEnding);
        }
        let mut column = offset - self.starts[line];
        for ch in self.wide_in(self.starts[line]..offset) {
            if offset < ch.end {
                return Err(PositionError::InsideCharacter);
            }
            column -= ch.excess_bytes(encoding);
        }
        Ok(TextPosition::new(line, column))
    }

    /// LSP columns beyond a line's content are clamped to that line's end.
    pub fn offset(
        &self,
        position: TextPosition,
        encoding: PositionEncoding,
    ) -> Result<usize, PositionError> {
        let line = self.line_range(position.line)?;
        let wide = self.wide_in(line.clone());
        let excess: usize = wide.iter().map(|ch| ch.excess_bytes(encoding)).sum();
        let column = position.column.min(line.len() - excess);
        let mut excess = 0;
        for ch in wide {
            let start_column = ch.start - line.start - excess;
            if column <= start_column {
                break;
            }
            if column < start_column + ch.width(encoding) {
                return Err(PositionError::InsideCharacter);
            }
            excess += ch.excess_bytes(encoding);
        }
        Ok(line.start + column + excess)
    }

    /// Preserve the public diagnostic renderer's one-based, LF-only behavior,
    /// including its rounding of an externally supplied partial-character span.
    pub(crate) fn diagnostic_position(&self, offset: usize) -> (usize, usize) {
        let offset = offset.min(self.length);
        let line = self.starts.partition_point(|start| *start <= offset) - 1;
        let excess: usize = self
            .wide_in(self.starts[line]..offset)
            .iter()
            .map(|ch| offset.min(ch.end) - ch.start - 1)
            .sum();
        (line + 1, offset - self.starts[line] - excess + 1)
    }

    fn wide_in(&self, range: Range<usize>) -> &[WideChar] {
        let start = self.wide.partition_point(|ch| ch.start < range.start);
        let end = self.wide.partition_point(|ch| ch.start < range.end);
        &self.wide[start..end]
    }
}
