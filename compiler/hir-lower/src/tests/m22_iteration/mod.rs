use super::*;

mod effects;
mod generic;
mod lowering;
mod protocol;

fn public_visibility() -> ast::VisibilitySyntax {
    ast::VisibilitySyntax::Explicit {
        visibility: ast::DeclaredVisibility::Public,
        span: sp(),
    }
}

fn public_nominal(mut declaration: Decl) -> Decl {
    match &mut declaration {
        Decl::Class(declaration) => declaration.visibility = public_visibility(),
        Decl::Interface(declaration) => declaration.visibility = public_visibility(),
        Decl::Struct(declaration) => declaration.visibility = public_visibility(),
        _ => panic!("iteration test visibility helper requires a nominal declaration"),
    }
    declaration
}

fn operator_method(is_override: bool, return_ty: TypeRef, body: Expr) -> ast::FunctionDecl {
    let mut method = method_full(
        is_override,
        false,
        "iterator",
        Vec::new(),
        Some(return_ty),
        FunctionBody::Expr(Box::new(body)),
    );
    method.operator = Some(ast::OperatorModifier { span: sp() });
    method
}

fn iterator_next(element: TypeRef) -> ast::FunctionDecl {
    let mut method = override_method_expr(
        "next",
        Vec::new(),
        Some(ty_generic("Option", vec![element])),
        none(),
    );
    method.visibility = public_visibility();
    method
}

fn class_with_interface(name: &str, interface: TypeRef, methods: Vec<ast::FunctionDecl>) -> Decl {
    let mut declaration = class_decl(
        ast::ClassModifier::Final,
        name,
        Vec::new(),
        None,
        Vec::new(),
        methods,
    );
    let Decl::Class(class) = &mut declaration else {
        unreachable!()
    };
    class.supertypes.push(bare_supertype(interface));
    declaration
}

fn struct_with_interface(name: &str, interface: TypeRef, methods: Vec<ast::FunctionDecl>) -> Decl {
    let mut declaration = struct_decl_methods(name, Vec::new(), methods);
    let Decl::Struct(strukt) = &mut declaration else {
        unreachable!()
    };
    strukt.supertypes.push(bare_supertype(interface));
    declaration
}

fn iterator_class(name: &str, element: TypeRef) -> Decl {
    class_with_interface(
        name,
        ty_generic("Iterator", vec![element.clone()]),
        vec![iterator_next(element)],
    )
}

fn source_class(name: &str, iterator: &str) -> Decl {
    class_decl(
        ast::ClassModifier::Final,
        name,
        Vec::new(),
        None,
        Vec::new(),
        vec![operator_method(
            false,
            ty_named(iterator),
            call(iterator, Vec::new()),
        )],
    )
}

fn async_element_declarations() -> Vec<Decl> {
    let mut component = with_suspend(method_full(
        false,
        false,
        "component1",
        Vec::new(),
        Some(ty_named("Int")),
        FunctionBody::Expr(Box::new(int_lit(1))),
    ));
    component.operator = Some(ast::OperatorModifier { span: sp() });
    vec![
        public_nominal(class_decl(
            ast::ClassModifier::Final,
            "AsyncElement",
            Vec::new(),
            None,
            Vec::new(),
            vec![component],
        )),
        iterator_class("AsyncElementIterator", ty_named("AsyncElement")),
        source_class("AsyncElementSource", "AsyncElementIterator"),
    ]
}

fn iteration_source_interface() -> Decl {
    let mut iterator = bodyless_method(false, "iterator", Vec::new(), Some(ty_named("I")));
    iterator.operator = Some(ast::OperatorModifier { span: sp() });
    generic_interface_decl("IterationSource", vec!["I"], vec![iterator])
}

fn upper(name: &str, interface: TypeRef) -> ast::TypeParamDecl {
    ast::TypeParamDecl {
        name: ident(name),
        inline_bound: Some(ast::TypeBound::Upper(interface)),
        span: sp(),
    }
}

fn where_clause(parameter: &str, interface: TypeRef) -> ast::WhereClause {
    ast::WhereClause {
        constraints: vec![ast::TypeConstraint {
            parameter: ident(parameter),
            bound: ast::TypeBound::Upper(interface),
            span: sp(),
        }],
        span: sp(),
    }
}

fn for_stmt(pattern: ast::Pattern, iterable: Expr, body: Vec<Statement>) -> Statement {
    Statement {
        kind: StatementKind::For(ast::For {
            pattern,
            iterable,
            body: block(body),
            span: sp(),
        }),
        span: sp(),
    }
}

