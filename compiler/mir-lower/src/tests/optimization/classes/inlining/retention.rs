use super::*;

fn merge_foreign(body: &mut mir::Body, in_callee: bool) {
    let (at, destination) = if in_callee {
        (1, named(body, "alias"))
    } else {
        let (at, statement) = body.blocks[body.entry]
            .statements
            .iter()
            .enumerate()
            .rev()
            .find(|(_, s)| matches!(s.kind, mir::StatementKind::Call(_)))
            .unwrap();
        let operand = statement_call(statement).0.operands().next().unwrap();
        let mir::ExprKind::Local(local) = operand.kind else {
            panic!("local receiver")
        };
        (at, local)
    };
    body.locals[destination].mutable = true;
    let tail = body.blocks[body.entry].statements.split_off(at);
    let terminator = std::mem::replace(
        &mut body.blocks[body.entry].terminator,
        mir::Terminator::Unreachable,
    );
    let join = body.blocks.alloc(mir::BasicBlock {
        terminator_origin: Default::default(),
        name: "merge".into(),
        statements: tail,
        terminator,
        unwind: None,
    });
    let change = body.blocks.alloc(mir::BasicBlock {
        terminator_origin: Default::default(),
        name: "foreign".into(),
        statements: vec![node(mir::StatementKind::Assign {
            local: destination,
            value: value(body, "foreign"),
        })],
        terminator: mir::Terminator::Goto(join),
        unwind: None,
    });
    body.blocks[body.entry].terminator = mir::Terminator::Branch {
        cond: value(body, "flag"),
        then_block: change,
        else_block: join,
    };
}

#[test]
fn receiver_escape_identity_address_and_mixed_aliases_do_not_earn_projection_credit() {
    for in_callee in [false, true] {
        for operation in 0..4 {
            let (mut module, scope, method) = program();
            let opaque = module
                .functions
                .iter()
                .find(|(_, f)| f.name == "opaque")
                .unwrap()
                .0;
            let body = &mut module.functions[if in_callee { method } else { scope }].body;
            let receiver = value(body, if in_callee { "alias" } else { "object" });
            let statement = match operation {
                0 => node(mir::StatementKind::Call(mir::CallEffect::Unit(mir::Call {
                    target: mir::CallTarget::Direct(mir::Callee::User(opaque)),
                    args: vec![receiver],
                }))),
                1 => node(mir::StatementKind::Expr(mir::Expr::new(
                    mir::Type::Boolean,
                    mir::ExprKind::Binary {
                        op: mir::BinOp::RefEq,
                        lhs: Box::new(receiver),
                        rhs: Box::new(value(body, "foreign")),
                    },
                ))),
                2 => node(mir::StatementKind::Expr(mir::Expr::new(
                    mir::Type::Ptr(Box::new(receiver.ty.clone())),
                    mir::ExprKind::AddressOf {
                        local: named(body, if in_callee { "alias" } else { "object" }),
                        pointee: Box::new(receiver.ty),
                    },
                ))),
                _ => {
                    merge_foreign(body, in_callee);
                    run(&mut module, true);
                    assert_eq!(method_calls(&module, scope, method), 1);
                    assert_eq!(counts(&module.functions[scope]).0, 1);
                    continue;
                }
            };
            body.blocks[body.entry].statements.push(statement);
            run(&mut module, true);
            assert_eq!(
                method_calls(&module, scope, method),
                1,
                "{in_callee}, {operation}"
            );
            assert_eq!(counts(&module.functions[scope]).0, 1);
        }
    }
}

#[test]
fn two_remaining_receiver_calls_do_not_share_one_elision_credit() {
    let (mut module, scope, method) = program();
    let body = &mut module.functions[scope].body;
    let call = body.blocks[body.entry]
        .statements
        .iter()
        .rev()
        .find(|s| matches!(s.kind, mir::StatementKind::Call(_)))
        .unwrap()
        .clone();
    body.blocks[body.entry].statements.push(call);
    run(&mut module, true);
    assert_eq!(method_calls(&module, scope, method), 2);
    assert_eq!(counts(&module.functions[scope]).0, 1);
}
