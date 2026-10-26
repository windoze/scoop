use crate::{
    render::{self, Document},
    spacing::Gap,
    syntax::Input,
};
use scoop_syntax::SyntaxKind as K;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Style {
    List,
    Block,
    Lambda,
    Control,
    Expression,
    Continuation,
}

struct Region {
    start: usize,
    end: usize,
    style: Style,
}

pub(crate) fn document(input: &Input, gaps: &[Gap]) -> Document {
    let regions = regions(input);
    sequence(input, gaps, &regions, 0, input.tokens.len(), false)
}

fn regions(input: &Input) -> Vec<Region> {
    let mut regions = Vec::new();
    let mut stack = Vec::new();
    for (index, token) in input.tokens.iter().enumerate() {
        let kind = token.kind();
        let parent = token.parent_kind();
        let generic = matches!(parent, K::TypeArgumentList | K::TypeParameterList);
        if matches!(
            kind,
            K::LParen | K::LBracket | K::LBrace | K::InterpolationStart
        ) || kind == K::Less && generic
        {
            let style = if kind == K::LBrace {
                match parent {
                    K::LambdaExpr => Style::Lambda,
                    K::CopyUpdateFields | K::PatternList | K::ParameterList => Style::List,
                    _ => Style::Block,
                }
            } else {
                Style::List
            };
            stack.push((index, style));
        } else if matches!(
            kind,
            K::RParen | K::RBracket | K::RBrace | K::InterpolationEnd
        ) || kind == K::Greater && generic
        {
            let (start, style) = stack
                .pop()
                .expect("successful parse has balanced delimiters");
            regions.push(Region {
                start,
                end: index + 1,
                style,
            });
        }
    }
    for &(kind, start, end, _) in &input.nodes {
        if kind == K::Expression && start > 0 && input.tokens[start - 1].kind() == K::Equal {
            regions.push(Region {
                start,
                end,
                style: Style::Continuation,
            });
        }
        if kind == K::SingleControlBody {
            regions.push(Region {
                start,
                end,
                style: Style::Control,
            });
        }
    }
    for &(start, end) in &input.expressions {
        regions.push(Region {
            start,
            end,
            style: Style::Expression,
        });
    }
    regions.sort_by_key(|region| {
        (
            region.start,
            std::cmp::Reverse(region.end),
            match region.style {
                Style::Control => 0,
                Style::Continuation => 1,
                Style::Expression => 2,
                _ => 3,
            },
        )
    });
    regions
}

fn sequence(
    input: &Input,
    gaps: &[Gap],
    regions: &[Region],
    start: usize,
    end: usize,
    leading: bool,
) -> Document {
    let mut docs = Vec::new();
    let mut index = start;
    let mut cursor = 0;
    while index < end {
        if (leading || index != start)
            && !regions.get(cursor).is_some_and(|r| {
                r.start == index && matches!(r.style, Style::Control | Style::Continuation)
            })
        {
            docs.push(gap(gaps[index]));
        }
        if let Some(region) = regions.get(cursor).filter(|region| region.start == index) {
            let nested_end = cursor
                + 1
                + regions[cursor + 1..].partition_point(|nested| nested.start < region.end);
            let nested = &regions[cursor + 1..nested_end];
            docs.push(delimited(input, gaps, nested, region));
            index = region.end;
            cursor = nested_end;
        } else {
            docs.push(render::token(input.tokens[index].syntax.text()));
            index += 1;
        }
    }
    Document::concat(docs)
}

