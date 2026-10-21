use std::ops::Range;

use pulldown_cmark::{Event, Parser, Tag, TagEnd};
use scoop_ast::{Diagnostic, DocMarkdown, DocSeeTarget, DocTag, DocTagKind, DocText, Span};

use crate::{markdown, paths};

struct Marker<'a> {
    name: &'a str,
    start: usize,
    after_name: usize,
}

pub(crate) fn split(
    text: &DocText,
    definitions: &[DocText],
    warnings: &mut Vec<Diagnostic>,
) -> (DocMarkdown, Vec<DocTag>) {
    let markers = markers(&text.text);
    let body_end = markers
        .first()
        .map_or(text.text.len(), |marker| marker.start);
    let body = markdown::prepare(trimmed(text, 0..body_end), definitions, warnings);
    let mut tags = Vec::new();
    for (index, marker) in markers.iter().enumerate() {
        let end = markers
            .get(index + 1)
            .map_or(text.text.len(), |next| next.start);
        let span = text.source_span(Span::new(marker.start as u32, marker.after_name as u32));
        let header_end = text.text[marker.after_name..end]
            .find('\n')
            .map_or(end, |offset| marker.after_name + offset);
        let (subject, after_subject) = paths::subject(&text.text, marker.after_name, header_end);
        let kind = match marker.name {
            "param" | "property" => {
                let Some(path) = named_subject(text, subject, marker, warnings) else {
                    continue;
                };
                if !path.rest.is_empty() {
                    warnings.push(Diagnostic::warning_at(
                        path.span,
                        format!("`@{}` requires one parameter name", marker.name),
                    ));
                    continue;
                }
                match marker.name {
                    "param" => DocTagKind::Param(path.first),
                    _ => DocTagKind::Property(path.first),
                }
            }
            "throws" => {
                let Some(path) = named_subject(text, subject, marker, warnings) else {
                    continue;
                };
                DocTagKind::Throws(path)
            }
            "see" => {
                let subject_text = &text.text[subject.start as usize..subject.end as usize];
                if subject_text.contains("://") || subject_text.starts_with("mailto:") {
                    DocTagKind::See(DocSeeTarget::Url {
                        text: subject_text.into(),
                        span: text.source_span(subject),
                    })
                } else {
                    let Some(path) = named_subject(text, subject, marker, warnings) else {
                        continue;
                    };
                    DocTagKind::See(DocSeeTarget::Symbol(path))
                }
            }
            "constructor" => DocTagKind::Constructor,
            "receiver" => DocTagKind::Receiver,
            "return" => DocTagKind::Return,
            "since" if !text.text[marker.after_name..end].trim().is_empty() => DocTagKind::Since,
            "since" => {
                warnings.push(Diagnostic::warning_at(
                    span,
                    "`@since` requires version text",
                ));
                continue;
            }
            name => {
                warnings.push(Diagnostic::warning_at(
                    span,
                    format!("unknown documentation tag `@{name}`"),
                ));
                continue;
            }
        };
        let body_start = match kind {
            DocTagKind::Param(_)
            | DocTagKind::Property(_)
            | DocTagKind::Throws(_)
            | DocTagKind::See(_) => after_subject,
            _ => marker.after_name,
        };
        let body_start = body_start + usize::from(text.text[body_start..end].starts_with(' '));
        tags.push(DocTag {
            kind,
            span,
            body: markdown::prepare(trimmed(text, body_start..end), definitions, warnings),
        });
    }
    (body, tags)
}

fn named_subject(
    text: &DocText,
    subject: Span,
    marker: &Marker<'_>,
    warnings: &mut Vec<Diagnostic>,
) -> Option<scoop_ast::QualifiedNameSyntax> {
    if subject.start == subject.end {
        warnings.push(Diagnostic::warning_at(
            text.source_span(Span::new(marker.start as u32, marker.after_name as u32)),
            format!("`@{}` requires a name", marker.name),
        ));
        None
    } else if let Some(path) = paths::parse(text, subject) {
        Some(path)
    } else {
        warnings.push(Diagnostic::warning_at(
            text.source_span(subject),
            format!("invalid name for documentation tag `@{}`", marker.name),
        ));
        None
    }
}

fn markers(text: &str) -> Vec<Marker<'_>> {
    let mut ordinary = Vec::new();
    let mut in_code = false;
    for (event, range) in Parser::new_ext(text, markdown::options()).into_offset_iter() {
        match event {
            Event::Start(Tag::CodeBlock(_)) => in_code = true,
            Event::End(TagEnd::CodeBlock) => in_code = false,
            Event::Text(_) if !in_code => ordinary.push(range),
            _ => {}
        }
    }
    let mut offset = 0;
    let mut markers = Vec::new();
    for line in text.split_inclusive('\n') {
        let trimmed = line.trim_start_matches([' ', '\t']);
        let start = offset + line.len() - trimmed.len();
        if let Some(tag) = trimmed.strip_prefix('@')
            && ordinary.iter().any(|range| range.contains(&start))
        {
            let length = tag.find(char::is_whitespace).unwrap_or(tag.len());
            markers.push(Marker {
                name: &tag[..length],
                start,
                after_name: start + length + 1,
            });
        }
        offset += line.len();
    }
    markers
}

fn trimmed(text: &DocText, range: Range<usize>) -> DocText {
    let slice = &text.text[range.clone()];
    let start = range.start + slice.len() - slice.trim_start_matches(['\r', '\n']).len();
    let end = range.start + slice.trim_end().len();
    text.slice(Span::new(start as u32, end.max(start) as u32))
}
