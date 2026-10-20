use scoop_ast::{DocTagKind, SourceDocumentation, Span};

mod normalization;
mod syntax;

fn prepare(source: &str) -> SourceDocumentation {
    super::prepare_source(&scoop_parser::parse(source).unwrap(), source)
}

fn original(source: &str, span: Span) -> &str {
    &source[span.start as usize..span.end as usize]
}
