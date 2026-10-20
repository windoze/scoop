use pulldown_cmark::{Event, LinkType, Options, Parser, Tag};
use scoop_ast::{Diagnostic, DocMarkdown, DocSymbolReference, DocText, Span};

pub(crate) fn options() -> Options {
    Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS
}

pub(crate) fn definitions(text: &DocText) -> Vec<DocText> {
    let parser = Parser::new_ext(&text.text, options());
    let mut definitions = parser
        .reference_definitions()
        .iter()
        .map(|(_, definition)| {
            text.slice(Span::new(
                definition.span.start as u32,
                definition.span.end as u32,
            ))
        })
        .collect::<Vec<_>>();
    definitions.sort_by_key(|definition| definition.span.start);
    definitions
}

pub(crate) fn prepare(
    text: DocText,
    definitions: &[DocText],
    warnings: &mut Vec<Diagnostic>,
) -> DocMarkdown {
    // Definitions may follow a tag and still apply to the document's body.
    let mut input = text.text.clone();
    for definition in definitions {
        input.push_str("\n\n");
        input.push_str(&definition.text);
    }
    let parser = Parser::new_with_broken_link_callback(
        &input,
        options(),
        Some(|_: pulldown_cmark::BrokenLink<'_>| Some(("".into(), "".into()))),
    );
    let mut references = Vec::new();
    for (event, range) in parser.into_offset_iter() {
        if range.end > text.text.len() {
            continue;
        }
        let Event::Start(Tag::Link { link_type, id, .. }) = event else {
            continue;
        };
        if !matches!(
            link_type,
            LinkType::ReferenceUnknown | LinkType::CollapsedUnknown | LinkType::ShortcutUnknown
        ) {
            continue;
        }
        let link_range = Span::new(range.start as u32, range.end as u32);
        // The reference label is the final spelling in a full reference and
        // the sole spelling in a shortcut/collapsed reference.
        let target = text.text[range.clone()].rfind(id.as_ref()).map(|offset| {
            Span::new(
                (range.start + offset) as u32,
                (range.start + offset + id.len()) as u32,
            )
        });
        if let Some(path) = target.and_then(|target| crate::paths::parse(&text, target)) {
            references.push(DocSymbolReference {
                range: link_range,
                path,
            });
        } else {
            warnings.push(Diagnostic::warning_at(
                text.source_span(link_range),
                format!("invalid documentation symbol `{id}`"),
            ));
        }
    }
    DocMarkdown {
        text,
        definitions: definitions.to_vec(),
        references,
    }
}
