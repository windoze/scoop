use super::*;

pub(super) fn ident(name: &str, span: Span) -> ast::Ident {
    ast::Ident {
        text: name.into(),
        span,
    }
}

pub(super) fn variable(name: &str, span: Span) -> ast::Expr {
    ast::Expr::Var(ident(name, span))
}

pub(super) fn string(value: String, span: Span) -> ast::Expr {
    ast::Expr::StringLiteral { value, span }
}

pub(super) fn call(receiver: ast::Expr, name: &str, args: Vec<ast::Expr>, span: Span) -> ast::Expr {
    ast::Expr::MethodCall {
        receiver: Box::new(receiver),
        name: ident(name, span),
        navigation: ast::Navigation::Direct,
        type_args: Vec::new(),
        args: args
            .into_iter()
            .map(ast::CallArgument::positional)
            .collect(),
        span,
    }
}

pub(super) fn statement(expression: ast::Expr) -> ast::Statement {
    ast::Statement {
        span: expression.span(),
        kind: ast::StatementKind::Expr(expression),
    }
}

pub(super) fn local(name: &str, init: ast::Expr, span: Span) -> ast::Statement {
    ast::Statement {
        kind: ast::StatementKind::ValDecl(ast::ValDecl {
            mutable: false,
            target: ast::Pattern::Binding(ident(name, span)),
            ty: None,
            init,
            span,
        }),
        span,
    }
}

pub(super) fn field(name: &str, span: Span) -> ast::Expr {
    ast::Expr::FieldAccess(ast::FieldAccess {
        receiver: Box::new(ast::Expr::This { span }),
        selector: ast::FieldSelector::Name(ident(name, span)),
        navigation: ast::Navigation::Direct,
        span,
    })
}

pub(super) fn record(
    fields: Vec<(String, ast::Expr, Span)>,
    encoder: ast::Expr,
    span: Span,
) -> ast::Block {
    let mut statements = vec![local(
        "__encoding_fields",
        call(encoder, "keyed", vec![], span),
        span,
    )];
    for (name, value, span) in fields {
        let child = call(
            variable("__encoding_fields", span),
            "field",
            vec![string(name, span)],
            span,
        );
        statements.push(statement(call(value, "encode", vec![child], span)));
    }
    statements.push(statement(call(
        variable("__encoding_fields", span),
        "end",
        vec![],
        span,
    )));
    ast::Block { statements, span }
}

pub(super) fn sequence(
    values: Vec<(ast::Expr, Span)>,
    encoder: ast::Expr,
    span: Span,
) -> ast::Block {
    let mut statements = vec![local(
        "__encoding_elements",
        call(encoder, "unkeyed", vec![], span),
        span,
    )];
    for (value, span) in values {
        let child = call(
            variable("__encoding_elements", span),
            "element",
            vec![],
            span,
        );
        statements.push(statement(call(value, "encode", vec![child], span)));
    }
    statements.push(statement(call(
        variable("__encoding_elements", span),
        "end",
        vec![],
        span,
    )));
    ast::Block { statements, span }
}
