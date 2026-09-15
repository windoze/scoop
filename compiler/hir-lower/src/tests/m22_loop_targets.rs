use super::*;

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

fn export_loop_targets(statements: &[hir::Statement], out: &mut Vec<hir::LoopId>) {
    for statement in statements {
        match &statement.kind {
            hir::StatementKind::While {
                target,
                condition_setup,
                body,
                ..
            } => {
                out.push(*target);
                export_loop_targets(condition_setup, out);
                export_loop_targets(body, out);
            }
            hir::StatementKind::If {
                then_body,
                else_body,
                ..
            } => {
                export_loop_targets(then_body, out);
                if let Some(else_body) = else_body {
                    export_loop_targets(else_body, out);
                }
            }
            hir::StatementKind::When(when) => {
                for arm in &when.arms {
                    if let Some(guard) = &arm.guard {
                        export_loop_targets(&guard.setup, out);
                    }
                    export_loop_targets(&arm.body, out);
                }
                if let hir::WhenFallback::Else(body) = &when.fallback {
                    export_loop_targets(body, out);
                }
            }
            hir::StatementKind::Try(try_) => {
                export_loop_targets(&try_.body, out);
                for catch in &try_.catches {
                    export_loop_targets(&catch.body, out);
                }
                if let Some(body) = &try_.finally_body {
                    export_loop_targets(body, out);
                }
            }
            _ => {}
        }
    }
}

fn concrete_loop_targets(
    statements: &[hir::concrete::Statement],
    out: &mut Vec<hir::concrete::LoopId>,
) {
    for statement in statements {
        match &statement.kind {
            hir::concrete::StatementKind::While {
                target,
                condition_setup,
                body,
                ..
            } => {
                out.push(*target);
                concrete_loop_targets(condition_setup, out);
                concrete_loop_targets(body, out);
            }
            hir::concrete::StatementKind::If {
                then_body,
                else_body,
                ..
            } => {
                concrete_loop_targets(then_body, out);
                if let Some(else_body) = else_body {
                    concrete_loop_targets(else_body, out);
                }
            }
            hir::concrete::StatementKind::When(when) => {
                for arm in &when.arms {
                    if let Some(guard) = &arm.guard {
                        concrete_loop_targets(&guard.setup, out);
                    }
                    concrete_loop_targets(&arm.body, out);
                }
                if let hir::concrete::WhenFallback::Else(body) = &when.fallback {
                    concrete_loop_targets(body, out);
                }
            }
            hir::concrete::StatementKind::Try(try_) => {
                concrete_loop_targets(&try_.body, out);
                for catch in &try_.catches {
                    concrete_loop_targets(&catch.body, out);
                }
                if let Some(body) = &try_.finally_body {
                    concrete_loop_targets(body, out);
                }
            }
            _ => {}
        }
    }
}

#[test]
fn loop_targets_remain_globally_unique_across_nested_callable_bodies() {
    let operation = Expr::Lambda {
        id: ast::LambdaId(0),
        is_suspend: false,
        parameters: Some(Vec::new()),
        body: block(vec![
            while_stmt(bool_lit(false), Vec::new()),
            stmt(unit_lit()),
        ]),
        span: sp(),
    };
    let output = lower_user_output(file(vec![fun(
        "main",
        vec![while_stmt(
            bool_lit(true),
            vec![val_ty(
                "operation",
                Some(ty_function(false, Vec::new(), ty_named("Unit"))),
                operation,
            )],
        )],
    )]))
    .expect("nested callable loops lower with isolated lexical stacks");

    let mut outer_targets = Vec::new();
    export_loop_targets(
        &export_body(&output.export, "main").statements,
        &mut outer_targets,
    );
    let (_, lambda) = output
        .export
        .lambdas
        .iter()
        .next()
        .expect("the nested lambda has an entity");
    let hir::FunctionKind::User(lambda_body) = &output.export.functions[lambda.function].kind
    else {
        panic!("the nested lambda has a body")
    };
    let mut lambda_targets = Vec::new();
    export_loop_targets(&lambda_body.statements, &mut lambda_targets);

    assert_eq!(outer_targets.len(), 1);
    assert_eq!(lambda_targets.len(), 1);
    assert_ne!(outer_targets[0], lambda_targets[0]);
}

