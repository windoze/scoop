use super::*;

#[derive(Clone, Copy)]
enum InvalidSourceDefinition {
    SelfRead,
    ForwardRead,
    ConditionalOnly,
    LoopOnly,
    WhenSubject,
}

#[test]
fn iteration_plan_validator_accepts_branch_merged_source_values() {
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

    hir::validate_iteration_plans(&export)
        .expect("the reader must accept producer-owned branch merge locals");
}

#[test]
fn iteration_definition_flow_excludes_abrupt_loop_paths() {
    let mut export = checked_basic_iteration_export();
    let source = first_for(export_body(&export, "main")).source();
    let local = allocate_forged_local(&mut export, source.local, "$forged.merge");
    let mut parts = first_for(export_body(&export, "main")).clone().into_parts();
    let original = parts.source_init.clone();
    let loop_target = hir::LoopId::from_raw(u32::MAX - 1);
    let future_read = local_read(
        &original,
        parts.next.element().local,
        parts.next.element().ty,
    );
    assert_ne!(parts.target, loop_target);
    parts.source_setup.push(hir::Statement {
        kind: hir::StatementKind::If {
            cond: bool_expression(&original, export.boolean, true),
            then_body: vec![hir::Statement {
                kind: hir::StatementKind::While {
                    target: loop_target,
                    condition_setup: vec![
                        hir::Statement {
                            kind: hir::StatementKind::Return { value: None },
                            span: sp(),
                        },
                        hir::Statement {
                            kind: hir::StatementKind::Expr(future_read),
                            span: sp(),
                        },
                    ],
                    cond: bool_expression(&original, export.boolean, false),
                    body: Vec::new(),
                },
                span: sp(),
            }],
            else_body: Some(vec![binding(local, original.clone())]),
        },
        span: sp(),
    });
    parts.source_init = local_read(&original, local, source.ty);
    replace_first_for(&mut export, "main", plan_from_parts(parts));

    hir::validate_iteration_plans(&export)
        .expect("an abrupt loop branch does not participate in normal definition merging");
}

