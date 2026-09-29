use super::*;

#[test]
fn break_and_continue_keep_the_innermost_typed_loop_target() {
    let output = lower_user_output(file(vec![
        iterator_class("JumpIterator", ty_named("Int")),
        source_class("JumpSource", "JumpIterator"),
        fun_sig(
            "jumps",
            Vec::new(),
            vec![("flag", ty_named("Boolean"))],
            None,
            vec![for_stmt(
                pat_bind("item"),
                call("JumpSource", Vec::new()),
                vec![
                    while_stmt(
                        var("flag"),
                        vec![if_stmt(
                            var("flag"),
                            vec![continue_stmt()],
                            Some(vec![break_stmt()]),
                        )],
                    ),
                    if_stmt(var("flag"), vec![continue_stmt()], Some(vec![break_stmt()])),
                ],
            )],
        ),
        fun("main", Vec::new()),
    ]))
    .expect("nested loop jumps must retain typed lexical targets");

    let (outer, _, _, loop_body) = export_loop(export_body(&output.export, "jumps"));
    let hir::StatementKind::While {
        target: inner,
        body: inner_body,
        ..
    } = &loop_body
        .iter()
        .find(|statement| matches!(statement.kind, hir::StatementKind::While { .. }))
        .unwrap()
        .kind
    else {
        panic!("the first source body statement must be the nested while")
    };
    assert_ne!(outer, *inner);
    let hir::StatementKind::If {
        then_body,
        else_body: Some(else_body),
        ..
    } = &inner_body[0].kind
    else {
        panic!("the nested while must contain both jump branches")
    };
    assert!(matches!(
        &then_body[0].kind,
        hir::StatementKind::Continue { target } if *target == *inner
    ));
    assert!(matches!(
        &else_body[0].kind,
        hir::StatementKind::Break { target } if *target == *inner
    ));

    let hir::StatementKind::If {
        then_body,
        else_body: Some(else_body),
        ..
    } = &loop_body
        .iter()
        .find(|statement| matches!(statement.kind, hir::StatementKind::If { .. }))
        .unwrap()
        .kind
    else {
        panic!("the outer source body must contain both jump branches")
    };
    assert!(matches!(
        &then_body[0].kind,
        hir::StatementKind::Continue { target } if *target == outer
    ));
    assert!(matches!(
        &else_body[0].kind,
        hir::StatementKind::Break { target } if *target == outer
    ));
}

