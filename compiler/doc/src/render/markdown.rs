use super::*;
use pulldown_cmark::{Event, Parser, Tag, TagEnd};

pub(super) fn render(markdown: &Markdown, page: &str, catalog: &SiteCatalog<'_>) -> String {
    let mut input = markdown.text.clone();
    for definition in &markdown.definitions {
        input.push_str("\n\n");
        input.push_str(definition);
    }
    html(&input, true, |range, destination| {
        match markdown
            .references
            .iter()
            .find(|r| r.range.start as usize == range.start && r.range.end as usize == range.end)
        {
            Some(reference) => target(&reference.target, page, catalog),
            None => safe_url(destination).map(str::to_owned),
        }
    })
}

pub(super) fn readme(input: &ReadmeInput, page: &str) -> String {
    html(&input.text, false, |_, destination| {
        match input.resources.get(destination) {
            Some(path) => path.as_ref().map(|path| routes::relative(page, path)),
            None => safe_url(destination).map(str::to_owned),
        }
    })
}

pub(super) fn target(link: &LinkTarget, page: &str, catalog: &SiteCatalog<'_>) -> Option<String> {
    match link {
        LinkTarget::Symbols(symbols) => catalog
            .symbol_link(symbols)
            .map(|url| routes::relative(page, &url)),
        LinkTarget::Url(url) => safe_url(url).map(str::to_owned),
        LinkTarget::Unresolved => None,
    }
}

fn html(
    input: &str,
    symbol_references: bool,
    mut resolve: impl FnMut(std::ops::Range<usize>, &str) -> Option<String>,
) -> String {
    let parser = Parser::new_with_broken_link_callback(
        input,
        crate::markdown::options(),
        Some(move |_: pulldown_cmark::BrokenLink<'_>| {
            symbol_references.then(|| ("".into(), "".into()))
        }),
    );
    let mut links = Vec::new();
    let mut images = Vec::new();
    let events = parser.into_offset_iter().map(|(event, range)| match event {
        Event::Html(text) | Event::InlineHtml(text) => Event::Text(text),
        Event::Start(Tag::Link { link_type, dest_url, title, id }) => {
            let destination = resolve(range, &dest_url);
            links.push(destination.is_some());
            match destination {
                Some(destination) => Event::Start(Tag::Link {
                    link_type, dest_url: destination.into(), title, id,
                }),
                None => Event::InlineHtml(
                    "<span class=\"unavailable\" title=\"No generated documentation or usable URL for this target\">".into(),
                ),
            }
        }
        Event::End(TagEnd::Link) if links.pop() == Some(false) => Event::InlineHtml("</span>".into()),
        Event::Start(Tag::Image { link_type, dest_url, title, id }) => {
            let destination = resolve(range, &dest_url);
            images.push(destination.is_some());
            match destination {
                Some(destination) => Event::Start(Tag::Image {
                    link_type, dest_url: destination.into(), title, id,
                }),
                None => Event::InlineHtml("<span class=\"unavailable\">".into()),
            }
        }
        Event::End(TagEnd::Image) if images.pop() == Some(false) => Event::InlineHtml("</span>".into()),
        other => other,
    });
    let mut html = String::new();
    pulldown_cmark::html::push_html(&mut html, events);
    html
}

pub(super) fn safe_url(url: &str) -> Option<&str> {
    let url = url.trim();
    if url.is_empty() || url.chars().any(char::is_control) {
        return None;
    }
    let scheme_end = url.find(':');
    let path_end = url.find(['/', '?', '#']).unwrap_or(url.len());
    if let Some(end) = scheme_end
        && end < path_end
        && !["http", "https", "mailto"]
            .iter()
            .any(|scheme| url[..end].eq_ignore_ascii_case(scheme))
    {
        return None;
    }
    Some(url)
}

pub(super) fn summary(markdown: &Markdown) -> String {
    let mut summary = String::new();
    let mut paragraph = false;
    for event in Parser::new_ext(&markdown.text, crate::markdown::options()) {
        match event {
            Event::Start(Tag::Paragraph) => paragraph = true,
            Event::End(TagEnd::Paragraph) if paragraph => break,
            Event::Text(text) | Event::Code(text) if paragraph => summary.push_str(&text),
            Event::SoftBreak | Event::HardBreak if paragraph => summary.push(' '),
            _ => {}
        }
    }
    summary
}
