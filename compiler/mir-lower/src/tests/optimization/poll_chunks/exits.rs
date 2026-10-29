use super::*;

#[test]
fn exit_reads_the_current_iteration_snapshot_from_either_body() {
    let (mut module, id) = sample(hir::IntegerKind::SIGNED_64, false);
    let function = &mut module.functions[id];
    let header = function.body.loop_header_polls[0].header();
    let mir::Terminator::Branch {
        then_block: inner, ..
    } = function.body.blocks[header].terminator
    else {
        unreachable!()
    };
    let source = function.body.blocks[inner].clone();
    let mir::StatementKind::ValDecl { local: saved, .. } = source.statements[0].kind else {
        unreachable!()
    };
    let snapshot = mir::Expr::local(saved, mir::Type::Integer(mir::IntegerKind::SIGNED_64));
    let exit = function.body.blocks.alloc(mir::BasicBlock {
        terminator_origin: Default::default(),
        name: "found.snapshot".into(),
        statements: Vec::new(),
        terminator: mir::Terminator::Return {
            value: snapshot.clone(),
        },
        unwind: None,
    });
    let latch = function.body.blocks.alloc(mir::BasicBlock {
        terminator_origin: Default::default(),
        name: "search.update".into(),
        statements: source.statements[1..].to_vec(),
        terminator: mir::Terminator::Goto(header),
        unwind: None,
    });
    function.body.blocks[inner].statements.truncate(1);
    function.body.blocks[inner].terminator = mir::Terminator::Branch {
        cond: mir::Expr::integer_compare(
            mir::IntegerComparisonOperation::new(
                mir::IntegerKind::SIGNED_64,
                mir::IntegerComparisonOperator::Equal,
            ),
            snapshot,
            mir::Expr::integer(mir::MirIntegerConstant::Signed64(63)),
        ),
        then_block: exit,
        else_block: latch,
    };
    let before = format!("{:?}", function.body.blocks[exit]);
    chunk(&mut module);
    let body = &module.functions[id].body;
    assert_eq!(format!("{:?}", body.blocks[exit]), before);
    let full = body.blocks.iter().find(|(_, block)| {
        block.name.starts_with("poll.full.")
            && matches!(block.terminator, mir::Terminator::Branch { then_block, .. } if then_block == exit)
    }).expect("early return must be reachable from the bounded body").1;
    assert!(
        matches!(full.statements[0].kind, mir::StatementKind::ValDecl { local, .. } if local == saved)
    );
    assert_eq!(
        body.locals
            .iter()
            .filter(|(_, local)| local.name.ends_with("saved"))
            .count(),
        1
    );
    assert_eq!(body.loop_header_polls.len(), 1);
}
