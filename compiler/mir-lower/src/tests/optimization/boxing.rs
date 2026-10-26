use super::*;

mod control;
mod dispatch;
mod observations;
mod retention;

fn sample() -> (mir::Module, mir::FunctionId) {
    let mut h = Harness::new();
    let any = h.any();
    let mut locals = Arena::new();
    let input = locals.alloc(local("input", h.int));
    let external = locals.alloc(local("external", any));
    let flag = locals.alloc(local("flag", h.boolean));
    let mut source = local("source", h.int);
    source.mutable = true;
    let source = locals.alloc(source);
    let object = locals.alloc(local("object", any));
    let mut alias = local("alias", any);
    alias.mutable = true;
    let alias = locals.alloc(alias);
    h.user_fn_full(
        "boxed",
        Vec::new(),
        vec![
            param("input", h.int, input),
            param("external", any, external),
            param("flag", h.boolean, flag),
        ],
        h.int,
        hir::Body {
            locals,
            statements: vec![
                val_decl(source, local_ref(input, h.int)),
                val_decl(
                    object,
                    expr(hir::ExprKind::Box(Box::new(local_ref(source, h.int))), any),
                ),
                val_decl(alias, local_ref(object, any)),
                expr_stmt(expr(
                    hir::ExprKind::IsInstance {
                        operand: Box::new(local_ref(alias, any)),
                        check_ty: h.int,
                    },
                    h.boolean,
                )),
                expr_stmt(expr(
                    hir::ExprKind::IsInstance {
                        operand: Box::new(local_ref(alias, any)),
                        check_ty: h.boolean,
                    },
                    h.boolean,
                )),
                stmt(hir::StatementKind::Return {
                    value: expr(hir::ExprKind::Unbox(Box::new(local_ref(alias, any))), h.int),
                }),
            ],
        },
    );
    let main = h.user_fn(
        "main",
        hir::Body {
            locals: Arena::new(),
            statements: Vec::new(),
        },
    );
    let module = lower(&h.finish(main));
    let function = module
        .functions
        .iter()
        .find(|(_, function)| function.name == "boxed")
        .unwrap()
        .0;
    (module, function)
}

fn named(body: &mir::Body, name: &str) -> mir::LocalId {
    body.locals
        .iter()
        .find(|(_, local)| local.name == name)
        .unwrap()
        .0
}

fn value(body: &mir::Body, name: &str) -> mir::Expr {
    let local = named(body, name);
    mir::Expr::local(local, body.locals[local].ty.clone())
}

fn node(kind: mir::StatementKind) -> mir::Statement {
    mir::Statement {
        kind,
        span: crate::source_span(SPAN),
    }
}

fn number(value: u32) -> mir::Expr {
    mir::Expr::integer(mir::MirIntegerConstant::Signed32(value))
}

fn elide(module: &mut mir::Module) -> (usize, bool) {
    let selected = mir::SelectedExternalMirSet::empty(module.cone);
    let result = crate::optimization::run(
        module,
        &selected,
        MirOptimizationOptions {
            allocation_elision: true,
            ..Default::default()
        },
    );
    assert_mir_foundation_projection(module);
    result
}

fn counts(function: &mir::Function) -> (usize, usize, usize) {
    let mut counts = (0, 0, 0);
    for (_, block) in function.body.blocks.iter() {
        mir::visit_block_exprs(block, &mut |expr| match expr.kind {
            mir::ExprKind::Box(_) => counts.0 += 1,
            mir::ExprKind::Unbox(_) => counts.1 += 1,
            mir::ExprKind::IsInstance { .. } => counts.2 += 1,
            _ => {}
        });
    }
    counts
}

#[test]
fn payload_snapshot_precedes_source_reassignment_and_replaces_aliases() {
    let (mut module, id) = sample();
    let body = &mut module.functions[id].body;
    let source = named(body, "source");
    body.blocks[body.entry].statements.insert(
        2,
        node(mir::StatementKind::Assign {
            local: source,
            value: number(99),
        }),
    );
    assert_eq!(counts(&module.functions[id]), (1, 1, 2));
    assert_eq!(elide(&mut module), (2, true));
    let body = &module.functions[id].body;
    assert_eq!(counts(&module.functions[id]), (0, 0, 0));
    let statements = &body.blocks[body.entry].statements;
    let input = named(body, "input");
    assert!(statements.iter().any(|statement| matches!(statement.kind,
        mir::StatementKind::Assign { local, value: mir::Expr { kind: mir::ExprKind::IntegerLiteral(mir::MirIntegerConstant::Signed32(99)), .. } }
            if local == source)));
    assert!(matches!(body.blocks[body.entry].terminator,
        mir::Terminator::Return { value: mir::Expr { kind: mir::ExprKind::Local(local), .. } }
            if local == input));
    let checks = statements
        .iter()
        .filter_map(|statement| match statement.kind {
            mir::StatementKind::Expr(mir::Expr {
                kind: mir::ExprKind::BoolLiteral(value),
                ..
            }) => Some(value),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(checks, [true, false]);
}

#[test]
fn failing_type_check_prunes_only_the_impossible_success_path() {
    let (mut module, id) = sample();
    let body = &mut module.functions[id].body;
    let check_index = body.blocks[body.entry].statements.iter().position(|statement| matches!(&statement.kind,
        mir::StatementKind::Expr(mir::Expr { kind: mir::ExprKind::IsInstance { check_ty, .. }, .. }) if **check_ty == mir::Type::Boolean
    )).unwrap();
    let check = match body.blocks[body.entry].statements.remove(check_index).kind {
        mir::StatementKind::Expr(check) => check,
        _ => panic!("last check is Int is Boolean"),
    };
    let success = body.blocks.alloc(mir::BasicBlock {
        name: "impossible".into(),
        statements: vec![node(mir::StatementKind::Expr(value(body, "alias")))],
        terminator: mir::Terminator::Return { value: number(1) },
        unwind: None,
    });
    let failure = body.blocks.alloc(mir::BasicBlock {
        name: "failure".into(),
        statements: Vec::new(),
        terminator: mir::Terminator::Trap {
            message: "preserved cast failure".into(),
        },
        unwind: None,
    });
    body.blocks[body.entry].terminator = mir::Terminator::Branch {
        cond: check,
        then_block: success,
        else_block: failure,
    };
    elide(&mut module);
    let body = &module.functions[id].body;
    assert_eq!(counts(&module.functions[id]), (0, 0, 0));
    assert!(
        matches!(body.blocks[body.entry].terminator, mir::Terminator::Goto(target) if target == failure)
    );
    assert!(matches!(
        body.blocks[failure].terminator,
        mir::Terminator::Trap { .. }
    ));
    assert!(matches!(
        body.blocks[success].terminator,
        mir::Terminator::Unreachable
    ));
}
