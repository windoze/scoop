use std::fmt;

/// Column units used by a source consumer. These do not change source bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PositionEncoding {
    Utf8,
    Utf16,
    Utf32,
}

/// A zero-based line and column in the consumer's chosen encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextPosition {
    pub line: usize,
    pub column: usize,
}

impl TextPosition {
    pub const fn new(line: usize, column: usize) -> Self {
        Self { line, column }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PositionError {
    OffsetOutOfBounds { offset: usize, length: usize },
    LineOutOfBounds { line: usize, line_count: usize },
    InsideCharacter,
    InsideLineEnding,
}

impl fmt::Display for PositionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OffsetOutOfBounds { offset, length } => {
                write!(f, "byte offset {offset} exceeds source length {length}")
            }
            Self::LineOutOfBounds { line, line_count } => {
                write!(f, "line {line} is outside a source with {line_count} lines")
            }
            Self::InsideCharacter => f.write_str("position splits a Unicode character"),
            Self::InsideLineEnding => f.write_str("position splits a CRLF line ending"),
        }
    }
}

impl std::error::Error for PositionError {}