fn generic_iteration_consumer() -> Decl {
    let mut consume = fun_sig(
        "consume",
        vec!["I", "S"],
        vec![("source", ty_named("S"))],
        None,
        vec![for_stmt(pat_bind("item"), var("source"), Vec::new())],
    );
    let Decl::Function(function) = &mut consume else {
        unreachable!()
    };
    function.type_params[0] = upper("I", ty_generic("Iterator", vec![ty_named("Int")]));
    function.type_params[1] = upper("S", ty_generic("IterationSource", vec![ty_named("I")]));
    consume
}

fn break_stmt() -> Statement {
    Statement {
        kind: StatementKind::Break,
        span: sp(),
    }
}

fn continue_stmt() -> Statement {
    Statement {
        kind: StatementKind::Continue,
        span: sp(),
    }
}

fn export_body<'module>(module: &'module hir::Module, name: &str) -> &'module hir::Body {
    module
        .functions
        .iter()
        .find_map(|(_, function)| {
            (function.name == name).then(|| match &function.kind {
                hir::FunctionKind::User(body) => body,
                _ => panic!("test function `{name}` must have a body"),
            })
        })
        .unwrap_or_else(|| panic!("missing test function `{name}`"))
}

fn concrete_body<'module>(
    module: &'module hir::concrete::Module,
    name: &str,
) -> &'module hir::concrete::Body {
    module
        .functions
        .iter()
        .find_map(|(_, function)| {
            (function.name == name).then(|| match &function.kind {
                hir::concrete::FunctionKind::User(body) => body,
                _ => panic!("test function `{name}` must have a concrete body"),
            })
        })
        .unwrap_or_else(|| panic!("missing concrete test function `{name}`"))
}

fn export_callee_name<'module>(module: &'module hir::Module, expr: &hir::Expr) -> &'module str {
    let function = match &expr.kind {
        hir::ExprKind::Call {
            callee: hir::CallableTarget::Local(callee),
            ..
        } => module.callable_function(*callee),
        hir::ExprKind::MethodCall { callee, .. } => module.callable_function(*callee),
        other => panic!("expected a resolved call, found {other:?}"),
    };
    &module.functions[function].name
}

fn concrete_callee_name<'module>(
    module: &'module hir::concrete::Module,
    expr: &hir::concrete::Expr,
) -> &'module str {
    let function = match &expr.kind {
        hir::concrete::ExprKind::Call {
            callee: hir::concrete::CallableTarget::Local(callee),
            ..
        } => module.callable_function(*callee),
        hir::concrete::ExprKind::MethodCall { callee, .. } => module.callable_function(*callee),
        other => panic!("expected a concrete call, found {other:?}"),
    };
    &module.functions[function].name
}

fn concrete_local_with_prefix(body: &hir::concrete::Body, prefix: &str) -> hir::concrete::LocalId {
    body.locals
        .iter()
        .find_map(|(id, local)| local.name.starts_with(prefix).then_some(id))
        .unwrap_or_else(|| panic!("missing concrete local with prefix `{prefix}`"))
}

fn concrete_local_init(
    body: &hir::concrete::Body,
    local: hir::concrete::LocalId,
) -> &hir::concrete::Expr {
    body.statements
        .iter()
        .find_map(|statement| match &statement.kind {
            hir::concrete::StatementKind::ValDecl {
                pattern: hir::concrete::Pattern::Binding { local: found },
                init,
            } if *found == local => Some(init),
            _ => None,
        })
        .expect("concrete hidden local must have one initializer")
}

fn export_loop(
    body: &hir::Body,
) -> (
    hir::LoopId,
    &[hir::Statement],
    &hir::Expr,
    &[hir::Statement],
) {
    body.statements
        .iter()
        .find_map(|statement| match &statement.kind {
            hir::StatementKind::While {
                target,
                condition_setup,
                cond,
                body,
            } => Some((*target, condition_setup.as_slice(), cond, body.as_slice())),
            _ => None,
        })
        .expect("the source for loop must be expanded before Export HIR")
}

fn export_local_with_prefix(body: &hir::Body, prefix: &str) -> hir::LocalId {
    body.locals
        .iter()
        .find_map(|(id, local)| local.name.starts_with(prefix).then_some(id))
        .unwrap_or_else(|| panic!("missing export local with prefix `{prefix}`"))
}

fn export_local_init(statements: &[hir::Statement], local: hir::LocalId) -> &hir::Expr {
    statements
        .iter()
        .find_map(|statement| match &statement.kind {
            hir::StatementKind::ValDecl {
                pattern: hir::Pattern::Binding { local: found },
                init,
            } if *found == local => Some(init),
            _ => None,
        })
        .expect("a desugared iteration local has an ordinary initializer")
}
