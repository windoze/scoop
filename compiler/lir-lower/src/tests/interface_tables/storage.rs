use super::*;

#[test]
fn assignment_and_call_results_invalidate_the_changed_source() {
    for call_result in [false, true] {
        let (mut module, scope) = fixture();
        let replace = module
            .functions
            .iter()
            .find(|(_, function)| function.name == "replace")
            .unwrap()
            .0;
        let function = &mut module.functions[scope];
        let destination = current(function);
        let replacement = local_expr(function.params[1].local, function.params[1].ty.clone());
        let write = if call_result {
            stmt(mir::StatementKind::Call(mir::CallEffect::Value {
                destination,
                call: mir::Call {
                    target: mir::CallTarget::Direct(mir::Callee::User(replace)),
                    args: vec![replacement],
                },
            }))
        } else {
            assign(destination, replacement)
        };
        function.body.blocks[function.body.entry]
            .statements
            .insert(2, write);
        let lowered = physical(module, lir::OptimizationMode::Release);
        assert_eq!(
            lookup_count(super::function(&lowered, scope)),
            2,
            "call result: {call_result}"
        );
    }
}

#[test]
fn addressed_sources_do_not_reuse_metadata() {
    let (mut module, scope) = fixture();
    let function = &mut module.functions[scope];
    let source = current(function);
    let ty = function.body.locals[source].ty.clone();
    function.body.blocks[function.body.entry].statements.insert(
        2,
        expr_stmt(mir::Expr::new(
            mir::Type::Ptr(Box::new(ty.clone())),
            mir::ExprKind::AddressOf {
                local: source,
                pointee: Box::new(ty),
            },
        )),
    );
    let lowered = physical(module, lir::OptimizationMode::Release);
    assert_eq!(lookup_count(super::function(&lowered, scope)), 2);
}

#[test]
fn tables_are_not_carried_into_successor_blocks_or_loops() {
    for backedge in [false, true] {
        let (mut module, scope) = fixture();
        let body = &mut module.functions[scope].body;
        let entry = body.entry;
        let second = body.blocks[entry].statements.pop().unwrap();
        let next = body.blocks.alloc(mir::BasicBlock {
            name: "next".into(),
            statements: vec![second],
            terminator: if backedge {
                mir::Terminator::Goto(entry)
            } else {
                body.blocks[entry].terminator.clone()
            },
            unwind: None,
        });
        body.blocks[entry].terminator = mir::Terminator::Goto(next);
        let lowered = physical(module, lir::OptimizationMode::Release);
        assert_eq!(lookup_count(super::function(&lowered, scope)), 2);
    }
}

#[test]
fn lir_invoke_continuations_clear_the_cache_inside_a_mir_block() {
    let (mut module, scope) = fixture();
    let body = &mut module.functions[scope].body;
    let cleanup = body.blocks.alloc(mir::BasicBlock {
        name: "cleanup".into(),
        statements: vec![stmt(mir::StatementKind::Eh(mir::EhStatement::LandingPad {
            cleanup: true,
        }))],
        terminator: mir::Terminator::Resume,
        unwind: None,
    });
    body.blocks[body.entry].unwind = Some(cleanup);
    let lowered = physical(module, lir::OptimizationMode::Release);
    let function = super::function(&lowered, scope);
    assert!(function.blocks.len() > 2);
    assert_eq!(lookup_count(function), 2);
}
