use super::*;

#[test]
fn iteration_plan_validator_checks_the_enclosing_suspend_contract() {
    for mut export in [
        checked_suspend_iterator_iteration_export(),
        checked_suspend_component_iteration_export(),
    ]
    .map(hir::ExportHirOutput::into_module)
    {
        hir::validate_iteration_plans(&export)
            .expect("the producer's suspend function permits its protocol call");
        let consume = export
            .functions
            .iter()
            .find_map(|(id, function)| (function.name == "consume").then_some(id))
            .expect("the test module contains consume");
        assert!(export.functions[consume].is_suspend);
        export.functions[consume].is_suspend = false;

        let error = hir::validate_iteration_plans(&export)
            .expect_err("a forged ordinary owner cannot retain a suspend protocol call");
        assert_eq!(
            error.message(),
            "suspend iteration protocol call appears in a non-suspend region"
        );
    }
}

#[test]
fn iteration_core_rejects_a_suspend_next_slot() {
    let mut export = checked_basic_iteration_export().into_module();
    let next = export.interface_methods[defined_export_core(&export).iteration.next()].function;
    assert!(!export.functions[next].is_suspend);
    export.functions[next].is_suspend = true;

    let error = hir::validate_iteration_plans(&export)
        .expect_err("the canonical Iterator.next slot must remain ordinary");
    assert_eq!(error.message(), "invalid canonical Iterator core relation");
}

#[test]
fn no_gc_for_checks_the_hidden_canonical_next_call() {
    let annotation = || ast::Annotation {
        name: ident("NoGC"),
        args: Vec::new(),
        span: sp(),
    };

    let mut iterator = struct_with_interface(
        "NoGcIterator",
        ty_generic("Iterator", vec![ty_named("Int")]),
        vec![iterator_next(ty_named("Int"))],
    );
    let Decl::Struct(iterator_declaration) = &mut iterator else {
        unreachable!()
    };
    iterator_declaration.annotations.push(annotation());

    let mut source_iterator = operator_method(
        false,
        ty_named("NoGcIterator"),
        struct_init("NoGcIterator", Vec::new()),
    );
    source_iterator.annotations.push(annotation());
    let mut source = struct_decl_methods("NoGcSource", Vec::new(), vec![source_iterator]);
    let Decl::Struct(source_declaration) = &mut source else {
        unreachable!()
    };
    source_declaration.annotations.push(annotation());

    let mut consume = fun_sig(
        "consume",
        Vec::new(),
        vec![("source", ty_named("NoGcSource"))],
        None,
        vec![for_stmt(pat_bind("item"), var("source"), Vec::new())],
    );
    let Decl::Function(consume_function) = &mut consume else {
        unreachable!()
    };
    consume_function.annotations.push(annotation());

    let errors = lower_user(file(vec![
        iterator,
        source,
        consume,
        fun("main", Vec::new()),
    ]))
    .expect_err("canonical Iterator.next is a managed call inside @NoGC code");
    assert!(errors.iter().any(|error| {
        error.message == "`@NoGC` code may not call managed function `Iterator.next`"
    }));
}

