/// Byte-offset range into a source file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: u32,
    pub end: u32,
}

impl Span {
    pub fn new(start: u32, end: u32) -> Self {
        Span { start, end }
    }
}

/// A single compiler diagnostic. `span` is genuinely optional: only
/// driver-level failures (unreadable file, linker failure) lack one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// Index into the driver's input file list; 0-based. Single-file
    /// compiles (and the parser, which sees one file) always use 0.
    pub file: usize,
    pub span: Option<Span>,
    pub message: String,
}

impl Diagnostic {
    pub fn at(span: Span, message: impl Into<String>) -> Self {
        Diagnostic {
            file: 0,
            span: Some(span),
            message: message.into(),
        }
    }

    /// Render as `<file>:<line>:<col>: error: <message>`.
    pub fn render(&self, file_name: &str, source: &str) -> String {
        match self.span {
            Some(span) => {
                let (line, col) = line_col(source, span.start);
                format!("{file_name}:{line}:{col}: error: {}", self.message)
            }
            None => format!("{file_name}: error: {}", self.message),
        }
    }
}

fn line_col(source: &str, offset: u32) -> (usize, usize) {
    let mut line = 1;
    let mut col = 1;
    for (i, ch) in source.char_indices() {
        if i as u32 >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}
