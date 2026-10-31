use annotate_snippets::{AnnotationKind, Group, Level, Origin, Renderer, Snippet};
use std::borrow::Cow;

use crate::Span;

pub(super) fn render(
    level: Level<'_>,
    code: Option<&str>,
    message: &str,
    file_name: &str,
    source: &str,
    span: Option<Span>,
    color: bool,
) -> String {
    let mut title = level.primary_title(message);
    if let Some(code) = code {
        title = title.id(code);
    }
    let mut group = Group::with_title(title);
    if let Some(span) = span {
        let range = span.start as usize..span.end as usize;
        if source.get(range.clone()).is_some() {
            // The renderer omits a trailing empty line. Retain that real EOF
            // line when a parser diagnostic points just after the final LF.
            let source =
                if range.is_empty() && range.start == source.len() && source.ends_with('\n') {
                    Cow::Owned(format!("{source}\n"))
                } else {
                    Cow::Borrowed(source)
                };
            group = group.element(
                Snippet::source(source)
                    .path(file_name)
                    .fold(true)
                    .annotation(AnnotationKind::Primary.span(range)),
            );
        } else {
            group = group.element(Origin::path(format!(
                "{file_name}:bytes {}..{}",
                span.start, span.end
            )));
        }
    } else if !file_name.is_empty() {
        group = group.element(Origin::path(file_name));
    }
    let renderer = if color {
        Renderer::styled()
    } else {
        Renderer::plain()
    };
    renderer.render(&[group])
}

#[cfg(test)]
mod tests {
    use crate::Diagnostic;

    use super::*;

    #[test]
    fn source_ranges_align_after_tabs_and_wide_characters() {
        let source = "\t中x(100)\n";
        let rendered =
            Diagnostic::at(Span::new(4, 5), "not callable").render("unicode.scoop", source);
        assert!(
            rendered.contains("1 |     中x(100)\n  |       ^"),
            "{rendered}"
        );
    }

    #[test]
    fn end_of_file_is_marked_even_when_the_range_is_empty() {
        for (source, location) in [
            ("", "1:1"),
            ("val x =", "1:8"),
            ("val x =\n", "2:1"),
            ("val x =\r\n", "2:1"),
        ] {
            let end = source.len() as u32;
            let rendered = Diagnostic::at(Span::new(end, end), "expected expression")
                .render("main.scoop", source);
            assert!(rendered.contains("expected expression"), "{rendered}");
            assert!(
                rendered.contains(&format!("main.scoop:{location}")),
                "{rendered}"
            );
            assert!(rendered.contains('^'), "{rendered}");
        }
    }

    #[test]
    fn multiline_ranges_and_crlf_keep_source_line_numbers() {
        let source = "before\r\nfirst(\r\n    second\r\n)\r\nafter\r\n";
        let start = source.find("first").unwrap() as u32;
        let end = source.find(')').unwrap() as u32 + 1;
        let rendered = Diagnostic::at(Span::new(start, end), "bad expression")
            .render("multiline.scoop", source);
        assert!(rendered.contains("multiline.scoop:2:1"), "{rendered}");
        for line in ["2 |", "first(", "3 |", "second", "4 |"] {
            assert!(rendered.contains(line), "{rendered}");
        }
        assert!(rendered.contains('^'), "{rendered}");
        assert!(!rendered.contains('\r'), "{rendered:?}");
    }

    #[test]
    fn invalid_ranges_keep_byte_offsets_without_rendering_a_false_snippet() {
        for span in [Span::new(1, 2), Span::new(0, 30), Span::new(3, 0)] {
            let rendered = Diagnostic::at(span, "original error").render("main.scoop", "中");
            assert!(rendered.contains("error: original error"), "{rendered}");
            assert!(
                rendered.contains(&format!("main.scoop:bytes {}..{}", span.start, span.end)),
                "{rendered}"
            );
            assert!(!rendered.contains('^'), "{rendered}");
        }
    }

    #[test]
    fn color_is_explicit_and_plain_output_has_no_terminal_controls() {
        let diagnostic = Diagnostic::at(Span::new(0, 1), "not callable");
        let plain = diagnostic.render_with_code("main.scoop", "x()", Some("E_CALL"), false);
        let styled = diagnostic.render_with_code("main.scoop", "x()", Some("E_CALL"), true);
        assert!(plain.starts_with("error[E_CALL]: not callable\n"));
        assert!(!plain.contains('\u{1b}'));
        assert!(styled.contains('\u{1b}'));
    }
}
