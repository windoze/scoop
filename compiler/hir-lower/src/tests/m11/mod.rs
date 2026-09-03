use super::*;

fn lambda(parameters: Option<Vec<ast::LambdaParam>>, tail: ast::Expr) -> ast::Expr {
    ast::Expr::Lambda {
        id: ast::LambdaId(0),
        is_suspend: false,
        parameters,
        body: block(vec![stmt(tail)]),
        span: sp(),
    }
}

fn suspend_lambda(parameters: Option<Vec<ast::LambdaParam>>, tail: ast::Expr) -> ast::Expr {
    let ast::Expr::Lambda {
        id,
        parameters,
        body,
        span,
        ..
    } = lambda(parameters, tail)
    else {
        unreachable!("lambda helper returns a lambda")
    };
    ast::Expr::Lambda {
        id,
        is_suspend: true,
        parameters,
        body,
        span,
    }
}

fn anonymous(
    params: Vec<(&str, ast::TypeRef)>,
    return_ty: Option<ast::TypeRef>,
    statements: Vec<ast::Statement>,
) -> ast::Expr {
    ast::Expr::AnonymousFunction {
        id: ast::AnonymousFunctionId(0),
        is_suspend: false,
        params: params
            .into_iter()
            .map(|(name, ty)| ast::Param {
                name: ident(name),
                ty,
                span: sp(),
            })
            .collect(),
        return_ty,
        body: block(statements),
        span: sp(),
    }
}

mod captures;
mod literals;
mod local_functions;
mod references;
mod types;
