use super::*;

#[test]
fn an_outer_iteration_object_keeps_mutable_state_through_an_inner_loop() {
    let (mut module, id) = sample(false);
    let body = &mut module.functions[id].body;
    let entry = body.entry;
    let updates = body.blocks[entry].statements.split_off(4);
    let creation = std::mem::take(&mut body.blocks[entry].statements);
    let mut reads = updates;
    reads.push(node(mir::StatementKind::Expr(read(
        value(body, "object"),
        1,
        body.locals[named(body, "before")].ty.clone(),
    ))));
    let outer = body.blocks.alloc(mir::BasicBlock {
        terminator_origin: Default::default(),
        name: "outer.creation".into(),
        statements: creation,
        terminator: mir::Terminator::Unreachable,
        unwind: None,
    });
    let inner = body.blocks.alloc(mir::BasicBlock {
        terminator_origin: Default::default(),
        name: "inner.update".into(),
        statements: reads,
        terminator: mir::Terminator::Unreachable,
        unwind: None,
    });
    body.blocks[entry].terminator = mir::Terminator::Goto(outer);
    body.blocks[outer].terminator = mir::Terminator::Goto(inner);
    body.blocks[inner].terminator = mir::Terminator::Branch {
        cond: value(body, "flag"),
        then_block: inner,
        else_block: outer,
    };
    body.loop_header_polls
        .push(mir::LoopHeaderPollTarget::new(outer));
    body.loop_header_polls
        .push(mir::LoopHeaderPollTarget::new(inner));
    elide(&mut module);
    assert_eq!(counts(&module.functions[id]), (0, 0, 0));
    assert_eq!(module.functions[id].body.loop_header_polls.len(), 2);
}
