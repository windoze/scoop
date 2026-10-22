//! Markdown presentation of the same bound comments used by generated pages.

use pulldown_cmark::{Event, Parser, Tag, TagEnd};
use scoop_hir::{
    DocumentationContent, DocumentationLink, DocumentationMarkdown, DocumentationSymbol,
    DocumentationTagKind,
};

pub fn hover_markdown(
    content: DocumentationContent<'_>,
    mut resolve: impl FnMut(DocumentationSymbol) -> Option<String>,
) -> String {
    let comment = match content {
        DocumentationContent::Markdown(markdown) => return render(markdown, &mut resolve),
        DocumentationContent::Comment(comment) => comment,
    };
    let mut result = render(&comment.body, &mut resolve);
    for tag in &comment.tags {
        let (kind, label) = match &tag.kind {
            DocumentationTagKind::Param(_) => ("param", Some(tag.label.as_str())),
            DocumentationTagKind::Property(_) => ("property", Some(tag.label.as_str())),
            DocumentationTagKind::Throws(_) => ("throws", Some(tag.label.as_str())),
            DocumentationTagKind::See(_) => ("see", Some(tag.label.as_str())),
            DocumentationTagKind::Constructor(_) => ("constructor", None),
            DocumentationTagKind::Receiver(_) => ("receiver", None),
            DocumentationTagKind::Return(_) => ("return", None),
            DocumentationTagKind::Since => ("since", None),
        };
        if !result.is_empty() {
            result.push_str("\n\n");
        }
        result.push_str(&format!("**@{kind}**"));
        if let Some(label) = label {
            result.push(' ');
            result.push_str(&escape(label));
        }
        let body = render(&tag.body, &mut resolve);
        if !body.is_empty() {
            result.push_str(" — ");
            result.push_str(&body);
        }
    }
    result
}

fn render(
    markdown: &DocumentationMarkdown,
    resolve: &mut impl FnMut(DocumentationSymbol) -> Option<String>,
) -> String {
    let text = &markdown.text.text;
    let mut output = String::new();
    let mut offset = 0;
    for reference in &markdown.references {
        let start = reference.range.start as usize;
        let end = reference.range.end as usize;
        output.push_str(&text[offset..start]);
        let label = label(&text[start..end]);
        let destination = match &reference.resolution {
            DocumentationLink::Symbols(symbols) if symbols.len() == 1 => resolve(*symbols.first()),
            DocumentationLink::Symbols(_) | DocumentationLink::Unresolved => None,
            DocumentationLink::Url(url) => Some(url.clone()),
        };
        match destination {
            Some(url) => output.push_str(&format!("[{label}](<{url}>)")),
            None => output.push_str(label),
        }
        offset = end;
    }
    output.push_str(&text[offset..]);
    for definition in &markdown.definitions {
        output.push_str("\n\n");
        output.push_str(&definition.text);
    }
    output
}

/// Pulldown supplies the displayed label's exact ranges, including inline markup.
fn label(reference: &str) -> &str {
    let parser = Parser::new_with_broken_link_callback(
        reference,
        crate::markdown::options(),
        Some(|_: pulldown_cmark::BrokenLink<'_>| Some(("".into(), "".into()))),
    );
    let mut content = None::<std::ops::Range<usize>>;
    let mut inside = false;
    for (event, range) in parser.into_offset_iter() {
        match event {
            Event::Start(Tag::Link { .. }) => inside = true,
            Event::End(TagEnd::Link) => break,
            _ if inside => match &mut content {
                Some(content) => {
                    content.start = content.start.min(range.start);
                    content.end = content.end.max(range.end);
                }
                None => content = Some(range),
            },
            _ => {}
        }
    }
    content.map_or("", |range| &reference[range])
}

fn escape(text: &str) -> String {
    text.chars()
        .flat_map(|character| {
            "\\`*_{}[]<>"
                .contains(character)
                .then_some('\\')
                .into_iter()
                .chain(std::iter::once(character))
        })
        .collect()
}
