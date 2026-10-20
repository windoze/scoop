use scoop_ast::{DocText, Ident, QualifiedNameSyntax, QualifiedNameTailSyntax, Span};

/// Documentation paths use the language's identifier spelling, including
/// dots inside quoted identifiers. Name lookup is left entirely to HIR.
pub(crate) fn parse(text: &DocText, range: Span) -> Option<QualifiedNameSyntax> {
    let spelling = &text.text[range.start as usize..range.end as usize];
    let (first, mut offset) = identifier(text, range.start, spelling)?;
    let mut rest = Vec::new();
    while offset < spelling.len() {
        if !spelling[offset..].starts_with('.') {
            return None;
        }
        let dot = range.start + offset as u32;
        offset += 1;
        let (identifier, length) =
            identifier(text, range.start + offset as u32, &spelling[offset..])?;
        offset += length;
        rest.push(QualifiedNameTailSyntax {
            dot_span: text.source_span(Span::new(dot, dot + 1)),
            identifier,
        });
    }
    Some(QualifiedNameSyntax {
        first,
        rest,
        span: text.source_span(range),
    })
}

fn identifier(text: &DocText, base: u32, spelling: &str) -> Option<(Ident, usize)> {
    let first = spelling.chars().next()?;
    let (name, length) = if first == '`' {
        let end = spelling[1..].find('`')? + 1;
        let name = &spelling[1..end];
        if name.is_empty() || name.contains(['\n', '\r']) {
            return None;
        }
        (name, end + 1)
    } else {
        if first != '_' && !unicode_ident::is_xid_start(first) {
            return None;
        }
        let end = spelling
            .char_indices()
            .skip(1)
            .find(|(_, character)| !unicode_ident::is_xid_continue(*character))
            .map(|(index, _)| index)
            .unwrap_or(spelling.len());
        (&spelling[..end], end)
    };
    Some((
        Ident {
            text: name.into(),
            span: text.source_span(Span::new(base, base + length as u32)),
        },
        length,
    ))
}

/// One tag subject; whitespace inside a quoted identifier is significant.
pub(crate) fn subject(text: &str, start: usize, end: usize) -> (Span, usize) {
    let start = start + text[start..end].len() - text[start..end].trim_start().len();
    let mut quoted = false;
    let mut finish = start;
    for character in text[start..end].chars() {
        if !quoted && character.is_whitespace() {
            break;
        }
        if character == '`' {
            quoted = !quoted;
        }
        finish += character.len_utf8();
    }
    (Span::new(start as u32, finish as u32), finish)
}
