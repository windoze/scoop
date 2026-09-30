use super::*;

#[test]
fn basic_for_expansion_keeps_source_iterator_and_next_exactly_once() {
    let output = lower_user_output(file(vec![
        iterator_class("BasicIterator", ty_named("Int")),
        source_class("BasicSource", "BasicIterator"),
        fun_expr(
            "makeSource",
            Vec::new(),
            Vec::new(),
            Some(ty_named("BasicSource")),
            call("BasicSource", Vec::new()),
        ),
        fun(
            "main",
            vec![for_stmt(
                pat_bind("item"),
                call("makeSource", Vec::new()),
                vec![val("seen", var("item"))],
            )],
        ),
    ]))
    .expect("a source for loop must lower to ordinary typed statements");

    let module = &output.export;
    let export_body = export_body(module, "main");
    let int = int_type(module);
    let source = export_local_with_prefix(export_body, "$for.source.");
    let raw = export_local_with_prefix(export_body, "$for.iterator.result.");
    let iterator = export_body
        .locals
        .iter()
        .find_map(|(id, local)| {
            (id != raw && local.name.starts_with("$for.iterator.")).then_some(id)
        })
        .expect("the adapted iterator has its own local");
    let next_result = export_local_with_prefix(export_body, "$for.next.");
    let element = export_local_with_prefix(export_body, "$for.element.");
    assert_eq!(
        export_callee_name(module, export_local_init(&export_body.statements, source)),
        "makeSource"
    );
    assert_eq!(
        export_callee_name(module, export_local_init(&export_body.statements, raw)),
        "BasicSource.iterator"
    );
    let hir::Type::Interface(application) = module.types[export_body.locals[iterator].ty] else {
        panic!("the saved iterator must have the exact core interface type")
    };
    let application = &module.interface_applications[application];
    assert_eq!(
        application.template,
        module.nominal_identities[defined_export_core(module).iteration.iterator()]
            .declaration_id()
    );
    assert_eq!(application.arguments, [int]);
    let iterator_local_ids = [source, raw, iterator, next_result, element];
    let iterator_paths = iterator_local_ids
        .into_iter()
        .map(|local| {
            let scoop_identity::LocalValueSelector::Synthetic { path, role } =
                &export_body.locals[local].selector
            else {
                panic!("for desugaring locals must have typed synthetic selectors")
            };
            assert_eq!(*role, scoop_identity::SyntheticLocalRole::DesugaredIterator);
            path
        })
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(iterator_paths.len(), iterator_local_ids.len());
    let (_, setup, cond, loop_body) = export_loop(export_body);
    assert_eq!(setup.len(), 1);
    let next_call = export_local_init(setup, next_result);
    let hir::ExprKind::MethodCall { callee, .. } = &next_call.kind else {
        panic!("the condition setup must contain the canonical next call")
    };
    assert_eq!(
        module.callable_function(*callee),
        module.interface_methods[defined_export_core(module).iteration.next()].function
    );
    let hir::Type::Enum(option) = module.types[next_call.ty] else {
        panic!("next returns the actual Option application")
    };
    assert_eq!(
        module.enum_applications[option].template,
        module.nominal_identities[defined_export_core(module).option.enumeration()]
            .declaration_id()
    );
    assert_eq!(module.enum_applications[option].arguments, [int]);
    assert!(
        matches!(&cond.kind, hir::ExprKind::IsSome(operand) if matches!(operand.kind, hir::ExprKind::Local(local) if local == next_result))
    );
    assert!(
        matches!(&export_local_init(loop_body, element).kind, hir::ExprKind::Unwrap { operand, trap_on_none: false } if matches!(operand.kind, hir::ExprKind::Local(local) if local == next_result))
    );
    let item = export_local_with_prefix(export_body, "item");
    assert_eq!(export_body.locals[item].ty, int);
    assert!(!export_body.locals[item].mutable);
    assert!(
        matches!(export_local_init(loop_body, item).kind, hir::ExprKind::Local(local) if local == element)
    );

    let concrete = &output.local;
    let body = concrete_body(concrete, "main");
    let source = concrete_local_with_prefix(body, "$for.source.");
    assert_eq!(
        concrete_callee_name(concrete, concrete_local_init(body, source)),
        "makeSource"
    );
    let raw_iterator = concrete_local_with_prefix(body, "$for.iterator.result.");
    assert_eq!(
        concrete_callee_name(concrete, concrete_local_init(body, raw_iterator)),
        "BasicSource.iterator"
    );
    let loops = body
        .statements
        .iter()
        .filter_map(|statement| match &statement.kind {
            hir::concrete::StatementKind::While {
                condition_setup,
                cond,
                body,
                ..
            } => Some((condition_setup, cond, body)),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [(condition_setup, cond, loop_body)] = loops.as_slice() else {
        panic!("one source for must expand to exactly one poll loop")
    };
    let [poll] = condition_setup.as_slice() else {
        panic!("the loop header must contain exactly one next result binding")
    };
    let hir::concrete::StatementKind::ValDecl {
        pattern: hir::concrete::Pattern::Binding { local: next_result },
        init,
    } = &poll.kind
    else {
        panic!("the single header action must materialize next()")
    };
    assert_eq!(concrete_callee_name(concrete, init), "Iterator.next");
    let hir::concrete::ExprKind::IsSome(some_operand) = &cond.kind else {
        panic!("the loop condition must test the canonical Option value")
    };
    assert!(
        matches!(&some_operand.kind, hir::concrete::ExprKind::Local(local) if *local == *next_result)
    );
    let hir::concrete::StatementKind::ValDecl { init: payload, .. } = &loop_body[0].kind else {
        panic!("the first successful iteration action binds the payload")
    };
    let hir::concrete::ExprKind::Unwrap {
        operand,
        trap_on_none: false,
    } = &payload.kind
    else {
        panic!("a successful next result is projected without a trap")
    };
    assert!(
        matches!(&operand.kind, hir::concrete::ExprKind::Local(local) if *local == *next_result)
    );
}

#[test]
fn member_iterator_layer_wins_before_extension_layer() {
    let mut extension = extension_expr(
        ty_named("LayeredSource"),
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

    let output = lower_user_output(file(vec![
        iterator_class("MemberIterator", ty_named("Int")),
        iterator_class("ExtensionIterator", ty_named("String")),
        source_class("LayeredSource", "MemberIterator"),
        extension,
        fun(
            "main",
            vec![for_stmt(
                pat_bind("item"),
                call("LayeredSource", Vec::new()),
                Vec::new(),
            )],
        ),
    ]))
    .expect("a valid member iterator must stop lookup before extensions");

    let module = &output.export;
    let body = export_body(module, "main");
    let raw = export_local_with_prefix(body, "$for.iterator.result.");
    let (_, setup, _, _) = export_loop(body);
    let next = export_local_with_prefix(body, "$for.next.");
    let hir::Type::Enum(option) = module.types[export_local_init(setup, next).ty] else {
        panic!("the poll result has the selected element type")
    };
    assert_eq!(
        export_callee_name(module, export_local_init(&body.statements, raw)),
        "LayeredSource.iterator"
    );
    assert_eq!(
        module.enum_applications[option].arguments,
        [int_type(module)]
    );
}

#[test]
fn selected_member_with_invalid_protocol_result_does_not_fall_back_to_extension() {
    let bad_source = class_decl(
        ast::ClassModifier::Final,
        "BadLayeredSource",
        Vec::new(),
        None,
        Vec::new(),
        vec![operator_method(false, ty_named("Int"), int_lit(0))],
    );
    let mut extension = extension_expr(
        ty_named("BadLayeredSource"),
        "iterator",
        Vec::new(),
        Vec::new(),
        Some(ty_named("FallbackIterator")),
        call("FallbackIterator", Vec::new()),
    );
    let Decl::Function(extension_function) = &mut extension else {
        unreachable!()
    };
    extension_function.operator = Some(ast::OperatorModifier { span: sp() });

    let errors = lower_user(file(vec![
        iterator_class("FallbackIterator", ty_named("Int")),
        bad_source,
        extension,
        fun(
            "main",
            vec![for_stmt(
                pat_bind("item"),
                call("BadLayeredSource", Vec::new()),
                Vec::new(),
            )],
        ),
    ]))
    .expect_err("a selected member's invalid result must not reopen extension lookup");

    assert_eq!(errors.len(), 1, "{errors:?}");
    assert_eq!(
        errors[0].message,
        "iterator operator result type `Int` does not implement `Iterator<T>`"
    );
}

#[test]
fn ordinary_function_reports_the_selected_suspend_iterator_effect() {
    let suspend_iterator = with_suspend(operator_method(
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
        vec![suspend_iterator],
    );
    let errors = lower_user(file(vec![
        iterator_class("SuspendIterator", ty_named("Int")),
        source,
        fun(
            "main",
            vec![for_stmt(
                pat_bind("item"),
                call("SuspendSource", Vec::new()),
                Vec::new(),
            )],
        ),
    ]))
    .expect_err("an ordinary callable cannot select a suspend iterator operator");

    assert_eq!(errors.len(), 1, "{errors:?}");
    assert_eq!(
        errors[0].message,
        "suspend function `SuspendSource.iterator` cannot be called from non-suspend function `main`"
    );
}

#[test]
fn suspend_for_binding_may_call_a_suspend_component() {
    let mut ordinary = async_element_declarations();
    ordinary.push(fun(
        "main",
        vec![for_stmt(
            pat_tuple(vec![pat_bind("value")], None),
            call("AsyncElementSource", Vec::new()),
            Vec::new(),
        )],
    ));
    let errors = lower_user(file(ordinary))
        .expect_err("an ordinary for binding cannot select a suspend component");
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert_eq!(
        errors[0].message,
        "suspend function `AsyncElement.component1` cannot be called from non-suspend function `main`"
    );

    let mut suspended = async_element_declarations();
    suspended.push(suspend_fun(
        "consume",
        vec![for_stmt(
            pat_tuple(vec![pat_bind("value")], None),
            call("AsyncElementSource", Vec::new()),
            vec![val("seen", var("value"))],
        )],
    ));
    suspended.push(fun("main", Vec::new()));
    let output = lower_user_output(file(suspended))
        .expect("a suspend for binding may retain and concretize its suspend component call");
    let (_, _, _, body) = export_loop(export_body(&output.export, "consume"));
    let call = body
        .iter()
        .find_map(|statement| match &statement.kind {
            hir::StatementKind::ValDecl { init, .. }
                if matches!(init.kind, hir::ExprKind::MethodCall { .. }) =>
            {
                Some(init)
            }
            _ => None,
        })
        .expect("the class binding must invoke component1");
    let hir::ExprKind::MethodCall { callee, .. } = &call.kind else {
        panic!("component1 must remain an exact method call")
    };
    let function = output.export.callable_function(*callee);
    assert!(output.export.functions[function].is_suspend);
}

#[test]
fn iterator_result_requires_one_exact_iterator_application() {
    let bad_source = class_decl(
        ast::ClassModifier::Final,
        "BadSource",
        Vec::new(),
        None,
        Vec::new(),
        vec![operator_method(false, ty_named("Int"), int_lit(0))],
    );
    let mut ambiguous = fun_sig(
        "ambiguous",
        vec!["I", "S"],
        vec![("source", ty_named("S"))],
        None,
        vec![for_stmt(pat_bind("item"), var("source"), Vec::new())],
    );
    let Decl::Function(function) = &mut ambiguous else {
        unreachable!()
    };
    function.type_params[0] = upper("I", ty_generic("Iterator", vec![ty_named("Int")]));
    function.type_params[1] = upper("S", ty_generic("IterationSource", vec![ty_named("I")]));
    function.where_clause = Some(where_clause(
        "I",
        ty_generic("Iterator", vec![ty_named("String")]),
    ));

    let errors = lower_user(file(vec![
        iteration_source_interface(),
        bad_source,
        ambiguous,
        fun(
            "main",
            vec![for_stmt(
                pat_bind("item"),
                call("BadSource", Vec::new()),
                Vec::new(),
            )],
        ),
    ]))
    .expect_err("zero and multiple Iterator applications must both be rejected");

    assert!(errors.iter().any(|error| {
        error.message == "iterator operator result type `Int` does not implement `Iterator<T>`"
    }));
    assert!(errors.iter().any(|error| {
        error.message
            == "iterator operator result type `I` implements multiple distinct `Iterator<T>` applications"
    }));
}

#[test]
fn for_rejects_refutable_patterns_and_loop_jumps_outside_a_loop() {
    for (element, pattern) in [
        (
            ty_nullable(ty_named("Int")),
            pat_pos(&["Some"], vec![pat_bind("value")], None),
        ),
        (ty_named("Int"), pat_lit(int_lit(1))),
    ] {
        let errors = lower_user(file(vec![
            iterator_class("PatternIterator", element),
            source_class("PatternSource", "PatternIterator"),
            fun(
                "main",
                vec![for_stmt(
                    pattern,
                    call("PatternSource", Vec::new()),
                    Vec::new(),
                )],
            ),
        ]))
        .expect_err("source for bindings must be recursively irrefutable");
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert_eq!(
            errors[0].message,
            "refutable patterns are only allowed in `when`"
        );
    }

    let errors = lower_user(file(vec![fun("main", vec![break_stmt(), continue_stmt()])]))
        .expect_err("unlabelled jumps cannot escape a callable without a loop");
    assert_eq!(errors.len(), 2, "{errors:?}");
    assert_eq!(errors[0].message, "`break` is only allowed inside a loop");
    assert_eq!(
        errors[1].message,
        "`continue` is only allowed inside a loop"
    );
}

#[test]
fn iteration_plan_keeps_canonical_next_some_and_none_identities() {
    let mut iterator = bodyless_method(
        false,
        "iterator",
        Vec::new(),
        Some(ty_generic("Iterator", vec![ty_named("Int")])),
    );
    iterator.operator = Some(ast::OperatorModifier { span: sp() });
    let module = lower_user(file(vec![
        enum_decl(
            "ShadowOption",
            Vec::new(),
            vec![
                variant_positional("Some", vec![ty_named("Int")]),
                variant_unit("None"),
            ],
        ),
        class_decl(
            ast::ClassModifier::Final,
            "Names",
            Vec::new(),
            None,
            Vec::new(),
            vec![method("next", Vec::new(), None, Vec::new())],
        ),
        interface_decl("CanonicalSource", vec![iterator]),
        fun_sig(
            "consume",
            Vec::new(),
            vec![("source", ty_named("CanonicalSource"))],
            None,
            vec![for_stmt(pat_bind("item"), var("source"), Vec::new())],
        ),
        fun("main", Vec::new()),
    ]))
    .expect("unrelated declarations with protocol spellings must not change plan identities");

    let body = export_body(&module, "consume");
    let (_, setup, _, _) = export_loop(body);
    let next_call = export_local_init(setup, export_local_with_prefix(body, "$for.next."));
    let hir::ExprKind::MethodCall { callee, .. } = &next_call.kind else {
        panic!("the poll invokes the checked core slot")
    };
    let next_function = module.callable_function(*callee);
    let canonical_next =
        module.interface_methods[defined_export_core(&module).iteration.next()].function;
    let shadow_next = module
        .functions
        .iter()
        .find_map(|(id, function)| (function.name == "Names.next").then_some(id))
        .expect("the unrelated user next declaration is retained");
    assert_eq!(next_function, canonical_next);
    assert_ne!(next_function, shadow_next);

    let hir::Type::Enum(option) = module.types[next_call.ty] else {
        panic!("next retains the core Option return type")
    };
    assert_eq!(
        module.enum_applications[option].template,
        module.nominal_identities[defined_export_core(&module).option.enumeration()]
            .declaration_id()
    );
    let shadow = module
        .enums
        .iter()
        .find_map(|(id, enumeration)| (enumeration.name == "ShadowOption").then_some(id))
        .expect("the shadow enum is retained");
    assert_ne!(shadow, defined_export_core(&module).option.enumeration());
}
