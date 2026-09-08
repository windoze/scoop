use super::*;

#[test]
fn iteration_plan_validator_rejects_a_cross_field_binding_mismatch() {
    let mut export = checked_basic_iteration_export().into_module();
    hir::validate_iteration_plans(&export).expect("the unmodified producer plan is valid");

    let mut parts = first_for(export_body(&export, "main")).clone().into_parts();
    parts.binding.subject = parts.next.result();
    replace_first_for(&mut export, "main", plan_from_parts(parts));

    let error = hir::validate_iteration_plans(&export)
        .expect_err("the reader boundary must reject mismatched cross-field identities");
    assert_eq!(
        error.message(),
        "for binding subject is not the exact iteration element temporary"
    );
}

#[test]
fn iteration_plan_validator_rejects_a_forged_method_owner() {
    let mut export = checked_basic_iteration_export().into_module();
    let (application, wrong_owner) = {
        let plan = first_for(export_body(&export, "main"));
        let hir::ExprKind::MethodCall { callee, .. } = &plan.iterator_call().kind else {
            panic!("the checked source uses a member iterator operator")
        };
        let hir::MethodCallee::Callable(hir::Callable::Method(application)) = *callee else {
            panic!("the checked source uses an ordinary method application")
        };
        (application, plan.conformance().application())
    };
    export.method_applications[application].owner =
        hir::MethodOwnerApplication::Interface(wrong_owner);

    let error = hir::validate_iteration_plans(&export)
        .expect_err("a method application cannot claim an unrelated interface owner");
    assert_eq!(
        error.message(),
        "method application does not belong to its interface owner"
    );
}

#[test]
fn iteration_plan_validator_recomputes_every_bound_callable_signature_field() {
    let export = checked_bound_iteration_export().into_module();
    let bound = {
        let plan = first_for(export_body(&export, "consume"));
        let hir::ExprKind::MethodCall { callee, .. } = &plan.iterator_call().kind else {
            panic!("the generic source uses a bound member iterator operator")
        };
        let hir::MethodCallee::Bound(bound) = *callee else {
            panic!("the generic source retains its bound callable identity")
        };
        bound
    };
    let signature = export.bound_callable_refs[bound].instantiated_signature;
    enum Corruption {
        Suspend,
        Parameter,
        Return,
    }
    for corruption in [
        Corruption::Suspend,
        Corruption::Parameter,
        Corruption::Return,
    ] {
        let mut forged = export.clone();
        let unit = forged.unit;
        match corruption {
            Corruption::Suspend => {
                let is_suspend = forged.function_types[signature].is_suspend;
                forged.function_types[signature].is_suspend = !is_suspend;
            }
            Corruption::Parameter => {
                assert!(forged.function_types[signature].parameter_types.is_empty());
                forged.function_types[signature].parameter_types.push(unit);
            }
            Corruption::Return => {
                assert_ne!(forged.function_types[signature].return_type, unit);
                forged.function_types[signature].return_type = unit;
            }
        }

        let error = hir::validate_iteration_plans(&forged)
            .expect_err("every bound callable signature field is recomputed from its member");
        assert_eq!(
            error.message(),
            "bound protocol signature does not match its interface member"
        );
    }
}

#[test]
fn iteration_plan_validator_rejects_setup_reuse_of_the_source_temporary() {
    let mut export = checked_basic_iteration_export().into_module();
    let mut parts = first_for(export_body(&export, "main")).clone().into_parts();
    let mut alias = parts
        .iterator_setup
        .first()
        .cloned()
        .expect("the checked member call materializes one receiver");
    let hir::StatementKind::ValDecl {
        pattern: hir::Pattern::Binding { local },
        ..
    } = &mut alias.kind
    else {
        panic!("the checked receiver setup is one binding")
    };
    *local = parts.source.local;
    parts.source_setup.push(alias);
    replace_first_for(&mut export, "main", plan_from_parts(parts));

    let error = hir::validate_iteration_plans(&export)
        .expect_err("source setup cannot reuse a compiler-reserved temporary");
    assert_eq!(
        error.message(),
        "for source setup aliases a reserved temporary"
    );
}

