//! Adapts one completed grammar result to the declaration stage's borrowed input.

use scoop_ast as ast;
use scoop_identity::{SourceIdentity, SourceRole};

pub fn declaration_source<'syntax, 'input>(
    parsed: &'syntax crate::ParsedFile,
    identity: &'input SourceIdentity,
    role: SourceRole,
    text: &'input str,
) -> ast::DeclarationSource<'syntax, 'input> {
    if let Some(source) = parsed.ast() {
        return ast::DeclarationSource {
            identity,
            role,
            text,
            header: Some(source.header()),
            documentation: None,
            declarations: source
                .declarations
                .iter()
                .map(ast::DeclarationInput::Complete)
                .collect(),
        };
    }
    let projections = parsed.declaration_data();
    let bodies = projections
        .declarations
        .iter()
        .filter_map(|item| {
            let ast::DeclarationFragment::FunctionBody(body) = &item.value else {
                return None;
            };
            Some((item.declaration, body.as_ref()))
        })
        .collect::<std::collections::HashMap<_, _>>();
    let complete = projections
        .declarations
        .iter()
        .filter_map(|item| {
            matches!(item.value, ast::DeclarationFragment::Complete(_)).then_some(item.declaration)
        })
        .collect::<std::collections::HashSet<_>>();
    let mut roots = projections
        .declarations
        .iter()
        .filter(|item| parsed.declarations()[item.declaration.0].parent.is_none())
        .collect::<Vec<_>>();
    roots.sort_by_key(|item| item.declaration.0);
    let declarations = roots
        .into_iter()
        .filter_map(|item| match &item.value {
            ast::DeclarationFragment::Complete(declaration) => {
                Some(ast::DeclarationInput::Complete(declaration))
            }
            ast::DeclarationFragment::FunctionHeader(header)
                if !complete.contains(&item.declaration) =>
            {
                let syntax = &parsed.declarations()[item.declaration.0];
                let header = ast::FunctionHeaderRef {
                    signature: &header.signature,
                    body_form: header.body_form,
                    span: ast::Span::new(header.signature.span.start, syntax.span.end),
                };
                Some(match bodies.get(&item.declaration) {
                    Some(body) => ast::DeclarationInput::RecoveredFunction { header, body },
                    None => ast::DeclarationInput::FunctionHeader(header),
                })
            }
            ast::DeclarationFragment::FunctionHeader(_)
            | ast::DeclarationFragment::FunctionBody(_) => None,
            ast::DeclarationFragment::Unavailable(header) => {
                Some(ast::DeclarationInput::Unavailable(header))
            }
        })
        .collect();
    let span = ast::Span::new(0, u32::from(parsed.syntax().text_range().end()));
    ast::DeclarationSource {
        identity,
        role,
        text,
        header: projections
            .source
            .as_ref()
            .and_then(|header| header.header(span)),
        declarations,
        documentation: None,
    }
}
