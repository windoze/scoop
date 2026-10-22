use scoop_hir::EditingAppliedCall;
use scoop_source::Span;
use scoop_syntax::{SyntaxKind as K, SyntaxNode, TextSize, TokenAtOffset};

pub(super) struct ArgumentList {
    opening: Span,
    closing: Span,
    commas: Vec<Span>,
}

pub(super) fn argument_lists(syntax: &SyntaxNode, offset: u32) -> Vec<ArgumentList> {
    let tokens = match syntax.token_at_offset(TextSize::from(offset)) {
        TokenAtOffset::None => Vec::new(),
        TokenAtOffset::Single(token) => vec![token],
        TokenAtOffset::Between(left, right) => vec![left, right],
    };
    let mut lists: Vec<ArgumentList> = Vec::new();
    for token in tokens {
        let Some(parent) = token.parent() else {
            continue;
        };
        for node in parent
            .ancestors()
            .filter(|node| node.kind() == K::ArgumentList)
        {
            let mut opening = None;
            let mut closing = None;
            let mut commas = Vec::new();
            for token in node
                .children_with_tokens()
                .filter_map(|element| element.into_token())
            {
                let span = Span::new(
                    token.text_range().start().into(),
                    token.text_range().end().into(),
                );
                match token.kind() {
                    K::LParen => opening = Some(span),
                    K::RParen => closing = Some(span),
                    K::Comma => commas.push(span),
                    _ => {}
                }
            }
            if let (Some(opening), Some(closing)) = (opening, closing)
                && opening.end <= offset
                && offset <= closing.start
                && !lists.iter().any(|list| list.opening == opening)
            {
                lists.push(ArgumentList {
                    opening,
                    closing,
                    commas,
                });
            }
        }
    }
    lists
}

/// The outer option distinguishes a cursor outside this call from a call with
/// no active parameter. Source input order is mapped to declaration positions.
pub(super) fn active_parameter(
    call: &EditingAppliedCall,
    offset: u32,
    lists: &[ArgumentList],
) -> Option<Option<usize>> {
    if offset < call.callee_end || offset >= call.span.end {
        return None;
    }
    if let Some(argument) = call.arguments.iter().find(|argument| {
        argument.trailing && argument.span.start <= offset && offset < argument.span.end
    }) {
        return Some(Some(argument.parameter));
    }
    let first_argument = call
        .arguments
        .first()
        .map_or(call.span.end, |argument| argument.span.start);
    let list = lists.iter().find(|list| {
        call.callee_end <= list.opening.start
            && list.opening.start < first_argument
            && list.closing.end <= call.span.end
    })?;
    let index = list
        .commas
        .iter()
        .filter(|comma| comma.end <= offset)
        .count();
    let regular = call
        .arguments
        .iter()
        .filter(|argument| !argument.trailing)
        .collect::<Vec<_>>();
    if regular.is_empty() {
        return Some((0..call.parameters.len()).find(|&parameter| {
            !call
                .arguments
                .iter()
                .any(|argument| argument.trailing && argument.parameter == parameter)
        }));
    }
    Some(regular.get(index).map(|argument| argument.parameter))
}