fn delimited(input: &Input, gaps: &[Gap], nested: &[Region], region: &Region) -> Document {
    let Region { start, end, style } = *region;
    if style == Style::Continuation {
        return Document::line()
            .append(sequence(input, gaps, nested, start, end, false))
            .nest(4)
            .group();
    }
    if style == Style::Expression {
        return expression(input, gaps, nested, start, end);
    }
    if style == Style::Control {
        return Document::hardline()
            .append(sequence(input, gaps, nested, start, end, false))
            .nest(4);
    }
    let open = render::token(input.tokens[start].syntax.text());
    let close = render::token(input.tokens[end - 1].syntax.text());
    if end == start + 2 {
        return open.append(close);
    }
    let multiline_lambda = style == Style::Lambda
        && input
            .nodes
            .iter()
            .filter(|&&(kind, s, e, parent)| {
                kind == K::Statement
                    && parent == Some(K::LambdaExpr)
                    && s > start
                    && e < end
                    && !nested.iter().any(|region| {
                        region.style == Style::Lambda && s > region.start && e < region.end
                    })
            })
            .count()
            > 1;
    let block = style == Style::Block || multiline_lambda;
    let edge = if block {
        Document::hardline()
    } else if style == Style::Lambda {
        Document::line()
    } else {
        Document::line_()
    };
    let mut pieces = Vec::new();
    let mut item_start = start + 1;
    let mut nested_cursor = 0;
    let mut index = item_start;
    while index < end - 1 {
        if let Some(child) = nested
            .get(nested_cursor)
            .filter(|child| child.start == index)
        {
            index = child.end;
            nested_cursor +=
                1 + nested[nested_cursor + 1..].partition_point(|r| r.start < child.end);
            continue;
        }
        if !block && input.tokens[index].kind() == K::Comma && index + 1 < end - 1 {
            let children = within(nested, item_start, index + 1);
            pieces.push(sequence(
                input,
                gaps,
                children,
                item_start,
                index + 1,
                false,
            ));
            pieces.push(
                if input.tokens[index + 1].is_comment() || input.tokens[index].is_comment() {
                    gap(gaps[index + 1])
                } else {
                    Document::line()
                },
            );
            item_start = index + 1;
        }
        index += 1;
    }
    pieces.push(sequence(
        input,
        gaps,
        within(nested, item_start, end - 1),
        item_start,
        end - 1,
        false,
    ));
    let tail = if input.tokens[end - 2].is_comment() {
        gap(gaps[end - 1])
    } else {
        edge.clone()
    };
    let result = open
        .append(edge.append(Document::concat(pieces)).nest(4))
        .append(tail)
        .append(close);
    if block { result } else { result.group() }
}

fn expression(
    input: &Input,
    gaps: &[Gap],
    nested: &[Region],
    start: usize,
    end: usize,
) -> Document {
    let mut pieces = Vec::new();
    let mut cursor = 0;
    let mut index = start;
    let mut item = start;
    while index < end {
        if let Some(child) = nested.get(cursor).filter(|child| child.start == index) {
            index = child.end;
            cursor += 1 + nested[cursor + 1..].partition_point(|region| region.start < child.end);
            continue;
        }
        let last_operator_token = input.tokens[index].parent_kind() == K::BinaryOperator
            && (index + 1 == end
                || input.tokens[index].operator() != input.tokens[index + 1].operator());
        index += 1;
        if last_operator_token {
            let part = sequence(input, gaps, within(nested, item, index), item, index, false);
            pieces.push(if item == start {
                part
            } else {
                Document::line().append(part).nest(4)
            });
            item = index;
        }
    }
    let part = sequence(input, gaps, within(nested, item, end), item, end, false);
    pieces.push(Document::line().append(part).nest(4));
    Document::concat(pieces).group()
}

fn within(regions: &[Region], start: usize, end: usize) -> &[Region] {
    let first = regions.partition_point(|region| region.start < start);
    let last = regions.partition_point(|region| region.start < end);
    &regions[first..last]
}

fn gap(gap: Gap) -> Document {
    match gap {
        Gap::None => Document::nil(),
        Gap::Space => Document::space(),
        Gap::Line => Document::hardline(),
        Gap::Blank => Document::hardline().append(Document::hardline()),
    }
}
