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
}

impl Diagnostic {
    pub fn at(span: Span, message: impl Into<String>) -> Self {
        Diagnostic {
            severity: DiagnosticSeverity::Error,
            file: 0,
            span: Some(span),
            message: message.into(),
        }
    }

    pub fn warning_at(span: Span, message: impl Into<String>) -> Self {
        Diagnostic {
            severity: DiagnosticSeverity::Warning,
            file: 0,
            span: Some(span),
            message: message.into(),
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
        };
        assert_eq!(
            no_span.render("main.scoop", ""),
            "main.scoop: warning: link warning"
        );
    }
}
