use super::*;

mod generic;
mod protocol;
mod validation;
mod validation_definitions;
mod validation_effects;

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

fn checked_suspend_component_iteration_export() -> hir::Module {
    let mut declarations = async_element_declarations();
    declarations.push(suspend_fun(
        "consume",
        vec![for_stmt(
            pat_tuple(vec![pat_bind("value")], None),
            call("AsyncElementSource", Vec::new()),
            vec![val("seen", var("value"))],
        )],
    ));
    declarations.push(fun("main", Vec::new()));
    lower_user(file(declarations)).expect("a suspend component plan passes its reader boundary")
}

fn checked_suspend_iterator_iteration_export() -> hir::Module {
    let iterator = with_suspend(operator_method(
        false,
        ty_named("SuspendIterator"),
        call("SuspendIterator", Vec::new()),
    ));
    let source = class_decl(
        ast::ClassModifier::Final,
        "SuspendSource",
        Vec::new(),
        None,
        Vec::new(),
        vec![iterator],
    );
    lower_user(file(vec![
        iterator_class("SuspendIterator", ty_named("Int")),
        source,
        suspend_fun(
            "consume",
            vec![for_stmt(
                pat_bind("item"),
                call("SuspendSource", Vec::new()),
                Vec::new(),
            )],
        ),
        fun("main", Vec::new()),
    ]))
    .expect("a suspend iterator plan passes its reader boundary")
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

fn checked_basic_iteration_export() -> hir::Module {
    lower_user(file(vec![
        iterator_class("CheckedIterator", ty_named("Int")),
        source_class("CheckedSource", "CheckedIterator"),
        fun(
            "main",
            vec![for_stmt(
                pat_bind("item"),
                call("CheckedSource", Vec::new()),
                Vec::new(),
            )],
        ),
    ]))
    .expect("the producer emits a valid source for plan")
}

fn checked_bound_iteration_export() -> hir::Module {
    lower_user(file(vec![
        iteration_source_interface(),
        generic_iteration_consumer(),
        fun("main", Vec::new()),
    ]))
    .expect("the producer emits a valid bound-call iteration plan")
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

fn first_for(body: &hir::Body) -> &hir::ForIterationPlan {
    body.statements
        .iter()
        .find_map(|statement| match &statement.kind {
            hir::StatementKind::For(plan) => Some(plan),
            _ => None,
        })
        .expect("test body must contain a source for plan")
}

fn plan_from_parts(parts: hir::ForIterationPlanParts) -> hir::ForIterationPlan {
    let hir::ForIterationPlanParts {
        target,
        source_setup,
        source,
        source_init,
        iterator_setup,
        iterator_call,
        conformance,
        next,
        binding,
        body,
    } = parts;
    hir::ForIterationPlan::new(
        target,
        source_setup,
        source,
        source_init,
        iterator_setup,
        iterator_call,
        conformance,
        next,
        binding,
        body,
    )
}

fn replace_first_for(
    module: &mut hir::Module,
    function_name: &str,
    replacement: hir::ForIterationPlan,
) {
    replace_nth_for(module, function_name, 0, replacement);
}

fn replace_nth_for(
    module: &mut hir::Module,
    function_name: &str,
    index: usize,
    replacement: hir::ForIterationPlan,
) {
    let function = module
        .functions
        .iter()
        .find_map(|(id, function)| (function.name == function_name).then_some(id))
        .unwrap_or_else(|| panic!("missing test function `{function_name}`"));
    let hir::FunctionKind::User(body) = &mut module.functions[function].kind else {
        panic!("test function `{function_name}` must have a body")
    };
    let plan = body
        .statements
        .iter_mut()
        .filter_map(|statement| match &mut statement.kind {
            hir::StatementKind::For(plan) => Some(plan),
            _ => None,
        })
        .nth(index)
        .unwrap_or_else(|| panic!("test function `{function_name}` must contain a source for"));
    **plan = replacement;
}

fn export_callee_name<'module>(module: &'module hir::Module, expr: &hir::Expr) -> &'module str {
    let function = match &expr.kind {
        hir::ExprKind::Call { callee, .. } => module.callable_function(*callee),
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
        hir::concrete::ExprKind::Call { callee, .. } => module.callable_function(*callee),
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