#[test]
fn iteration_plan_validator_rejects_source_reads_of_future_plan_locals() {
    let mut export = checked_basic_iteration_export().into_module();
    let mut parts = first_for(export_body(&export, "main")).clone().into_parts();
    parts.source_init.kind = hir::ExprKind::Local(parts.source.local);
    replace_first_for(&mut export, "main", plan_from_parts(parts));

    let error = hir::validate_iteration_plans(&export)
        .expect_err("the source initializer cannot read its own future temporary");
    assert_eq!(
        error.message(),
        "for source prefix references a future plan local"
    );

    let mut export = checked_basic_iteration_export().into_module();
    let mut parts = first_for(export_body(&export, "main")).clone().into_parts();
    let mut illegal_read = parts.source_init.clone();
    illegal_read.kind = hir::ExprKind::Local(parts.next.element().local);
    illegal_read.ty = parts.next.element().ty;
    parts.source_setup.push(hir::Statement {
        kind: hir::StatementKind::Expr(illegal_read),
        span: sp(),
    });
    replace_first_for(&mut export, "main", plan_from_parts(parts));

    let error = hir::validate_iteration_plans(&export)
        .expect_err("source setup cannot read the later per-iteration element temporary");
    assert_eq!(
        error.message(),
        "for source prefix references a future plan local"
    );

    let mut export = lower_user(file(vec![
        iterator_class("BodyIterator", ty_named("Int")),
        source_class("BodySource", "BodyIterator"),
        fun(
            "main",
            vec![for_stmt(
                pat_bind("item"),
                call("BodySource", Vec::new()),
                vec![val("later", call("BodySource", Vec::new()))],
            )],
        ),
    ]))
    .expect("the producer keeps source evaluation before loop-body locals")
    .into_module();
    let later = export_body(&export, "main")
        .locals
        .iter()
        .find_map(|(local, declaration)| (declaration.name == "later").then_some(local))
        .expect("the loop body has one later local");
    let mut parts = first_for(export_body(&export, "main")).clone().into_parts();
    parts.source_init.kind = hir::ExprKind::Local(later);
    replace_first_for(&mut export, "main", plan_from_parts(parts));

    let error = hir::validate_iteration_plans(&export)
        .expect_err("source evaluation cannot read a local first defined in the loop body");
    assert_eq!(
        error.message(),
        "for source prefix references a future plan local"
    );
}

#[test]
fn iteration_plan_validator_follows_callable_value_capture_sources() {
    let closure = ast::Expr::Lambda {
        id: ast::LambdaId(0),
        is_suspend: false,
        parameters: None,
        body: block(vec![stmt(var("captured"))]),
        span: sp(),
    };
    let mut export = lower_user(file(vec![
        iterator_class("CaptureIterator", ty_named("Int")),
        source_class("CaptureSource", "CaptureIterator"),
        fun(
            "main",
            vec![
                val("captured", int_lit(1)),
                val("closure", closure),
                for_stmt(
                    pat_bind("item"),
                    call("CaptureSource", Vec::new()),
                    Vec::new(),
                ),
            ],
        ),
    ]))
    .expect("the producer emits one ordinary captured lambda before the loop")
    .into_module();
    let mut parts = first_for(export_body(&export, "main")).clone().into_parts();
    let future = parts.next.element();
    let closure = export_body(&export, "main")
        .statements
        .iter()
        .find_map(|statement| match &statement.kind {
            hir::StatementKind::ValDecl { init, .. }
                if matches!(init.kind, hir::ExprKind::Lambda(_)) =>
            {
                Some(init.clone())
            }
            _ => None,
        })
        .expect("the closure val retains its lambda expression");
    let hir::ExprKind::Lambda(lambda) = &closure.kind else {
        unreachable!()
    };
    let lambda = *lambda;
    let capture = export.lambdas[lambda]
        .captures
        .first_mut()
        .expect("the lambda captures one local");
    capture.source.kind = hir::ExprKind::Local(future.local);
    capture.source.ty = future.ty;
    parts.source_setup.push(hir::Statement {
        kind: hir::StatementKind::Expr(hir::Expr {
            kind: hir::ExprKind::Lambda(lambda),
            ..closure
        }),
        span: sp(),
    });
    replace_first_for(&mut export, "main", plan_from_parts(parts));

    let error = hir::validate_iteration_plans(&export)
        .expect_err("callable creation cannot hide a read of a future plan local");
    assert_eq!(
        error.message(),
        "for source prefix references a future plan local"
    );
}