#[test]
fn concretization_rebinds_setup_and_body_jumps_to_its_fresh_target() {
    let output = lower_user_output(file(vec![fun(
        "main",
        vec![while_stmt(bool_lit(true), Vec::new())],
    )]))
    .expect("ordinary while lowers");
    let entry = output.export.entry();
    let mut export = output.export.into_module();
    let hir::FunctionKind::User(body) = &mut export.functions[entry].kind else {
        panic!("main has a body")
    };
    let hir::StatementKind::While {
        target,
        condition_setup,
        body,
        ..
    } = &mut body.statements[0].kind
    else {
        panic!("main contains a while")
    };
    let source_target = *target;
    condition_setup.push(hir::Statement {
        kind: hir::StatementKind::Break {
            target: source_target,
        },
        span: sp(),
    });
    body.push(hir::Statement {
        kind: hir::StatementKind::Continue {
            target: source_target,
        },
        span: sp(),
    });

    let concrete =
        concretize_export(&export).expect("test Export HIR carries locally defined core protocols");
    let hir::concrete::StatementKind::While {
        target,
        condition_setup,
        body,
        ..
    } = &concrete_body(&concrete, "main").statements[0].kind
    else {
        panic!("concrete main contains a while")
    };
    assert!(matches!(
        condition_setup[0].kind,
        hir::concrete::StatementKind::Break { target: jump } if jump == *target
    ));
    assert!(matches!(
        body[0].kind,
        hir::concrete::StatementKind::Continue { target: jump } if jump == *target
    ));
}

#[test]
#[should_panic(expected = "break or continue does not target the innermost active loop")]
fn concretization_rejects_a_non_innermost_loop_target() {
    let output = lower_user_output(file(vec![fun(
        "main",
        vec![while_stmt(
            bool_lit(true),
            vec![while_stmt(bool_lit(true), Vec::new())],
        )],
    )]))
    .expect("nested while statements lower");
    let entry = output.export.entry();
    let mut export = output.export.into_module();
    let hir::FunctionKind::User(body) = &mut export.functions[entry].kind else {
        panic!("main has a body")
    };
    let hir::StatementKind::While {
        target: outer_target,
        body: outer_body,
        ..
    } = &mut body.statements[0].kind
    else {
        panic!("main contains an outer while")
    };
    let outer_target = *outer_target;
    let hir::StatementKind::While {
        body: inner_body, ..
    } = &mut outer_body[0].kind
    else {
        panic!("main contains an inner while")
    };
    inner_body.push(hir::Statement {
        kind: hir::StatementKind::Break {
            target: outer_target,
        },
        span: sp(),
    });

    let _ =
        concretize_export(&export).expect("test Export HIR carries locally defined core protocols");
}

#[test]
fn repeated_default_materialization_allocates_fresh_loop_targets() {
    let default_value = Expr::If(Box::new(ast::If {
        cond: bool_lit(true),
        then_block: block(vec![
            while_stmt(bool_lit(false), Vec::new()),
            stmt(int_lit(1)),
        ]),
        else_block: Some(block(vec![stmt(int_lit(2))])),
        span: sp(),
    }));
    let mut choose_decl = fun_sig(
        "choose",
        Vec::new(),
        vec![("value", ty_named("Int"))],
        Some(ty_named("Int")),
        vec![ret(Some(var("value")))],
    );
    let Decl::Function(choose) = &mut choose_decl else {
        unreachable!("fun_sig creates a function")
    };
    choose.params[0].syntax = ast::ParameterSyntax::Default {
        expression: default_value,
        equals_span: sp(),
    };
    let output = lower_user_output(file(vec![
        choose_decl,
        fun(
            "main",
            vec![
                val("first", call("choose", Vec::new())),
                val("second", call("choose", Vec::new())),
            ],
        ),
    ]))
    .expect("a structured default can be materialized twice");

    let mut template_targets = Vec::new();
    for (_, template) in output.export.export_default_exprs.iter() {
        export_loop_targets(&template.statements, &mut template_targets);
    }
    assert_eq!(template_targets.len(), 1);

    let mut materialized_targets = Vec::new();
    export_loop_targets(
        &export_body(&output.export, "main").statements,
        &mut materialized_targets,
    );
    assert_eq!(materialized_targets.len(), 2);
    assert_ne!(materialized_targets[0], materialized_targets[1]);
    assert!(
        materialized_targets
            .iter()
            .all(|target| *target != template_targets[0])
    );

    let mut concrete_targets = Vec::new();
    concrete_loop_targets(
        &concrete_body(&output.local, "main").statements,
        &mut concrete_targets,
    );
    assert_eq!(concrete_targets.len(), 2);
    assert_ne!(concrete_targets[0], concrete_targets[1]);
}

#[test]
#[should_panic(expected = "Export HIR loop identity space exhausted")]
fn loop_target_allocator_rejects_overflow() {
    let mut lowerer = Lowerer::new();
    lowerer.next_loop_identity = u32::MAX;
    let _ = lowerer.fresh_loop();
}