#[test]
fn iteration_definition_owners_include_assignment_only_merge_locals() {
    let mut export = lower_user(file(vec![
        iterator_class("MergedIterator", ty_named("Int")),
        source_class("MergedSource", "MergedIterator"),
        fun(
            "main",
            vec![
                for_stmt(pat_bind("first"), conditional_source(), Vec::new()),
                for_stmt(pat_bind("second"), conditional_source(), Vec::new()),
            ],
        ),
    ]))
    .expect("the producer allocates a distinct branch-result local for each plan");
    let plans = export_body(&export, "main")
        .statements
        .iter()
        .filter_map(|statement| match &statement.kind {
            hir::StatementKind::For(plan) => Some(plan),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [first, second] = plans.as_slice() else {
        panic!("the test function must contain two source loops")
    };
    let hir::ExprKind::Local(first_merge) = &first.source_init().kind else {
        panic!("the conditional source must read its assignment-only merge local")
    };
    let first_merge = *first_merge;
    let mut parts = (**second).clone().into_parts();
    let [conditional] = parts.source_setup.as_mut_slice() else {
        panic!("the second source must contain one conditional setup")
    };
    let hir::StatementKind::If {
        then_body,
        else_body: Some(else_body),
        ..
    } = &mut conditional.kind
    else {
        panic!("the second source setup must be a two-branch conditional")
    };
    replace_assignment_local(then_body, first_merge);
    replace_assignment_local(else_body, first_merge);
    parts.source_init.kind = hir::ExprKind::Local(first_merge);
    replace_nth_for(&mut export, "main", 1, plan_from_parts(parts));

    let error = hir::validate_iteration_plans(&export)
        .expect_err("assignment-only merge locals remain owned by exactly one plan");
    assert_eq!(
        error.message(),
        "iteration plan reuses a local defined outside that plan"
    );
}

#[test]
fn iteration_plan_validator_checks_source_prefix_definite_definition() {
    for invalid in [
        InvalidSourceDefinition::SelfRead,
        InvalidSourceDefinition::ForwardRead,
        InvalidSourceDefinition::ConditionalOnly,
        InvalidSourceDefinition::LoopOnly,
        InvalidSourceDefinition::WhenSubject,
    ] {
        let mut export = checked_basic_iteration_export();
        let source = first_for(export_body(&export, "main")).source();
        let local = allocate_forged_local(&mut export, source.local, "$forged.future");
        let mut parts = first_for(export_body(&export, "main")).clone().into_parts();
        let original = parts.source_init.clone();
        let read = local_read(&original, local, source.ty);
        let declaration = binding(local, original.clone());

        match invalid {
            InvalidSourceDefinition::SelfRead => {
                parts.source_setup.push(binding(local, read));
            }
            InvalidSourceDefinition::ForwardRead => {
                parts.source_setup.push(hir::Statement {
                    kind: hir::StatementKind::Expr(read),
                    span: sp(),
                });
                parts.source_setup.push(declaration);
            }
            InvalidSourceDefinition::ConditionalOnly => {
                parts.source_setup.push(hir::Statement {
                    kind: hir::StatementKind::If {
                        cond: bool_expression(&original, export.boolean, false),
                        then_body: vec![declaration],
                        else_body: None,
                    },
                    span: sp(),
                });
                parts.source_init = local_read(&original, local, source.ty);
            }
            InvalidSourceDefinition::LoopOnly => {
                parts.source_setup.push(hir::Statement {
                    kind: hir::StatementKind::While {
                        target: hir::LoopId::from_raw(u32::MAX - 1),
                        condition_setup: Vec::new(),
                        cond: bool_expression(&original, export.boolean, false),
                        body: vec![declaration],
                    },
                    span: sp(),
                });
                parts.source_init = local_read(&original, local, source.ty);
            }
            InvalidSourceDefinition::WhenSubject => {
                parts.source_setup.push(hir::Statement {
                    kind: hir::StatementKind::When(hir::When {
                        subject: read,
                        arms: vec![hir::WhenArm {
                            pattern: hir::Pattern::Wildcard,
                            guard: None,
                            body: Vec::new(),
                            span: sp(),
                        }],
                        fallback: hir::WhenFallback::Impossible(
                            hir::ExhaustivenessProof::IrrefutableArm {
                                subject_ty: source.ty,
                            },
                        ),
                    }),
                    span: sp(),
                });
                parts.source_setup.push(declaration);
            }
        }
        replace_first_for(&mut export, "main", plan_from_parts(parts));

        let error = hir::validate_iteration_plans(&export)
            .expect_err("source evaluation cannot read a local before every path defines it");
        assert_eq!(
            error.message(),
            "for source prefix references a future plan local"
        );
    }
}

fn allocate_forged_local(module: &mut hir::Module, like: hir::LocalId, name: &str) -> hir::LocalId {
    let hir::FunctionKind::User(body) = &mut module.functions[module.entry].kind else {
        panic!("main has a body")
    };
    let mut declaration = body.locals[like].clone();
    declaration.name = name.to_string();
    declaration.mutable = false;
    body.locals.alloc(declaration)
}

fn local_read(template: &hir::Expr, local: hir::LocalId, ty: hir::TypeId) -> hir::Expr {
    hir::Expr {
        kind: hir::ExprKind::Local(local),
        ty,
        ..template.clone()
    }
}

fn bool_expression(template: &hir::Expr, ty: hir::TypeId, value: bool) -> hir::Expr {
    hir::Expr {
        kind: hir::ExprKind::BoolLiteral(value),
        ty,
        ..template.clone()
    }
}

fn binding(local: hir::LocalId, init: hir::Expr) -> hir::Statement {
    hir::Statement {
        kind: hir::StatementKind::ValDecl {
            pattern: hir::Pattern::Binding { local },
            init,
        },
        span: sp(),
    }
}

fn conditional_source() -> Expr {
    Expr::If(Box::new(ast::If {
        cond: bool_lit(true),
        then_block: block(vec![stmt(call("MergedSource", Vec::new()))]),
        else_block: Some(block(vec![stmt(call("MergedSource", Vec::new()))])),
        span: sp(),
    }))
}

fn replace_assignment_local(statements: &mut [hir::Statement], replacement: hir::LocalId) {
    let [statement] = statements else {
        panic!("a conditional result branch must contain one assignment")
    };
    let hir::StatementKind::Assign {
        target: hir::AssignTarget::Local(local),
        ..
    } = &mut statement.kind
    else {
        panic!("a conditional result branch must assign its merge local")
    };
    *local = replacement;
}
