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

/// Severity of one compiler diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
}

impl DiagnosticSeverity {
    fn label(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
        }
    }
}

/// A single compiler diagnostic. `span` is genuinely optional: only
/// driver-level failures (unreadable file, linker failure) lack one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: DiagnosticSeverity,
    /// Index into the driver's input file list; 0-based. Single-file
    /// compiles (and the parser, which sees one file) always use 0.
    pub file: usize,
    pub span: Option<Span>,
    pub message: String,
    /// Related source locations which explain the primary diagnostic.
    /// Notes never replace the primary location and therefore always have
    /// a concrete span.
    pub notes: Vec<DiagnosticNote>,
}

/// One structured related location attached to a [`Diagnostic`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticNote {
    /// Index into the same driver input file list as [`Diagnostic::file`].
    pub file: usize,
    pub span: Span,
    pub message: String,
}

impl Diagnostic {
    pub fn at(span: Span, message: impl Into<String>) -> Self {
        Self::at_file(0, span, message)
    }

    pub fn at_file(file: usize, span: Span, message: impl Into<String>) -> Self {
        Diagnostic {
            severity: DiagnosticSeverity::Error,
            file,
            span: Some(span),
            message: message.into(),
            notes: Vec::new(),
        }
    }

    pub fn warning_at(span: Span, message: impl Into<String>) -> Self {
        Self::warning_at_file(0, span, message)
    }

    pub fn warning_at_file(file: usize, span: Span, message: impl Into<String>) -> Self {
        Diagnostic {
            severity: DiagnosticSeverity::Warning,
            file,
            span: Some(span),
            message: message.into(),
            notes: Vec::new(),
        }
    }

    /// Construct a diagnostic which is not tied to a source span.
    pub fn without_span(
        severity: DiagnosticSeverity,
        file: usize,
        message: impl Into<String>,
    ) -> Self {
        Self {
            severity,
            file,
            span: None,
            message: message.into(),
            notes: Vec::new(),
        }
    }

    /// Attach one related source location while retaining builder-style use.
    pub fn with_note(mut self, note: DiagnosticNote) -> Self {
        self.notes.push(note);
        self
    }

    /// Reattribute a diagnostic produced from a single-source view to its
    /// index in a larger compilation request. Every note from that view is
    /// reattributed together with the primary location.
    pub fn reattribute_single_source(&mut self, file: usize) {
        self.file = file;
        for note in &mut self.notes {
            note.file = file;
        }
    }

    /// Render as `<file>:<line>:<col>: <severity>: <message>`.
    pub fn render(&self, file_name: &str, source: &str) -> String {
        match self.span {
            Some(span) => {
                let (line, col) = line_col(source, span.start);
                format!(
                    "{file_name}:{line}:{col}: {}: {}",
                    self.severity.label(),
                    self.message
                )
            }
            None => format!("{file_name}: {}: {}", self.severity.label(), self.message),
        }
    }
}

impl DiagnosticNote {
    pub fn at(file: usize, span: Span, message: impl Into<String>) -> Self {
        Self {
            file,
            span,
            message: message.into(),
        }
    }

    /// Render as `<file>:<line>:<col>: note: <message>`.
    pub fn render(&self, file_name: &str, source: &str) -> String {
        let (line, col) = line_col(source, self.span.start);
        format!("{file_name}:{line}:{col}: note: {}", self.message)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostic_render_uses_typed_severity_with_and_without_a_span() {
        let error = Diagnostic::at(Span::new(6, 10), "bad value");
        assert_eq!(
            error.render("main.scoop", "first\nvalue"),
            "main.scoop:2:1: error: bad value"
        );

        let warning = Diagnostic::warning_at(Span::new(6, 10), "suspicious value");
        assert_eq!(
            warning.render("main.scoop", "first\nvalue"),
            "main.scoop:2:1: warning: suspicious value"
        );

        let no_span = Diagnostic {
            severity: DiagnosticSeverity::Warning,
            file: 0,
            span: None,
            message: "link warning".to_string(),
            notes: Vec::new(),
        };
        assert_eq!(
            no_span.render("main.scoop", ""),
            "main.scoop: warning: link warning"
        );
    }

    #[test]
    fn diagnostic_notes_are_structured_and_render_their_own_location() {
        let note = DiagnosticNote::at(3, Span::new(13, 17), "declared here");
        let diagnostic = Diagnostic::at(Span::new(0, 3), "cannot access").with_note(note.clone());

        assert_eq!(diagnostic.notes, vec![note.clone()]);
        assert_eq!(
            note.render("library.scoop", "first\nsecond\nthird"),
            "library.scoop:3:1: note: declared here"
        );
    }

    #[test]
    fn reattributes_primary_and_single_source_notes_together() {
        let mut diagnostic = Diagnostic::at(Span::new(0, 3), "duplicate").with_note(
            DiagnosticNote::at(0, Span::new(8, 11), "first declared here"),
        );

        diagnostic.reattribute_single_source(4);

        assert_eq!(diagnostic.file, 4);
        assert_eq!(diagnostic.notes[0].file, 4);
    }
}
