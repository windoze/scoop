use std::fmt;

/// Half-open UTF-8 byte range in one source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    pub start: u32,
    pub end: u32,
}

impl Span {
    pub const fn new(start: u32, end: u32) -> Self {
        Self { start, end }
    }
}

/// Check the actual offset representation before lexing the source.
pub fn source_span(byte_length: usize) -> Result<Span, SourceLengthError> {
    u32::try_from(byte_length)
        .map(|end| Span::new(0, end))
        .map_err(|_| SourceLengthError { byte_length })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceLengthError {
    pub byte_length: usize,
}

impl fmt::Display for SourceLengthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "source length {} exceeds the {}-byte source span representation",
            self.byte_length,
            u32::MAX,
        )
    }
}

impl std::error::Error for SourceLengthError {}
