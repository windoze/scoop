use crate::{SourceText, Span};
use scoop_identity::SourceIdentity;

mod render;

/// Severity of one compiler diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
}

impl DiagnosticSeverity {
    fn level(self) -> annotate_snippets::Level<'static> {
        match self {
            Self::Error => annotate_snippets::Level::ERROR,
            Self::Warning => annotate_snippets::Level::WARNING,
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
    /// Canonical source, bound before the producing HIR context is discarded.
    pub source: Option<Box<SourceIdentity>>,
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
    /// Canonical source, bound before the producing HIR context is discarded.
    pub source: Option<Box<SourceIdentity>>,
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
            source: None,
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
            source: None,
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
            source: None,
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
        self.source = None;
        for note in &mut self.notes {
            note.file = file;
            note.source = None;
        }
    }

    /// Resolve only still-local indices; imported origins already bound by an
    /// earlier stage retain their own source identity.
    pub fn resolve_sources(&mut self, sources: &[SourceIdentity]) {
        if self.source.is_none() {
            self.source = sources.get(self.file).cloned().map(Box::new);
        }
        for note in &mut self.notes {
            if note.source.is_none() {
                note.source = sources.get(note.file).cloned().map(Box::new);
            }
        }
    }

    /// Render a plain diagnostic with its source range underlined.
    pub fn render(&self, file_name: &str, source: &str) -> String {
        self.render_with_code(file_name, source, None, false)
    }

    /// Render from the immutable source shared by this diagnostic batch.
    pub fn render_with_source(&self, file_name: &str, source: &SourceText) -> String {
        self.render(file_name, source.as_str())
    }

    /// Attach the CLI's diagnostic code and optionally style terminal output.
    pub fn render_with_code(
        &self,
        file_name: &str,
        source: &str,
        code: Option<&str>,
        color: bool,
    ) -> String {
        render::render(
            self.severity.level(),
            code,
            &self.message,
            file_name,
            source,
            self.span,
            color,
        )
    }
}

impl DiagnosticNote {
    pub fn at(file: usize, span: Span, message: impl Into<String>) -> Self {
        Self {
            file,
            source: None,
            span,
            message: message.into(),
        }
    }

    /// Render a related diagnostic with its own source range.
    pub fn render(&self, file_name: &str, source: &str) -> String {
        self.render_with_color(file_name, source, false)
    }

    pub fn render_with_source(&self, file_name: &str, source: &SourceText) -> String {
        self.render(file_name, source.as_str())
    }

    pub fn render_with_color(&self, file_name: &str, source: &str, color: bool) -> String {
        render::render(
            annotate_snippets::Level::NOTE,
            None,
            &self.message,
            file_name,
            source,
            Some(self.span),
            color,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostic_render_uses_typed_severity_with_and_without_a_span() {
        let error = Diagnostic::at(Span::new(6, 10), "bad value");
        let rendered = error.render("main.scoop", "first\nvalue");
        assert!(rendered.starts_with("error: bad value\n"), "{rendered}");
        assert!(rendered.contains("--> main.scoop:2:1"), "{rendered}");
        assert!(rendered.contains("2 | value\n  | ^^^^"), "{rendered}");

        let warning = Diagnostic::warning_at(Span::new(6, 10), "suspicious value");
        let rendered = warning.render("main.scoop", "first\nvalue");
        assert!(
            rendered.starts_with("warning: suspicious value\n"),
            "{rendered}"
        );
        assert!(rendered.contains("--> main.scoop:2:1"), "{rendered}");
        assert!(rendered.contains("2 | value\n  | ^^^^"), "{rendered}");

        let no_span = Diagnostic {
            severity: DiagnosticSeverity::Warning,
            file: 0,
            source: None,
            span: None,
            message: "link warning".to_string(),
            notes: Vec::new(),
        };
        assert_eq!(
            no_span.render("main.scoop", ""),
            "warning: link warning\n --> main.scoop"
        );
    }

    #[test]
    fn diagnostic_notes_are_structured_and_render_their_own_location() {
        let note = DiagnosticNote::at(3, Span::new(13, 17), "declared here");
        let diagnostic = Diagnostic::at(Span::new(0, 3), "cannot access").with_note(note.clone());

        assert_eq!(diagnostic.notes, vec![note.clone()]);
        let rendered = note.render("library.scoop", "first\nsecond\nthird");
        assert!(rendered.starts_with("note: declared here\n"), "{rendered}");
        assert!(rendered.contains("--> library.scoop:3:1"), "{rendered}");
        assert!(rendered.contains("3 | third\n  | ^^^^"), "{rendered}");
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
