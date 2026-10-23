use super::*;

#[test]
fn identity_addressed_aliases_and_unknown_calls_keep_the_object() {
    for operation in 0..3 {
        let (mut module, id) = sample(false);
        let body = &mut module.functions[id].body;
        let object = value(body, "object");
        let statement = match operation {
            0 => node(mir::StatementKind::Expr(mir::Expr::new(
                mir::Type::Boolean,
                mir::ExprKind::Binary {
                    op: mir::BinOp::RefEq,
                    lhs: Box::new(object),
                    rhs: Box::new(value(body, "external")),
                },
            ))),
            1 => node(mir::StatementKind::Expr(mir::Expr::new(
                mir::Type::Ptr(Box::new(object.ty.clone())),
                mir::ExprKind::AddressOf {
                    local: named(body, "alias"),
                    pointee: Box::new(object.ty),
                },
            ))),
            _ => node(mir::StatementKind::Call(mir::CallEffect::Value {
                destination: named(body, "before"),
                call: mir::Call {
                    target: mir::CallTarget::Direct(mir::Callee::User(id)),
                    args: vec![
                        value(body, "input"),
                        value(body, "replacement"),
                        value(body, "text"),
                        value(body, "replacement_text"),
                        object,
                        value(body, "flag"),
                    ],
                },
            })),
        };
        body.blocks[body.entry].statements.push(statement);
        elide(&mut module);
        assert_eq!(counts(&module.functions[id]).0, 1);
    }
}

#[test]
fn merging_a_foreign_object_into_an_alias_keeps_both_field_states() {
    let (mut module, id) = sample(false);
    let body = &mut module.functions[id].body;
    let local = named(body, "alias");
    let assignment = node(mir::StatementKind::Assign {
        local,
        value: value(body, "external"),
    });
    body.blocks[body.entry].statements.insert(4, assignment);
    elide(&mut module);
    assert_eq!(counts(&module.functions[id]), (1, 1, 3));
}

#[test]
fn release_hooks_and_unwind_regions_keep_real_allocations() {
    for hook in [false, true] {
        let (mut module, id) = sample(false);
        let body = &mut module.functions[id].body;
        if hook {
            let mir::Type::Class(class) = body.locals[named(body, "object")].ty else {
                unreachable!()
            };
            module.classes[class].release_policy = mir::ReleasePolicy::SynchronousGcFree {
                hook: mir::ReleaseHookTarget::External { owner: class },
            };
        } else {
            let unwind = body.blocks.alloc(mir::BasicBlock {
                name: "unwind".into(),
                statements: vec![],
                terminator: mir::Terminator::Resume,
                unwind: None,
            });
            body.blocks[body.entry].unwind = Some(unwind);
        }
        elide(&mut module);
        assert_eq!(counts(&module.functions[id]).0, 1);
    }
}