#[test]
fn iteration_plan_validator_rejects_aliases_to_outer_region_definitions() {
    let mut export = lower_user(file(vec![
        iterator_class("CheckedIterator", ty_named("Int")),
        source_class("CheckedSource", "CheckedIterator"),
        fun(
            "main",
            vec![
                val("existing", int_lit(0)),
                for_stmt(
                    pat_bind("item"),
                    call("CheckedSource", Vec::new()),
                    Vec::new(),
                ),
            ],
        ),
    ]))
    .expect("the producer emits unique locals across the function region")
    .into_module();
    hir::validate_iteration_plans(&export).expect("the unmodified producer plan is valid");
    let existing = export_body(&export, "main")
        .locals
        .iter()
        .find_map(|(local, declaration)| (declaration.name == "existing").then_some(local))
        .expect("the outer val has one local identity");
    let mut parts = first_for(export_body(&export, "main")).clone().into_parts();
    let hir::IrrefutableBindingShape::Binding(shape_leaf) = &mut parts.binding.shape else {
        panic!("the checked source has one plain binding leaf")
    };
    shape_leaf.local = existing;
    let action_leaf = parts
        .binding
        .actions
        .iter_mut()
        .find_map(|action| match action {
            hir::IrrefutableBindingAction::Bind { target, .. } => Some(target),
            _ => None,
        })
        .expect("the checked source schedules its binding leaf");
    action_leaf.local = existing;
    replace_first_for(&mut export, "main", plan_from_parts(parts));

    let error = hir::validate_iteration_plans(&export)
        .expect_err("a for binding cannot redefine a preceding val local");
    assert_eq!(
        error.message(),
        "iteration plan reuses a local defined outside that plan"
    );

    let mut export = lower_user(file(vec![
        iterator_class("ParameterIterator", ty_named("Int")),
        source_class("ParameterSource", "ParameterIterator"),
        fun_sig(
            "consume",
            Vec::new(),
            vec![("source", ty_named("ParameterSource"))],
            None,
            vec![for_stmt(pat_bind("item"), var("source"), Vec::new())],
        ),
        fun("main", Vec::new()),
    ]))
    .expect("the producer emits a fresh source temporary distinct from its parameter")
    .into_module();
    let function = export
        .functions
        .iter()
        .find_map(|(id, function)| (function.name == "consume").then_some(id))
        .expect("consume function");
    let parameter = export.functions[function].params[0].local;
    let mut parts = first_for(export_body(&export, "consume"))
        .clone()
        .into_parts();
    parts.source.local = parameter;
    let hir::StatementKind::ValDecl { init, .. } = &mut parts.iterator_setup[0].kind else {
        panic!("the iterator receiver setup is one binding")
    };
    let hir::ExprKind::Local(receiver) = &mut init.kind else {
        panic!("the iterator receiver setup reads the source temporary")
    };
    *receiver = parameter;
    replace_first_for(&mut export, "consume", plan_from_parts(parts));

    let error = hir::validate_iteration_plans(&export)
        .expect_err("a for source temporary cannot redefine a function parameter");
    assert_eq!(
        error.message(),
        "iteration plan reuses a local defined outside that plan"
    );
}

