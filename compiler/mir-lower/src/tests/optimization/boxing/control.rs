use super::*;

#[test]
fn loop_local_boxes_and_dominating_preheader_boxes_can_be_removed() {
    for preheader in [false, true] {
        let (mut module, id) = sample();
        let body = &mut module.functions[id].body;
        let flag = value(body, "flag");
        let entry = body.entry;
        let start = if preheader { 3 } else { 0 };
        let statements = body.blocks[entry].statements.split_off(start);
        let result = std::mem::replace(
            &mut body.blocks[entry].terminator,
            mir::Terminator::Unreachable,
        );
        let mir::Terminator::Return { value: unboxed } = result else {
            panic!("sample return");
        };
        let header = body.blocks.alloc(mir::BasicBlock {
            name: "loop".into(),
            statements,
            terminator: mir::Terminator::Unreachable,
            unwind: None,
        });
        body.blocks[header]
            .statements
            .push(node(mir::StatementKind::Expr(unboxed)));
        let exit = body.blocks.alloc(mir::BasicBlock {
            name: "exit".into(),
            statements: Vec::new(),
            terminator: mir::Terminator::Return { value: number(0) },
            unwind: None,
        });
        body.blocks[entry].terminator = mir::Terminator::Goto(header);
        body.blocks[header].terminator = mir::Terminator::Branch {
            cond: flag,
            then_block: header,
            else_block: exit,
        };
        body.loop_header_polls
            .push(mir::LoopHeaderPollTarget::new(header));
        elide(&mut module);
        assert_eq!(counts(&module.functions[id]), (0, 0, 0));
        assert_eq!(module.functions[id].body.loop_header_polls.len(), 1);
    }
}

#[test]
fn outer_iteration_snapshot_can_be_reused_by_an_inner_loop() {
    let (mut module, id) = sample();
    let body = &mut module.functions[id].body;
    let entry = body.entry;
    let flag = value(body, "flag");
    let mut checks = body.blocks[entry].statements.split_off(3);
    let mir::Terminator::Return { value } = std::mem::replace(
        &mut body.blocks[entry].terminator,
        mir::Terminator::Unreachable,
    ) else {
        panic!("sample return");
    };
    checks.push(node(mir::StatementKind::Expr(value)));
    let statements = std::mem::take(&mut body.blocks[entry].statements);
    let creation = body.blocks.alloc(mir::BasicBlock {
        name: "outer.creation".into(),
        statements,
        terminator: mir::Terminator::Unreachable,
        unwind: None,
    });
    let inner = body.blocks.alloc(mir::BasicBlock {
        name: "inner".into(),
        statements: checks,
        terminator: mir::Terminator::Unreachable,
        unwind: None,
    });
    body.blocks[entry].terminator = mir::Terminator::Goto(creation);
    body.blocks[creation].terminator = mir::Terminator::Goto(inner);
    body.blocks[inner].terminator = mir::Terminator::Branch {
        cond: flag,
        then_block: inner,
        else_block: creation,
    };
    elide(&mut module);
    assert_eq!(counts(&module.functions[id]), (0, 0, 0));
}

#[test]
fn previous_iteration_alias_is_not_replaced_by_the_latest_payload() {
    let (mut module, id) = sample();
    let body = &mut module.functions[id].body;
    let entry = body.entry;
    let flag = value(body, "flag");
    let alias = named(body, "alias");
    let current = value(body, "object");
    let creation = body.blocks[entry].statements.remove(1);
    let bind = body.blocks[entry].statements.remove(1);
    let checks = body.blocks[entry].statements.split_off(1);
    let header = body.blocks.alloc(mir::BasicBlock {
        name: "create".into(),
        statements: vec![creation],
        terminator: mir::Terminator::Unreachable,
        unwind: None,
    });
    let first = body.blocks.alloc(mir::BasicBlock {
        name: "first".into(),
        statements: vec![bind],
        terminator: mir::Terminator::Unreachable,
        unwind: None,
    });
    let later = body.blocks.alloc(mir::BasicBlock {
        name: "later".into(),
        statements: checks,
        terminator: mir::Terminator::Unreachable,
        unwind: None,
    });
    body.blocks[later]
        .statements
        .push(node(mir::StatementKind::Assign {
            local: alias,
            value: current,
        }));
    body.blocks[entry].terminator = mir::Terminator::Goto(header);
    body.blocks[header].terminator = mir::Terminator::Branch {
        cond: flag,
        then_block: first,
        else_block: later,
    };
    body.blocks[first].terminator = mir::Terminator::Goto(header);
    body.blocks[later].terminator = mir::Terminator::Goto(header);
    elide(&mut module);
    assert_eq!(counts(&module.functions[id]).0, 1);
}

#[test]
fn unique_static_origin_without_creation_dominance_is_retained() {
    let (mut module, id) = sample();
    let body = &mut module.functions[id].body;
    let entry = body.entry;
    let cond = value(body, "flag");
    let mut tail = body.blocks[entry].statements.split_off(1);
    let unbox = tail.pop().unwrap();
    assert!(matches!(
        unbox.kind,
        mir::StatementKind::ValDecl {
            init: mir::Expr {
                kind: mir::ExprKind::Unbox(_),
                ..
            },
            ..
        }
    ));
    let result = std::mem::replace(
        &mut body.blocks[entry].terminator,
        mir::Terminator::Unreachable,
    );
    let use_block = body.blocks.alloc(mir::BasicBlock {
        name: "use".into(),
        statements: vec![unbox],
        terminator: result,
        unwind: None,
    });
    let creation = body.blocks.alloc(mir::BasicBlock {
        name: "creation".into(),
        statements: tail,
        terminator: mir::Terminator::Goto(use_block),
        unwind: None,
    });
    body.blocks[entry].terminator = mir::Terminator::Branch {
        cond,
        then_block: creation,
        else_block: use_block,
    };
    elide(&mut module);
    assert_eq!(counts(&module.functions[id]).0, 1);
}
