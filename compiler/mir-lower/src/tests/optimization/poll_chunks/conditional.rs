use super::*;

fn diamond(variant: usize) -> (mir::Module, mir::FunctionId) {
    let (mut module, id) = sample(hir::IntegerKind::SIGNED_64, false);
    let function = &mut module.functions[id];
    let header = function.body.loop_header_polls[0].header();
    let mir::Terminator::Branch {
        then_block: inner,
        else_block: exit,
        ..
    } = function.body.blocks[header].terminator
    else {
        unreachable!()
    };
    let source = function.body.blocks[inner].clone();
    let latch = function.body.blocks.alloc(mir::BasicBlock {
        name: "joined.update".into(),
        statements: source.statements[1..].to_vec(),
        terminator: mir::Terminator::Goto(header),
        unwind: None,
    });
    let left = function.body.blocks.alloc(mir::BasicBlock {
        name: "left.snapshot".into(),
        statements: source.statements[..1].to_vec(),
        terminator: mir::Terminator::Goto(latch),
        unwind: None,
    });
    let right = function
        .body
        .blocks
        .alloc(function.body.blocks[left].clone());
    let condition = mir::Expr::integer_compare(
        mir::IntegerComparisonOperation::new(
            mir::IntegerKind::SIGNED_64,
            mir::IntegerComparisonOperator::GreaterThan,
        ),
        mir::Expr::local(
            function.params[0].local,
            mir::Type::Integer(mir::IntegerKind::SIGNED_64),
        ),
        mir::Expr::integer(mir::MirIntegerConstant::Signed64(0)),
    );
    function.body.blocks[inner].statements.clear();
    function.body.blocks[inner].terminator = mir::Terminator::Branch {
        cond: condition.clone(),
        then_block: left,
        else_block: right,
    };
    match variant {
        0 => {}
        1 => {
            let mir::StatementKind::ValDecl { init, .. } =
                &mut function.body.blocks[right].statements[0].kind
            else {
                unreachable!()
            };
            *init = mir::Expr::integer(mir::MirIntegerConstant::Signed64(7));
        }
        2 => {
            let update = std::mem::take(&mut function.body.blocks[latch].statements);
            function.body.blocks[left].statements.extend(update);
        }
        3 => {
            function.body.blocks[right].terminator = mir::Terminator::Branch {
                cond: condition,
                then_block: right,
                else_block: latch,
            }
        }
        4 => function.body.blocks[right].terminator = mir::Terminator::Goto(exit),
        _ => unreachable!(),
    }
    (module, id)
}

#[test]
fn branch_snapshots_meet_before_the_common_unit_step() {
    let (mut module, id) = diamond(0);
    let polls = module.functions[id].body.loop_header_polls.clone();
    chunk(&mut module);
    let body = &module.functions[id].body;
    assert_eq!(body.loop_header_polls, polls);
    let full = body
        .blocks
        .iter()
        .filter(|(_, block)| block.name.starts_with("poll.full."))
        .map(|(id, _)| id)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(full.len(), 4);
    let mut backedges = 0;
    for &id in &full {
        match &body.blocks[id].terminator {
            mir::Terminator::Goto(next) => assert!(full.contains(next)),
            mir::Terminator::Branch {
                then_block,
                else_block,
                ..
            } => {
                assert!(full.contains(then_block));
                if *else_block == polls[0].header() {
                    backedges += 1;
                } else {
                    assert!(full.contains(else_block));
                }
            }
            _ => panic!("unexpected copied terminator"),
        }
    }
    assert_eq!(backedges, 1);
    let locals = body
        .locals
        .iter()
        .filter(|(_, local)| local.name.ends_with("saved"))
        .count();
    assert_eq!(locals, 2);
}

#[test]
fn divergent_snapshots_bypassed_updates_and_inner_cycles_keep_polls() {
    for variant in 1..4 {
        let (mut module, id) = diamond(variant);
        let before = format!("{:?}", module.functions[id].body);
        chunk(&mut module);
        assert_eq!(
            format!("{:?}", module.functions[id].body),
            before,
            "variant {variant}"
        );
    }
}

#[test]
fn early_exit_keeps_its_original_code_and_skips_the_latch() {
    let (mut module, id) = diamond(4);
    let function = &module.functions[id];
    let header = function.body.loop_header_polls[0].header();
    let mir::Terminator::Branch {
        then_block: inner, ..
    } = function.body.blocks[header].terminator
    else {
        unreachable!()
    };
    let mir::Terminator::Branch {
        else_block: exit, ..
    } = function.body.blocks[inner].terminator
    else {
        unreachable!()
    };
    let original = format!("{:?}", function.body.blocks[exit]);
    chunk(&mut module);
    let body = &module.functions[id].body;
    assert_eq!(format!("{:?}", body.blocks[exit]), original);
    let full = body.blocks.iter().find(|(_, block)| {
        block.name.starts_with("poll.full.")
            && matches!(block.terminator, mir::Terminator::Branch { else_block, .. } if else_block == exit)
    });
    assert!(
        full.is_some(),
        "the copied branch must reach the shared exit"
    );
    assert_eq!(body.loop_header_polls.len(), 1);
}