#[test]
fn iteration_plan_validator_rejects_jumps_to_a_different_loop() {
    for jump in [
        hir::StatementKind::Break {
            target: hir::LoopId::from_raw(u32::MAX),
        },
        hir::StatementKind::Continue {
            target: hir::LoopId::from_raw(u32::MAX),
        },
    ] {
        let mut export = checked_basic_iteration_export().into_module();
        let mut parts = first_for(export_body(&export, "main")).clone().into_parts();
        assert_ne!(parts.target, hir::LoopId::from_raw(u32::MAX));
        parts.body.push(hir::Statement {
            kind: jump,
            span: sp(),
        });
        replace_first_for(&mut export, "main", plan_from_parts(parts));

        let error = hir::validate_iteration_plans(&export)
            .expect_err("a source-loop jump must target its innermost typed loop");
        assert_eq!(
            error.message(),
            "break or continue does not target the innermost active loop"
        );
    }
}

#[test]
fn iteration_plan_validator_rejects_a_mutable_binding_leaf() {
    let executable = checked_basic_iteration_export();
    let entry = executable.entry();
    let mut export = executable.into_module();
    let mut parts = first_for(export_body(&export, "main")).clone().into_parts();
    let hir::IrrefutableBindingShape::Binding(shape_leaf) = &mut parts.binding.shape else {
        panic!("the checked source has one plain binding leaf")
    };
    shape_leaf.mutability = hir::BindingMutability::Mutable;
    let leaf_local = shape_leaf.local;
    let bind = parts
        .binding
        .actions
        .iter_mut()
        .find_map(|action| match action {
            hir::IrrefutableBindingAction::Bind { target, .. } => Some(target),
            _ => None,
        })
        .expect("the checked source schedules its binding leaf");
    bind.mutability = hir::BindingMutability::Mutable;
    replace_first_for(&mut export, "main", plan_from_parts(parts));
    let hir::FunctionKind::User(body) = &mut export.functions[entry].kind else {
        panic!("main must have a body")
    };
    body.locals[leaf_local].mutable = true;

    let error = hir::validate_iteration_plans(&export)
        .expect_err("source for bindings must remain immutable at the reader boundary");
    assert_eq!(error.message(), "for binding leaf must be immutable");
}

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

    let plan = first_for(export_body(&output.export, "jumps"));
    let outer = plan.target();
    let hir::StatementKind::While {
        target: inner,
        body: inner_body,
        ..
    } = &plan.body()[0].kind
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
    } = &plan.body()[1].kind
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
    let plan = first_for(body);
    let hir::IrrefutableBindingShape::Struct { fields, .. } = &plan.binding().shape else {
        panic!("the producer must retain one declaration-order struct shape")
    };
    assert_eq!(
        fields
            .iter()
            .map(|(field, _)| field.local_index())
            .collect::<Vec<_>>(),
        [0, 1]
    );
    assert_eq!(
        plan.binding()
            .actions
            .iter()
            .filter_map(|action| match action {
                hir::IrrefutableBindingAction::Project {
                    projection: hir::BindingProjection::StructField(field),
                    ..
                } => Some(field.local_index()),
                hir::IrrefutableBindingAction::Project { .. } => {
                    panic!("a struct binding must use struct field projections")
                }
                _ => None,
            })
            .collect::<Vec<_>>(),
        [1, 0]
    );
    assert_eq!(
        plan.binding()
            .actions
            .iter()
            .filter_map(|action| match action {
                hir::IrrefutableBindingAction::Bind { target, .. } => {
                    Some(body.locals[target.local].name.as_str())
                }
                _ => None,
            })
            .collect::<Vec<_>>(),
        ["r", "l"]
    );
}
