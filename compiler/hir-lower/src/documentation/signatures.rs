use super::*;

pub(super) fn prefix(span: ast::Span, name: ast::Span) -> Vec<Part> {
    vec![Part::Source(ast::Span::new(span.start, name.end))]
}

pub(super) fn list(
    parts: &mut Vec<Part>,
    open: &str,
    close: &str,
    spans: impl IntoIterator<Item = ast::Span>,
) {
    parts.push(Part::Text(open.into()));
    for (index, span) in spans.into_iter().enumerate() {
        if index != 0 {
            parts.push(Part::Text(", ".into()));
        }
        parts.push(Part::Source(span));
    }
    parts.push(Part::Text(close.into()));
}

pub(super) fn generics(parts: &mut Vec<Part>, parameters: &[ast::TypeParamDecl]) {
    if !parameters.is_empty() {
        list(
            parts,
            "<",
            ">",
            parameters.iter().map(|parameter| parameter.span),
        );
    }
}

pub(super) fn parents(parts: &mut Vec<Part>, parents: &[ast::SupertypeSpec]) {
    if !parents.is_empty() {
        list(parts, " : ", "", parents.iter().map(|parent| parent.span));
    }
}

pub(super) fn constraints(parts: &mut Vec<Part>, clause: Option<&ast::WhereClause>) {
    if let Some(clause) = clause {
        parts.push(Part::Text(" ".into()));
        parts.push(Part::Source(clause.span));
    }
}
