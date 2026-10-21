use scoop_ast::{DocComment, DocCommentKind, DocText, DocTextMapping, Span};

/// Normalize only the comments retained and attached by the language parser.
pub(crate) fn normalize(comments: &[DocComment], source: &str) -> DocText {
    let first = &comments[0];
    let last = &comments[comments.len() - 1];
    let mut output = DocText {
        text: String::new(),
        span: Span::new(first.span.start, last.span.end),
        mapping: Vec::new(),
    };
    for (index, comment) in comments.iter().enumerate() {
        if index > 0 {
            let previous = &comments[index - 1];
            let gap = &source[previous.span.end as usize..comment.span.start as usize];
            let consecutive_lines = previous.kind == DocCommentKind::Line
                && comment.kind == DocCommentKind::Line
                && gap.chars().all(char::is_whitespace)
                && gap.bytes().filter(|byte| *byte == b'\n').count() == 1;
            push(
                &mut output,
                if consecutive_lines { "\n" } else { "\n\n" },
                Span::new(previous.span.end, comment.span.start),
            );
        }
        match comment.kind {
            DocCommentKind::Line => {
                let text = comment.text[3..]
                    .strip_suffix('\r')
                    .unwrap_or(&comment.text[3..]);
                let (text, skipped) = match text.strip_prefix(' ') {
                    Some(text) => (text, 4),
                    None => (text, 3),
                };
                let start = comment.span.start + skipped;
                push(
                    &mut output,
                    text,
                    Span::new(start, start + text.len() as u32),
                );
            }
            DocCommentKind::Block => block(&mut output, comment),
        }
    }
    output
}

struct Line<'a> {
    text: &'a str,
    start: usize,
    end: usize,
}

fn block(output: &mut DocText, comment: &DocComment) {
    if comment.text.len() < 5 {
        return;
    }
    let inner = &comment.text[3..comment.text.len() - 2];
    let mut offset = 3;
    let mut lines = inner
        .split_inclusive('\n')
        .map(|line| {
            let start = offset;
            offset += line.len();
            let content = line.strip_suffix('\n').unwrap_or(line);
            Line {
                text: content.strip_suffix('\r').unwrap_or(content),
                start,
                end: offset,
            }
        })
        .collect::<Vec<_>>();
    let indentation = lines
        .iter()
        .skip(1)
        .filter(|line| !line.text.trim().is_empty())
        .map(|line| line.text.len() - line.text.trim_start_matches([' ', '\t']).len())
        .min()
        .unwrap_or(0);
    for (index, line) in lines.iter_mut().enumerate() {
        let skipped = if index == 0 {
            usize::from(line.text.starts_with(' '))
        } else {
            indentation.min(line.text.len() - line.text.trim_start_matches([' ', '\t']).len())
        };
        line.text = &line.text[skipped..];
        line.start += skipped;
        if index > 0
            && let Some(text) = line.text.strip_prefix('*')
        {
            line.start += 1;
            line.text = text;
            if let Some(text) = line.text.strip_prefix(' ') {
                line.start += 1;
                line.text = text;
            }
        }
    }
    let start = lines.iter().position(|line| !line.text.trim().is_empty());
    let end = lines.iter().rposition(|line| !line.text.trim().is_empty());
    let (Some(start), Some(end)) = (start, end) else {
        return;
    };
    for (index, line) in lines[start..=end].iter().enumerate() {
        let at = comment.span.start + line.start as u32;
        push(
            output,
            line.text,
            Span::new(at, at + line.text.len() as u32),
        );
        if index < end - start {
            push(
                output,
                "\n",
                Span::new(
                    at + line.text.len() as u32,
                    comment.span.start + line.end as u32,
                ),
            );
        }
    }
}

fn push(output: &mut DocText, text: &str, source: Span) {
    if text.is_empty() {
        return;
    }
    let start = output.text.len() as u32;
    output.text.push_str(text);
    output.mapping.push(DocTextMapping {
        normalized: Span::new(start, output.text.len() as u32),
        source,
    });
}