#[test]
fn iteration_default_regions_keep_structural_suspend_ownership() {
    let suspend_iterator = with_suspend(operator_method(
        false,
        ty_named("DefaultIterator"),
        call("DefaultIterator", Vec::new()),
    ));
    let suspend_source = class_decl(
        ast::ClassModifier::Final,
        "DefaultSource",
        Vec::new(),
        None,
        Vec::new(),
        vec![suspend_iterator],
    );
    let default_value = Expr::If(Box::new(ast::If {
        cond: bool_lit(true),
        then_block: block(vec![
            for_stmt(
                pat_bind("item"),
                call("DefaultSource", Vec::new()),
                Vec::new(),
            ),
            stmt(int_lit(1)),
        ]),
        else_block: Some(block(vec![stmt(int_lit(2))])),
        span: sp(),
    }));
    let mut choose_declaration = fun_sig(
        "choose",
        Vec::new(),
        vec![("value", ty_named("Int"))],
        None,
        Vec::new(),
    );
    let Decl::Function(choose) = &mut choose_declaration else {
        unreachable!()
    };
    choose.is_suspend = true;
    choose.params[0].syntax = ast::ParameterSyntax::Default {
        expression: default_value,
        equals_span: sp(),
    };
    let executable = lower_user(file(vec![
        iterator_class("DefaultIterator", ty_named("Int")),
        suspend_source,
        choose_declaration,
        fun("main", Vec::new()),
    ]))
    .expect("a suspend declaration owns a default region with suspend iteration");
    let main = executable.entry();
    let export = executable.into_module();
    hir::validate_iteration_plans(&export).expect("the producer's default relation is valid");
    let (expression, source) = export
        .source_parameter_interfaces
        .iter()
        .flat_map(|interface| &interface.parameters)
        .find_map(|parameter| match parameter.calling {
            hir::ExportParameterCalling::Default { source, .. } => {
                Some((export.export_default_sources[source].expression, source))
            }
            _ => None,
        })
        .expect("choose has one exported default");
    assert!(export.export_default_exprs[expression].allows_suspend);

    let mut forged = export.clone();
    let choose = forged
        .functions
        .iter()
        .find_map(|(id, function)| (function.name == "choose").then_some(id))
        .expect("choose function");
    forged.functions[choose].is_suspend = false;
    let error = hir::validate_iteration_plans(&forged)
        .expect_err("the stored default effect must agree with its declaration owner");
    assert_eq!(
        error.message(),
        "export default suspend permission differs from its owner"
    );

    let mut forged = export.clone();
    forged.export_default_sources[source].expression =
        hir::ExportDefaultExprId::from_raw(u32::MAX.into());
    let error = hir::validate_iteration_plans(&forged)
        .expect_err("every referenced default source must name an arena expression");
    assert_eq!(error.message(), "default source has an invalid expression");

    let mut forged = export;
    let inherited = forged
        .source_parameter_interfaces
        .iter()
        .find(|interface| interface.owner == hir::ExportParameterOwner::Function(choose))
        .expect("choose parameter interface")
        .parameters
        .clone();
    let main_interface = forged
        .source_parameter_interfaces
        .iter_mut()
        .find(|interface| interface.owner == hir::ExportParameterOwner::Function(main))
        .expect("main parameter interface");
    main_interface.parameters = inherited;
    let error = hir::validate_iteration_plans(&forged)
        .expect_err("one template cannot be shared by suspend and ordinary owners");
    assert_eq!(
        error.message(),
        "export default has inconsistent owner suspend contracts"
    );
}

#[test]
fn iteration_plan_validator_accepts_inherited_and_upcast_protocol_receivers() {
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

    hir::validate_iteration_plans(&export)
        .expect("the reader must preserve the producer's subtype receiver semantics");
}

#[test]
fn iteration_plan_validator_rejects_a_forged_box_receiver_adaptation() {
    let extension = |receiver: &str| {
        let mut declaration = extension_expr(
            ty_named(receiver),
            "iterator",
            Vec::new(),
            Vec::new(),
            Some(ty_named("BoxIterator")),
            call("BoxIterator", Vec::new()),
        );
        let Decl::Function(function) = &mut declaration else {
            unreachable!()
        };
        function.operator = Some(ast::OperatorModifier { span: sp() });
        declaration
    };
    let executable = lower_user(file(vec![
        interface_decl("BoxSource", Vec::new()),
        interface_decl("UnrelatedSource", Vec::new()),
        struct_with_interface("BoxedValue", ty_named("BoxSource"), Vec::new()),
        iterator_class("BoxIterator", ty_named("Int")),
        extension("BoxSource"),
        extension("UnrelatedSource"),
        fun(
            "main",
            vec![for_stmt(
                pat_bind("item"),
                call("BoxedValue", Vec::new()),
                Vec::new(),
            )],
        ),
    ]))
    .expect("the value source is legally boxed to its implemented interface");
    let entry = executable.entry();
    let mut export = executable.into_module();
    hir::validate_iteration_plans(&export).expect("the producer's box relation is valid");

    let unrelated = export
        .interfaces
        .iter()
        .find_map(|(_, interface)| {
            (interface.name == "UnrelatedSource").then_some(interface.self_application)
        })
        .expect("unrelated interface application");
    let unrelated_type = export.interface_applications[unrelated].canonical_type;
    let unrelated_iterator = export
        .functions
        .iter()
        .find_map(|(id, function)| {
            (function.method.is_none()
                && function.modifiers.operator == Some(hir::OperatorKind::Iterator)
                && function
                    .params
                    .first()
                    .is_some_and(|parameter| parameter.ty == unrelated_type))
            .then_some(id)
        })
        .expect("the unrelated interface has one iterator extension");

    let mut parts = first_for(export_body(&export, "main")).clone().into_parts();
    let hir::StatementKind::ValDecl {
        pattern: hir::Pattern::Binding { local: setup_local },
        init,
    } = &mut parts.iterator_setup[0].kind
    else {
        panic!("the extension receiver is materialized once")
    };
    let setup_local = *setup_local;
    assert!(matches!(init.kind, hir::ExprKind::Box(_)));
    init.ty = unrelated_type;
    let hir::ExprKind::Call { callee, args } = &mut parts.iterator_call.kind else {
        panic!("the extension iterator is a direct call")
    };
    *callee = hir::Callable::Function(unrelated_iterator);
    let [receiver] = args.as_mut_slice() else {
        panic!("the extension iterator has exactly one receiver")
    };
    receiver.ty = unrelated_type;
    replace_first_for(&mut export, "main", plan_from_parts(parts));
    let hir::FunctionKind::User(body) = &mut export.functions[entry].kind else {
        panic!("main has a body")
    };
    body.locals[setup_local].ty = unrelated_type;

    let error = hir::validate_iteration_plans(&export)
        .expect_err("boxing cannot invent an unrelated interface conformance");
    assert_eq!(error.message(), "iterator");
}

