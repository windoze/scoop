use super::*;
use scoop_syntax::{DeclarationSyntax, SyntaxKind};

pub(super) struct LocalFunctionOwner {
    declaration: DeclarationIndex,
    parent: DeclarationIndex,
    block: Option<ast::BodyBlockId>,
}

impl BodyRecorder {
    pub(in crate::parser::syntax) fn local_function_header(
        &mut self,
        declaration: DeclarationIndex,
        declarations: &[DeclarationSyntax],
    ) {
        let Some(parent) = declarations[declaration.0].parent else {
            return;
        };
        if declarations[parent.0].kind == SyntaxKind::FunctionDecl {
            self.local_functions.push(LocalFunctionOwner {
                declaration,
                parent,
                block: self.block_for(parent),
            });
        }
    }
}

pub(super) fn attach(
    bodies: &mut BTreeMap<usize, ast::RecoveredFunctionBody>,
    owners: Vec<LocalFunctionOwner>,
    declarations: &[DeclarationSyntax],
    headers: &[DeclarationData<ast::FunctionHeader>],
) {
    let headers = headers
        .iter()
        .map(|item| (item.declaration, &item.value))
        .collect::<std::collections::HashMap<_, _>>();
    // Grammar headers are in declaration order, so descendants are attached first.
    for owner in owners.into_iter().rev() {
        if declarations[owner.declaration.0].source_span.is_some() {
            continue;
        }
        let Some(body) = bodies.get(&owner.declaration.0).cloned() else {
            continue;
        };
        let Some(parent) = bodies.get_mut(&owner.parent.0) else {
            continue;
        };
        let header = headers[&owner.declaration].clone();
        let span = ast::Span::new(
            header.signature.span.start,
            declarations[owner.declaration.0].span.end,
        );
        parent.local_functions.insert(
            0,
            ast::RecoveredLocalFunction {
                block: owner.block,
                header,
                body: Box::new(body),
                span,
            },
        );
    }
}
