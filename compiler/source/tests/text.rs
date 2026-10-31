use scoop_source::{Diagnostic, DiagnosticNote, SourceText, Span, source_span};

#[test]
fn cloned_text_reuses_its_index_and_old_values_stay_immutable() {
    let text = SourceText::new("hello\r\n世界".to_owned());
    let clone = text.clone();
    assert!(std::ptr::eq(text.line_index(), clone.line_index()));
    let changed = SourceText::new("changed".to_owned());
    assert_eq!(clone.as_str(), "hello\r\n世界");
    assert_ne!(clone, changed);
    assert_eq!(text.span(), Ok(Span::new(0, 13)));
}

#[test]
fn source_equality_is_independent_of_cached_indices() {
    let first = SourceText::new("one\rtwo\n中😀".to_owned());
    let second = SourceText::new(first.as_str().to_owned());
    first.line_index();
    Diagnostic::at(Span::new(4, 5), "bad value").render_with_source("main.scoop", &second);
    assert_eq!(first, second);
}

#[test]
fn shared_source_renders_the_same_ranges_and_related_locations() {
    let text = SourceText::new("one\r\n中😀\rnext\nlast".to_owned());
    for offset in 0..text.len() as u32 + 3 {
        let span = Span::new(offset, offset);
        let diagnostic = Diagnostic::at(span, "bad value");
        let note = DiagnosticNote::at(0, span, "declared here");
        assert_eq!(
            diagnostic.render_with_source("main.scoop", &text),
            diagnostic.render("main.scoop", text.as_str())
        );
        assert_eq!(
            note.render_with_source("main.scoop", &text),
            note.render("main.scoop", text.as_str())
        );
    }
    let rendered =
        Diagnostic::at(Span::new(14, 15), "bad value").render_with_source("main.scoop", &text);
    assert!(rendered.contains("main.scoop:2:5"), "{rendered}");
    assert!(rendered.contains('^'), "{rendered}");
}

#[test]
fn source_span_checks_its_representation_without_allocating_source_text() {
    assert_eq!(source_span(0), Ok(Span::new(0, 0)));
    assert_eq!(source_span(u32::MAX as usize), Ok(Span::new(0, u32::MAX)));
    #[cfg(target_pointer_width = "64")]
    {
        let error = source_span(u32::MAX as usize + 1).unwrap_err();
        assert_eq!(error.byte_length, 4_294_967_296);
        assert!(error.to_string().contains("source span representation"));
    }
}
