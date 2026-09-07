use super::*;

// --- patterns ---

pub(crate) fn pat_bind(name: &str) -> ast::Pattern {
    ast::Pattern::Binding(ident(name))
}

pub(crate) fn pat_bind_at(name: &str, span: Span) -> ast::Pattern {
    ast::Pattern::Binding(ident_at(name, span))
}

pub(crate) fn pat_wild() -> ast::Pattern {
    ast::Pattern::Wildcard { span: sp() }
}

pub(crate) fn pat_lit(expr: Expr) -> ast::Pattern {
    ast::Pattern::Literal {
        expr: Box::new(expr),
        span: sp(),
    }
}

/// `Path?(p1, p2, ..)` — enum positional variant or struct positional.
pub(crate) fn pat_pos(
    path: &[&str],
    elements: Vec<ast::Pattern>,
    rest: Option<Span>,
) -> ast::Pattern {
    ast::Pattern::Positional {
        path: path.iter().map(|p| ident(p)).collect(),
        elements,
        rest,
        span: sp(),
    }
}

/// `Path?{ f1, f2: renamed, .. }` — enum named-field variant or
/// struct field pattern.
pub(crate) fn pat_named(
    path: &[&str],
    fields: Vec<(&str, Option<&str>)>,
    rest: Option<Span>,
) -> ast::Pattern {
    pat_named_subpatterns(
        path,
        fields
            .into_iter()
            .map(|(field, rename)| (field, pat_bind(rename.unwrap_or(field))))
            .collect(),
        rest,
    )
}

pub(crate) fn pat_named_subpatterns(
    path: &[&str],
    fields: Vec<(&str, ast::Pattern)>,
    rest: Option<Span>,
) -> ast::Pattern {
    ast::Pattern::Named {
        path: path.iter().map(|p| ident(p)).collect(),
        fields: fields
            .into_iter()
            .map(|(field, subpattern)| ast::FieldPattern {
                field: ident(field),
                subpattern: Box::new(subpattern),
                span: sp(),
            })
            .collect(),
        rest,
        span: sp(),
    }
}

pub(crate) fn pat_tuple(elements: Vec<ast::Pattern>, rest: Option<Span>) -> ast::Pattern {
    ast::Pattern::Tuple {
        elements,
        rest,
        span: sp(),
    }
}
