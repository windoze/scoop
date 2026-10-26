use super::*;

mod conditional;
mod exits;

fn sample(kind: hir::IntegerKind, snapshot: bool) -> (mir::Module, mir::FunctionId) {
    let mut h = Harness::new();
    let ty = h.integer(kind);
    let mut locals = Arena::new();
    let n = locals.alloc(local("n", ty));
    let mut index = local("i", ty);
    index.mutable = true;
    let i = locals.alloc(index);
    let mut saved = local("saved", ty);
    saved.mutable = snapshot;
    let saved = locals.alloc(saved);
    let one = integer_lit(&h, kind, 1);
    let increment = integer_binary(
        &mut h,
        kind,
        hir::IntegerBinaryOperator::Add,
        local_ref(saved, ty),
        one,
    );
    let loop_body = if snapshot {
        let captured = locals.alloc(local("captured", ty));
        vec![
            val_decl(saved, integer_lit(&h, kind, 5)),
            val_decl(captured, increment),
            stmt(hir::StatementKind::Assign {
                target: hir::AssignTarget::Local(saved),
                value: local_ref(i, ty),
            }),
            stmt(hir::StatementKind::Assign {
                target: hir::AssignTarget::Local(i),
                value: local_ref(captured, ty),
            }),
        ]
    } else {
        vec![
            val_decl(saved, local_ref(i, ty)),
            stmt(hir::StatementKind::Assign {
                target: hir::AssignTarget::Local(i),
                value: increment,
            }),
        ]
    };
    h.user_fn_full(
        "counted",
        Vec::new(),
        vec![param("n", ty, n)],
        ty,
        hir::Body {
            locals,
            statements: vec![
                val_decl(i, integer_lit(&h, kind, 0)),
                stmt(hir::StatementKind::While {
                    loop_kind: hir::LoopKind::While,
                    target: hir::LoopId::from_raw(0),
                    condition_setup: Vec::new(),
                    cond: bool_lit(&h, false),
                    body: loop_body,
                }),
                stmt(hir::StatementKind::Return {
                    value: local_ref(i, ty),
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
    let mut module = lower(&h.finish(main));
    let id = module
        .functions
        .iter()
        .find(|(_, function)| function.name == "counted")
        .unwrap()
        .0;
    let function = &mut module.functions[id];
    let mir::Type::Integer(kind) = function.params[0].ty else {
        unreachable!()
    };
    let i = function
        .body
        .locals
        .iter()
        .find(|(_, local)| local.name == "i")
        .unwrap()
        .0;
    let header = function.body.loop_header_polls[0].header();
    let mir::Terminator::Branch { cond, .. } = &mut function.body.blocks[header].terminator else {
        unreachable!()
    };
    *cond = mir::Expr::integer_compare(
        mir::IntegerComparisonOperation::new(kind, mir::IntegerComparisonOperator::LessThan),
        mir::Expr::local(i, mir::Type::Integer(kind)),
        mir::Expr::local(function.params[0].local, mir::Type::Integer(kind)),
    );
    (module, id)
}

fn chunk(module: &mut mir::Module) {
    let selected = mir::SelectedExternalMirSet::empty(module.cone);
    crate::optimization::run(
        module,
        &selected,
        MirOptimizationOptions {
            poll_chunking: true,
            ..Default::default()
        },
    );
    assert_mir_foundation_projection(module);
}

#[test]
fn bounded_inner_keeps_original_poll_and_generates_owned_counter() {
    for kind in [hir::IntegerKind::SIGNED_32, hir::IntegerKind::SIGNED_64] {
        let (mut module, id) = sample(kind, false);
        let before = module.functions[id].body.clone();
        chunk(&mut module);
        let body = &module.functions[id].body;
        assert_eq!(body.loop_header_polls, before.loop_header_polls);
        assert_eq!(body.blocks.len(), before.blocks.len() + 3);
        assert_eq!(body.locals.len(), before.locals.len() + 2);
        let saved = body
            .blocks
            .iter()
            .flat_map(|(_, block)| &block.statements)
            .filter_map(|statement| match statement.kind {
                mir::StatementKind::ValDecl { local, .. }
                    if body.locals[local].name.ends_with("saved") =>
                {
                    Some(local)
                }
                _ => None,
            })
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(saved.len(), 2);
        let inner = body
            .blocks
            .iter()
            .find(|(block, value)| {
                matches!(value.terminator,
            mir::Terminator::Branch { then_block, .. } if *block == then_block)
            })
            .unwrap()
            .0;
        assert!(
            !body
                .loop_header_polls
                .iter()
                .any(|target| target.header() == inner)
        );
        // A second producer pass must not wrap the already bounded loop again.
        chunk(&mut module);
        assert_eq!(
            module.functions[id].body.blocks.len(),
            before.blocks.len() + 3
        );
    }
}

#[test]
fn short_loop_nongc_nonunit_step_and_branching_body_keep_polls() {
    for variant in 0..4 {
        let (mut module, id) = sample(hir::IntegerKind::SIGNED_32, false);
        let function = &mut module.functions[id];
        let header = function.body.loop_header_polls[0].header();
        let mir::Terminator::Branch {
            then_block: inner, ..
        } = function.body.blocks[header].terminator
        else {
            unreachable!()
        };
        match variant {
            0 => {
                let mir::Terminator::Branch { cond, .. } =
                    &mut function.body.blocks[header].terminator
                else {
                    unreachable!()
                };
                let mir::ExprKind::IntegerCompare { rhs, .. } = &mut cond.kind else {
                    unreachable!()
                };
                **rhs = mir::Expr::integer(mir::MirIntegerConstant::Signed32(64));
            }
            1 => function.gc_effect = mir::GcEffect::NoGc,
            2 => mir::visit_block_exprs_mut(&mut function.body.blocks[inner], &mut |expr| {
                if matches!(expr.kind, mir::ExprKind::IntegerLiteral(_)) {
                    *expr = mir::Expr::integer(mir::MirIntegerConstant::Signed32(2));
                }
            }),
            _ => {
                function.body.blocks[inner].terminator = mir::Terminator::Branch {
                    cond: mir::Expr::new(mir::Type::Boolean, mir::ExprKind::BoolLiteral(true)),
                    then_block: header,
                    else_block: header,
                };
            }
        }
        let blocks = function.body.blocks.len();
        chunk(&mut module);
        assert_eq!(
            module.functions[id].body.blocks.len(),
            blocks,
            "variant {variant}"
        );
    }
}

#[test]
fn linear_inline_blocks_merge_without_sharing_assigned_temporaries() {
    let (mut module, id) = sample(hir::IntegerKind::SIGNED_64, false);
    let function = &mut module.functions[id];
    let header = function.body.loop_header_polls[0].header();
    let mir::Terminator::Branch {
        then_block: inner, ..
    } = function.body.blocks[header].terminator
    else {
        unreachable!()
    };
    let head_tail = function
        .body
        .blocks
        .alloc(function.body.blocks[header].clone());
    function.body.blocks[header].statements.clear();
    function.body.blocks[header].terminator = mir::Terminator::Goto(head_tail);
    let mut tail = function.body.blocks[inner].clone();
    let first = tail.statements.remove(0);
    let mir::StatementKind::ValDecl { local: saved, init } = first.kind else {
        unreachable!()
    };
    function.body.locals[saved].mutable = true;
    function.body.blocks[inner].statements = vec![mir::Statement {
        kind: mir::StatementKind::Assign {
            local: saved,
            value: init,
        },
        span: first.span,
    }];
    let body_tail = function.body.blocks.alloc(tail);
    function.body.blocks[inner].terminator = mir::Terminator::Goto(body_tail);
    let polls = function.body.loop_header_polls.clone();
    let blocks = function.body.blocks.len();
    chunk(&mut module);
    let body = &module.functions[id].body;
    assert_eq!(body.blocks.len(), blocks + 3);
    assert_eq!(body.loop_header_polls, polls);
    for tail in [head_tail, body_tail] {
        assert!(body.blocks[tail].statements.is_empty());
        assert!(matches!(
            body.blocks[tail].terminator,
            mir::Terminator::Unreachable
        ));
    }
    let assigned = body
        .blocks
        .iter()
        .flat_map(|(_, b)| &b.statements)
        .filter_map(|s| match s.kind {
            mir::StatementKind::Assign { local, .. }
                if body.locals[local].name.ends_with("saved") =>
            {
                Some(local)
            }
            _ => None,
        })
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(assigned.len(), 2);
}

#[test]
fn normal_exit_reads_the_shared_last_iteration_temporary() {
    let (mut module, id) = sample(hir::IntegerKind::SIGNED_64, false);
    let function = &mut module.functions[id];
    let header = function.body.loop_header_polls[0].header();
    let mir::Terminator::Branch { cond, .. } = &mut function.body.blocks[header].terminator else {
        unreachable!()
    };
    let mir::ExprKind::IntegerCompare { rhs, .. } = &mut cond.kind else {
        unreachable!()
    };
    // This loop executes before its final temporary is consumed.
    **rhs = mir::Expr::integer(mir::MirIntegerConstant::Signed64(65));
    let saved = function
        .body
        .locals
        .iter()
        .find(|(_, l)| l.name == "saved")
        .unwrap()
        .0;
    for (_, block) in function.body.blocks.iter_mut() {
        if let mir::Terminator::Return { value } = &mut block.terminator {
            *value = mir::Expr::local(saved, mir::Type::Integer(mir::IntegerKind::SIGNED_64));
        }
    }
    chunk(&mut module);
    let body = &module.functions[id].body;
    assert!(
        body.blocks
            .iter()
            .any(|(_, block)| block.name.starts_with("poll.full."))
    );
    assert_eq!(
        body.locals
            .iter()
            .filter(|(_, local)| local.name.ends_with("saved"))
            .count(),
        1
    );
    let definitions = body.blocks.iter().flat_map(|(_, block)| &block.statements).filter(|statement| {
        matches!(statement.kind, mir::StatementKind::ValDecl { local, .. } if local == saved)
    }).count();
    assert_eq!(definitions, 2);
}

#[test]
fn reassigned_temporary_cannot_reinterpret_an_earlier_snapshot_as_unit_step() {
    for kind in [hir::IntegerKind::SIGNED_32, hir::IntegerKind::SIGNED_64] {
        let (mut module, id) = sample(kind, true);
        let before = format!("{:?}", module.functions[id].body);
        chunk(&mut module);
        assert_eq!(format!("{:?}", module.functions[id].body), before);
    }
}
