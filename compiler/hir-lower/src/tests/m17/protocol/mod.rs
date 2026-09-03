use super::super::*;

mod exports;
mod materialization;
mod origins;
mod resolution;

fn function_body<'module>(module: &'module hir::Module, name: &str) -> &'module hir::Body {
    let (_, function) = module
        .functions
        .iter()
        .find(|(_, function)| function.name == name)
        .expect("test function");
    let hir::FunctionKind::User(body) = &function.kind else {
        unreachable!("test function has a source body")
    };
    body
}

fn concrete_function_body<'module>(
    module: &'module hir::LocalConcreteHir,
    name: &str,
) -> &'module hir::concrete::Body {
    let (_, function) = module
        .functions
        .iter()
        .find(|(_, function)| function.name == name)
        .expect("concrete test function");
    let hir::concrete::FunctionKind::User(body) = &function.kind else {
        unreachable!("test function has a concrete source body")
    };
    body
}

fn call_with_span(name: &str, span: ast::Span) -> Expr {
    Expr::Call(ast::CallExpr {
        callee: ast::Ident {
            text: name.to_string(),
            span,
        },
        type_args: Vec::new(),
        args: Vec::new(),
        span,
    })
}

fn with_default(mut declaration: Decl, parameter: usize, expression: Expr) -> Decl {
    let Decl::Function(function) = &mut declaration else {
        unreachable!("the test helper accepts a function declaration")
    };
    function.params[parameter].syntax = ast::ParameterSyntax::Default {
        expression,
        equals_span: sp(),
    };
    declaration
}

fn with_vararg(mut declaration: Decl, parameter: usize) -> Decl {
    let Decl::Function(function) = &mut declaration else {
        unreachable!("the test helper accepts a function declaration")
    };
    function.params[parameter].syntax = ast::ParameterSyntax::Vararg {
        modifier_span: sp(),
        default: ast::VarargDefaultSyntax::EmptyWhenOmitted,
    };
    declaration
}

fn method_with_default(
    mut declaration: ast::FunctionDecl,
    parameter: usize,
    expression: Expr,
) -> ast::FunctionDecl {
    declaration.params[parameter].syntax = ast::ParameterSyntax::Default {
        expression,
        equals_span: sp(),
    };
    declaration
}

fn method_with_vararg(mut declaration: ast::FunctionDecl, parameter: usize) -> ast::FunctionDecl {
    declaration.params[parameter].syntax = ast::ParameterSyntax::Vararg {
        modifier_span: sp(),
        default: ast::VarargDefaultSyntax::EmptyWhenOmitted,
    };
    declaration
}

fn with_constructor_syntax(
    mut declaration: Decl,
    parameter: usize,
    syntax: ast::ParameterSyntax,
) -> Decl {
    let Decl::Class(class) = &mut declaration else {
        unreachable!("the test helper accepts a class declaration")
    };
    let ast::ClassConstructorDecl::Declared(parameters) = &mut class.constructor else {
        unreachable!("the test helper accepts an explicit primary constructor")
    };
    parameters[parameter].syntax = syntax;
    declaration
}

fn identity_lambda() -> Expr {
    Expr::Lambda {
        id: ast::LambdaId(0),
        is_suspend: false,
        parameters: Some(vec![ast::LambdaParam {
            target: pat_bind("value"),
            ty: Some(ty_named("T")),
            span: sp(),
        }]),
        body: block(vec![stmt(var("value"))]),
        span: sp(),
    }
}
