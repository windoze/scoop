use super::*;

#[test]
fn merging_external_values_or_two_allocations_retains_every_candidate() {
    for second_box in [false, true] {
        let (mut module, id) = sample();
        let body = &mut module.functions[id].body;
        let alias = named(body, "alias");
        if second_box {
            body.blocks[body.entry].statements[2] = node(mir::StatementKind::ValDecl {
                local: alias,
                init: mir::Expr::new(mir::Type::Any, mir::ExprKind::Box(Box::new(number(7)))),
            });
        }
        let replacement = value(body, if second_box { "object" } else { "external" });
        body.blocks[body.entry]
            .statements
            .push(node(mir::StatementKind::Assign {
                local: alias,
                value: replacement,
            }));
        elide(&mut module);
        assert_eq!(
            counts(&module.functions[id]),
            (if second_box { 2 } else { 1 }, 1, 2)
        );
    }
}

#[test]
fn unknown_calls_identity_and_aggregate_embedding_keep_the_object() {
    for usage in 0..3 {
        let (mut module, id) = sample();
        let body = &mut module.functions[id].body;
        let object = value(body, "object");
        let kind = match usage {
            0 => mir::StatementKind::Call(mir::CallEffect::Value {
                destination: named(body, "source"),
                call: mir::Call {
                    target: mir::CallTarget::Direct(mir::Callee::User(id)),
                    args: vec![value(body, "input"), object, value(body, "flag")],
                },
            }),
            1 => mir::StatementKind::Expr(mir::Expr::new(
                mir::Type::Boolean,
                mir::ExprKind::Binary {
                    op: mir::BinOp::RefEq,
                    lhs: Box::new(object.clone()),
                    rhs: Box::new(object),
                },
            )),
            _ => mir::StatementKind::Expr(mir::Expr::new(
                mir::Type::Tuple(vec![mir::Type::Any]),
                mir::ExprKind::TupleLiteral(vec![object]),
            )),
        };
        body.blocks[body.entry].statements.push(node(kind));
        elide(&mut module);
        assert_eq!(counts(&module.functions[id]), (1, 1, 0));
    }
}

#[test]
fn taking_an_alias_address_before_assignment_blocks_replacement() {
    let (mut module, id) = sample();
    let body = &mut module.functions[id].body;
    let alias = named(body, "alias");
    body.blocks[body.entry].statements.insert(
        0,
        node(mir::StatementKind::Expr(mir::Expr::new(
            mir::Type::Ptr(Box::new(mir::Type::Any)),
            mir::ExprKind::AddressOf {
                local: alias,
                pointee: Box::new(mir::Type::Any),
            },
        ))),
    );
    assert_eq!(elide(&mut module), (1, true));
    assert_eq!(counts(&module.functions[id]), (1, 1, 2));
}

#[test]
fn unmatched_unbox_retains_the_allocation_while_unrelated_unwinds_do_not() {
    for exception_region in [false, true] {
        let (mut module, id) = sample();
        let body = &mut module.functions[id].body;
        if exception_region {
            let unwind = body.blocks.alloc(mir::BasicBlock {
                name: "unwind".into(),
                statements: Vec::new(),
                terminator: mir::Terminator::Resume,
                unwind: None,
            });
            body.blocks[body.entry].unwind = Some(unwind);
        } else {
            let extra = mir::Expr::new(
                mir::Type::Boolean,
                mir::ExprKind::Unbox(Box::new(value(body, "alias"))),
            );
            body.blocks[body.entry]
                .statements
                .push(node(mir::StatementKind::Expr(extra)));
        }
        elide(&mut module);
        assert_eq!(
            counts(&module.functions[id]).0,
            usize::from(!exception_region)
        );
    }
}