#[test]
fn iteration_plan_validator_rejects_an_unboxed_value_interface_receiver() {
    let source_interface = interface_decl("ValueIterationSource", Vec::new());
    let mut source_iterator = extension_expr(
        ty_named("ValueIterationSource"),
        "iterator",
        Vec::new(),
        Vec::new(),
        Some(ty_named("ValueIterator")),
        struct_init("ValueIterator", Vec::new()),
    );
    let Decl::Function(source_iterator_function) = &mut source_iterator else {
        unreachable!()
    };
    source_iterator_function.operator = Some(ast::OperatorModifier { span: sp() });
    let executable = lower_user(file(vec![
        struct_with_interface(
            "ValueIterator",
            ty_generic("Iterator", vec![ty_named("Int")]),
            vec![iterator_next(ty_named("Int"))],
        ),
        source_interface,
        struct_with_interface("ValueSource", ty_named("ValueIterationSource"), Vec::new()),
        source_iterator,
        fun(
            "main",
            vec![for_stmt(
                pat_bind("item"),
                struct_init("ValueSource", Vec::new()),
                Vec::new(),
            )],
        ),
    ]))
    .expect("a value source is boxed before calling an interface receiver extension");
    let entry = executable.entry();
    let mut export = executable.into_module();
    hir::validate_iteration_plans(&export).expect("the producer's boxed receiver is valid");

    let mut parts = first_for(export_body(&export, "main")).clone().into_parts();
    let source_type = parts.source.ty;
    let hir::StatementKind::ValDecl {
        pattern: hir::Pattern::Binding { local: setup_local },
        init,
    } = &mut parts.iterator_setup[0].kind
    else {
        panic!("the interface extension receiver is materialized once")
    };
    let setup_local = *setup_local;
    let hir::ExprKind::Box(source) = &init.kind else {
        panic!("the value receiver is boxed for interface dispatch")
    };
    *init = *source.clone();
    let hir::ExprKind::Call { args, .. } = &mut parts.iterator_call.kind else {
        panic!("the interface receiver iterator is an extension call")
    };
    let [receiver] = args.as_mut_slice() else {
        panic!("the extension call has exactly one receiver argument")
    };
    receiver.ty = source_type;
    replace_first_for(&mut export, "main", plan_from_parts(parts));
    let hir::FunctionKind::User(body) = &mut export.functions[entry].kind else {
        panic!("main has a body")
    };
    body.locals[setup_local].ty = source_type;

    let error = hir::validate_iteration_plans(&export)
        .expect_err("value-to-interface extension dispatch cannot omit its box adaptation");
    assert_eq!(error.message(), "iterator");
}