#[test]
fn named_struct_for_binding_keeps_shape_and_action_orders_independent() {
    let module = lower_user(file(vec![
        public_nominal(struct_decl(
            "Pair",
            vec![("left", ty_named("Int")), ("right", ty_named("String"))],
        )),
        iterator_class("PairIterator", ty_named("Pair")),
        source_class("PairSource", "PairIterator"),
        fun(
            "main",
            vec![for_stmt(
                pat_named(
                    &["Pair"],
                    vec![("right", Some("r")), ("left", Some("l"))],
                    None,
                ),
                call("PairSource", Vec::new()),
                Vec::new(),
            )],
        ),
    ]))
    .expect("a named struct binding may list fields in reverse declaration order");

    let body = export_body(&module, "main");
    let (_, _, _, loop_body) = export_loop(body);
    let pair = module
        .structs
        .iter()
        .find(|(_, declaration)| declaration.name == "Pair")
        .unwrap()
        .1;
    assert_eq!(
        pair.semantic_fields()
            .iter()
            .map(|field| field.name.as_str())
            .collect::<Vec<_>>(),
        ["left", "right"]
    );
    assert_eq!(
        loop_body
            .iter()
            .filter_map(|statement| match &statement.kind {
                hir::StatementKind::ValDecl { init, .. } => match &init.kind {
                    hir::ExprKind::FieldAccess {
                        field: hir::FieldRef::StructField(field),
                        ..
                    } => Some(field.local_index()),
                    _ => None,
                },
                _ => None,
            })
            .collect::<Vec<_>>(),
        [1, 0]
    );
    assert_eq!(
        loop_body
            .iter()
            .filter_map(|statement| match &statement.kind {
                hir::StatementKind::ValDecl {
                    pattern: hir::Pattern::Binding { local },
                    ..
                } if matches!(body.locals[*local].name.as_str(), "r" | "l") =>
                    Some(body.locals[*local].name.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>(),
        ["r", "l"]
    );
}

#[test]
fn branch_merged_values_can_supply_the_iteration_source() {
    let attempted = Expr::Try(Box::new(ast::Try {
        body: block(vec![stmt(call("MergedSource", Vec::new()))]),
        catches: vec![catch_clause(
            "error",
            ty_named("UnwrapException"),
            vec![stmt(call("MergedSource", Vec::new()))],
        )],
        finally_body: None,
        span: sp(),
    }));

    let export = lower_user(file(vec![
        iterator_class("MergedIterator", ty_named("Int")),
        source_class("MergedSource", "MergedIterator"),
        fun(
            "main",
            vec![
                for_stmt(pat_bind("fromIf"), conditional_source(), Vec::new()),
                for_stmt(pat_bind("fromTry"), attempted, Vec::new()),
            ],
        ),
    ]))
    .expect("normal branch merges must definitely define each for source value");

    assert_eq!(
        export_body(&export, "main")
            .statements
            .iter()
            .filter(|statement| matches!(statement.kind, hir::StatementKind::While { .. }))
            .count(),
        2
    );
}

#[test]
fn inherited_and_upcast_receivers_resolve_iteration_protocols() {
    let inherited_source = class_decl(
        ast::ClassModifier::Open,
        "InheritedSourceBase",
        Vec::new(),
        None,
        Vec::new(),
        vec![operator_method(
            false,
            ty_named("InheritedIterator"),
            call("InheritedIterator", Vec::new()),
        )],
    );
    let inherited_source_child = class_decl(
        ast::ClassModifier::Final,
        "InheritedSourceChild",
        Vec::new(),
        Some(("InheritedSourceBase", Vec::new())),
        Vec::new(),
        Vec::new(),
    );

    let mut component = method_full(
        false,
        false,
        "component1",
        Vec::new(),
        Some(ty_named("Int")),
        FunctionBody::Expr(Box::new(int_lit(1))),
    );
    component.operator = Some(ast::OperatorModifier { span: sp() });
    let element_base = public_nominal(class_decl(
        ast::ClassModifier::Open,
        "ElementBase",
        Vec::new(),
        None,
        Vec::new(),
        vec![component],
    ));
    let element_child = public_nominal(class_decl(
        ast::ClassModifier::Final,
        "ElementChild",
        Vec::new(),
        Some(("ElementBase", Vec::new())),
        Vec::new(),
        Vec::new(),
    ));

    let mut extension = extension_expr(
        ty_named("ExtensionSourceBase"),
        "iterator",
        Vec::new(),
        Vec::new(),
        Some(ty_named("ExtensionIterator")),
        call("ExtensionIterator", Vec::new()),
    );
    let Decl::Function(extension_function) = &mut extension else {
        unreachable!()
    };
    extension_function.operator = Some(ast::OperatorModifier { span: sp() });

    let export = lower_user(file(vec![
        iterator_class("InheritedIterator", ty_named("Int")),
        inherited_source,
        inherited_source_child,
        element_base,
        element_child,
        iterator_class("ElementIterator", ty_named("ElementChild")),
        source_class("ElementSource", "ElementIterator"),
        iterator_class("ExtensionIterator", ty_named("Int")),
        class_decl(
            ast::ClassModifier::Open,
            "ExtensionSourceBase",
            Vec::new(),
            None,
            Vec::new(),
            Vec::new(),
        ),
        class_decl(
            ast::ClassModifier::Final,
            "ExtensionSourceChild",
            Vec::new(),
            Some(("ExtensionSourceBase", Vec::new())),
            Vec::new(),
            Vec::new(),
        ),
        extension,
        fun(
            "main",
            vec![
                for_stmt(
                    pat_bind("inherited"),
                    call("InheritedSourceChild", Vec::new()),
                    Vec::new(),
                ),
                for_stmt(
                    pat_tuple(vec![pat_bind("component")], None),
                    call("ElementSource", Vec::new()),
                    Vec::new(),
                ),
                for_stmt(
                    pat_bind("extended"),
                    call("ExtensionSourceChild", Vec::new()),
                    Vec::new(),
                ),
            ],
        ),
    ]))
    .expect("inherited members and a reference upcast extension are valid protocol receivers");

    assert_eq!(
        export_body(&export, "main")
            .statements
            .iter()
            .filter(|statement| matches!(statement.kind, hir::StatementKind::While { .. }))
            .count(),
        3
    );
}

fn conditional_source() -> Expr {
    Expr::If(Box::new(ast::If {
        cond: bool_lit(true),
        then_block: block(vec![stmt(call("MergedSource", Vec::new()))]),
        else_block: Some(block(vec![stmt(call("MergedSource", Vec::new()))])),
        span: sp(),
    }))
}
