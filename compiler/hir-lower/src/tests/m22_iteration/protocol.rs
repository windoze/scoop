use super::*;

#[test]
fn basic_for_plan_keeps_source_iterator_and_next_exactly_once() {
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
    .expect("a source for loop must lower to one complete typed plan");

    let module = &output.export;
    let export_body = export_body(module, "main");
    let plan = first_for(export_body);
    let int = int_type(module);
    assert!(plan.source_setup().is_empty());
    let [receiver_setup] = plan.iterator_setup() else {
        panic!("the iterator receiver must be materialized exactly once")
    };
    let hir::StatementKind::ValDecl { init, .. } = &receiver_setup.kind else {
        panic!("the iterator receiver setup must be one temporary binding")
    };
    assert!(matches!(
        &init.kind,
        hir::ExprKind::Local(local) if *local == plan.source().local
    ));
    assert_eq!(plan.source().ty, plan.source_init().ty);
    assert_eq!(export_callee_name(module, plan.source_init()), "makeSource");
    assert_eq!(
        export_callee_name(module, plan.iterator_call()),
        "BasicSource.iterator"
    );

    let conformance = plan.conformance();
    assert_eq!(conformance.source().ty, plan.iterator_call().ty);
    let application = &module.interface_applications[conformance.application()];
    assert_eq!(application.template, module.iteration_core.iterator());
    assert_eq!(application.arguments, [int]);
    assert_eq!(conformance.iterator().ty, application.canonical_type);

    let next = plan.next();
    let iterator_local_ids = [
        plan.source().local,
        plan.conformance().source().local,
        plan.conformance().iterator().local,
        next.result().local,
        next.element().local,
    ];
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
    assert_eq!(
        iterator_paths.len(),
        iterator_local_ids.len(),
        "each for-protocol local must have a distinct structural path"
    );
    let next_application = &module.method_applications[next.callable()];
    assert_eq!(
        next_application.function,
        module.interface_methods[module.iteration_core.next()].function
    );
    assert_eq!(next.element().ty, int);
    assert_eq!(plan.binding().subject, next.element());
    let option = &module.enum_applications[next.option().application()];
    assert_eq!(option.template, module.option_core.enumeration());
    assert_eq!(option.arguments, [int]);
    let hir::IrrefutableBindingShape::Binding(binding) = &plan.binding().shape else {
        panic!("a plain for variable must retain one binding leaf")
    };
    assert_eq!(binding.ty, int);
    assert_eq!(plan.body().len(), 1);

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
    let hir::concrete::ExprKind::VariantTest {
        operand: some_operand,
        variant: some,
    } = &cond.kind
    else {
        panic!("the loop condition must test the canonical Some variant")
    };
    assert!(matches!(
        &some_operand.kind,
        hir::concrete::ExprKind::Local(local) if *local == *next_result
    ));
    let Some(payload_statement) = loop_body.first() else {
        panic!("a successful next result must project its payload")
    };
    let hir::concrete::StatementKind::ValDecl { init: payload, .. } = &payload_statement.kind
    else {
        panic!("the first successful-iteration action must bind the payload")
    };
    let hir::concrete::ExprKind::VariantPayloadProject {
        operand: payload_operand,
        field,
    } = &payload.kind
    else {
        panic!("the element binding must project the canonical Some payload")
    };
    assert_eq!(field.variant(), *some);
    assert!(matches!(
        &payload_operand.kind,
        hir::concrete::ExprKind::Local(local) if *local == *next_result
    ));
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
    let plan = first_for(export_body(module, "main"));
    assert_eq!(
        export_callee_name(module, plan.iterator_call()),
        "LayeredSource.iterator"
    );
    assert_eq!(
        module.interface_applications[plan.conformance().application()].arguments,
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
    let plan = first_for(export_body(&output.export, "consume"));
    let call = plan
        .binding()
        .actions
        .iter()
        .find_map(|action| match action {
            hir::IrrefutableBindingAction::Component { call, .. } => Some(call),
            _ => None,
        })
        .expect("the class binding plan must contain component1");
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

    let plan = first_for(export_body(&module, "consume"));
    let next = &module.method_applications[plan.next().callable()];
    let canonical_next = module.interface_methods[module.iteration_core.next()].function;
    let shadow_next = module
        .functions
        .iter()
        .find_map(|(id, function)| (function.name == "Names.next").then_some(id))
        .expect("the unrelated user next declaration is retained");
    assert_eq!(next.function, canonical_next);
    assert_ne!(next.function, shadow_next);

    let option = plan.next().option();
    assert_eq!(
        option.some_payload().variant().declaration(),
        module.option_core.some()
    );
    assert_eq!(option.none().declaration(), module.option_core.none());
    assert_eq!(
        module.enum_applications[option.application()].template,
        module.option_core.enumeration()
    );
    let shadow = module
        .enums
        .iter()
        .find_map(|(id, enumeration)| (enumeration.name == "ShadowOption").then_some(id))
        .expect("the shadow enum is retained");
    assert_ne!(shadow, module.option_core.enumeration());
}
