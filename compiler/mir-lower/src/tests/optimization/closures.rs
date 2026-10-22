use super::*;

mod sample;

fn scope(module: &mir::Module) -> mir::FunctionId {
    module
        .functions
        .iter()
        .find(|(_, function)| function.name == "scope")
        .unwrap()
        .0
}

fn counts(module: &mir::Module) -> (usize, usize, usize) {
    let mut counts = (0, 0, 0);
    for (_, block) in module.functions[scope(module)].body.blocks.iter() {
        mir::visit_block_exprs(block, &mut |expr| match expr.kind {
            mir::ExprKind::ClosureAlloc { .. } => counts.0 += 1,
            mir::ExprKind::ClosureCapture { .. } => counts.1 += 1,
            mir::ExprKind::Box(_) => counts.2 += 1,
            _ => {}
        });
    }
    counts
}

fn run(module: &mut mir::Module, options: MirOptimizationOptions, rounds: usize) {
    let selected = mir::SelectedExternalMirSet::empty(module.cone);
    crate::optimization::run_to_limit(module, &selected, options, rounds);
    assert_mir_foundation_projection(module);
}

#[test]
fn environment_stays_until_the_callable_use_is_expanded() {
    let mut module = sample::sample(false, 2);
    run(
        &mut module,
        MirOptimizationOptions {
            allocation_elision: true,
            ..Default::default()
        },
        3,
    );
    assert_eq!(counts(&module), (1, 0, 0));
    let mut once = sample::sample(false, 2);
    run(&mut once, MirOptimizationOptions::RELEASE, 1);
    assert_eq!(counts(&once), (1, 0, 0));
    let mut twice = sample::sample(false, 2);
    run(&mut twice, MirOptimizationOptions::RELEASE, 2);
    assert_eq!(counts(&twice), (0, 0, 0));
}

#[test]
fn removing_the_environment_exposes_the_captured_box_in_the_next_round() {
    let mut twice = sample::sample(true, 1);
    run(&mut twice, MirOptimizationOptions::RELEASE, 2);
    assert_eq!(counts(&twice), (0, 0, 1));
    let mut complete = sample::sample(true, 1);
    run(&mut complete, MirOptimizationOptions::RELEASE, 3);
    assert_eq!(counts(&complete), (0, 0, 0));
}

#[test]
fn no_capture_environment_needs_no_placeholder_snapshot() {
    let mut module = sample::sample(false, 0);
    optimize(&mut module);
    assert_eq!(counts(&module), (0, 0, 0));
    assert!(
        module.functions[scope(&module)]
            .body
            .locals
            .iter()
            .all(|(_, local)| !local.name.starts_with("$allocation."))
    );
}

#[test]
fn identity_observation_keeps_the_environment_after_inlining() {
    let mut module = sample::sample(false, 1);
    let id = scope(&module);
    let body = &mut module.functions[id].body;
    let closure = body
        .locals
        .iter()
        .find(|(_, local)| local.name == "closure")
        .unwrap()
        .0;
    let value = mir::Expr::local(closure, body.locals[closure].ty.clone());
    let position = body.blocks[body.entry]
        .statements
        .iter()
        .position(|statement| {
            matches!(statement.kind,
        mir::StatementKind::ValDecl { local, .. } if local == closure)
        })
        .unwrap()
        + 1;
    body.blocks[body.entry].statements.insert(
        position,
        mir::Statement {
            kind: mir::StatementKind::Expr(mir::Expr::new(
                mir::Type::Boolean,
                mir::ExprKind::Binary {
                    op: mir::BinOp::RefEq,
                    lhs: Box::new(value.clone()),
                    rhs: Box::new(value),
                },
            )),
            span: crate::source_span(SPAN),
        },
    );
    optimize(&mut module);
    assert_eq!(counts(&module).0, 1);
}

#[test]
fn snapshots_follow_capture_evaluation_order_and_keep_physical_field_indices() {
    let mut module = sample::sample(false, 2);
    let id = scope(&module);
    let mut expected = Vec::new();
    for (_, block) in module.functions[id].body.blocks.iter_mut() {
        mir::visit_block_exprs_mut(block, &mut |expr| {
            if let mir::ExprKind::ClosureAlloc { captures, .. } = &mut expr.kind {
                captures.sort_by_key(|capture| std::cmp::Reverse(capture.field()));
                expected = captures
                    .iter()
                    .map(|capture| {
                        let mir::ExprKind::Local(local) = capture.value().kind else {
                            panic!("capture local");
                        };
                        (capture.field(), local)
                    })
                    .collect();
            }
        });
    }
    assert_eq!(expected.len(), 2);
    // Changing the inputs after allocation makes both snapshots observable.
    let body = &mut module.functions[id].body;
    for &(_, local) in &expected {
        body.locals[local].mutable = true;
    }
    for (_, block) in body.blocks.iter_mut() {
        let creation = block.statements.iter().position(|statement| {
            let mut found = false;
            mir::visit_statement_exprs(statement, &mut |expression| {
                found |= matches!(expression.kind, mir::ExprKind::ClosureAlloc { .. });
            });
            found
        });
        if let Some(creation) = creation {
            let updates = expected.iter().map(|&(_, local)| mir::Statement {
                kind: mir::StatementKind::Assign {
                    local,
                    value: mir::Expr::integer(mir::MirIntegerConstant::Signed32(99)),
                },
                span: crate::source_span(SPAN),
            });
            block.statements.splice(creation + 1..creation + 1, updates);
        }
    }
    optimize(&mut module);
    assert_eq!(counts(&module), (0, 0, 0));
    let body = &module.functions[id].body;
    let snapshots = body
        .blocks
        .iter()
        .flat_map(|(_, block)| &block.statements)
        .filter_map(|statement| {
            let mir::StatementKind::ValDecl { local, init } = &statement.kind else {
                return None;
            };
            let name = &body.locals[*local].name;
            if !name.starts_with("$allocation.") {
                return None;
            }
            let mir::ExprKind::Local(input) = init.kind else {
                panic!("snapshot copies the original value");
            };
            Some((
                name.rsplit('.').next().unwrap().parse::<u32>().unwrap(),
                input,
            ))
        })
        .collect::<Vec<_>>();
    assert_eq!(snapshots, expected);
}
