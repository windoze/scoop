use super::*;

#[test]
fn a_guard_before_a_loop_is_reused_only_without_source_writes() {
    for write in [false, true] {
        let (mut module, scope, success) = guarded(false);
        let function = &mut module.functions[scope];
        let source = current(function);
        if write {
            let replacement = local_expr(function.params[1].local, mir::Type::Any);
            function.body.blocks[success]
                .statements
                .push(assign(source, replacement));
        }
        function.body.blocks[success].terminator = mir::Terminator::Goto(success);
        let lowered = physical(module, lir::OptimizationMode::Release);
        let function = super::super::function(&lowered, scope);
        assert_eq!(
            runtime_count(function, lir::NoGcRuntimeFunction::ITableFind),
            usize::from(!write)
        );
        assert_eq!(lookup_count(function), usize::from(write));
    }
}

#[test]
fn a_merge_reuses_only_a_guard_shared_by_every_reachable_predecessor() {
    for checked_paths in [false, true] {
        let (mut module, scope, success) = guarded(false);
        let body = &mut module.functions[scope].body;
        let calls = std::mem::take(&mut body.blocks[success].statements);
        let merge = body.blocks.alloc(mir::BasicBlock {
            terminator_origin: Default::default(),
            name: "merge".into(),
            statements: calls,
            terminator: body.blocks[success].terminator.clone(),
            unwind: None,
        });
        body.blocks[success].terminator = mir::Terminator::Goto(merge);
        if checked_paths {
            let other = body.blocks.alloc(mir::BasicBlock {
                terminator_origin: Default::default(),
                name: "other_checked".into(),
                statements: vec![],
                terminator: mir::Terminator::Goto(merge),
                unwind: None,
            });
            body.blocks[success].terminator = mir::Terminator::Branch {
                cond: mir::Expr::bool(true),
                then_block: merge,
                else_block: other,
            };
        } else {
            let mir::Terminator::Branch { else_block, .. } = body.blocks[body.entry].terminator
            else {
                unreachable!()
            };
            body.blocks[else_block].terminator = mir::Terminator::Goto(merge);
        }
        let lowered = physical(module, lir::OptimizationMode::Release);
        let function = super::super::function(&lowered, scope);
        assert_eq!(
            runtime_count(function, lir::NoGcRuntimeFunction::ITableFind),
            usize::from(checked_paths)
        );
        assert_eq!(lookup_count(function), usize::from(!checked_paths));
    }
}

#[test]
fn invoke_normal_continuations_keep_guarded_metadata() {
    let (mut module, scope, success) = guarded(false);
    let body = &mut module.functions[scope].body;
    let cleanup = body.blocks.alloc(mir::BasicBlock {
        terminator_origin: Default::default(),
        name: "cleanup".into(),
        statements: vec![stmt(mir::StatementKind::Eh(mir::EhStatement::LandingPad {
            cleanup: true,
        }))],
        terminator: mir::Terminator::Resume,
        unwind: None,
    });
    body.blocks[success].unwind = Some(cleanup);
    let lowered = physical(module, lir::OptimizationMode::Release);
    let function = super::super::function(&lowered, scope);
    assert_eq!(
        runtime_count(function, lir::NoGcRuntimeFunction::ITableFind),
        1
    );
    assert_eq!(lookup_count(function), 0);
    assert!(
        function
            .blocks
            .iter()
            .any(|(_, block)| block.name.starts_with("invoke.normal"))
    );
}

#[test]
fn block_allocation_order_does_not_control_query_availability() {
    let (mut module, scope, success) = guarded(false);
    let body = &mut module.functions[scope].body;
    let old_entry = body.entry;
    let entry = body.blocks.alloc(body.blocks[old_entry].clone());
    body.blocks[old_entry].statements.clear();
    body.blocks[old_entry].terminator = mir::Terminator::Unreachable;
    body.entry = entry;
    assert!(success < entry);
    let lowered = physical(module, lir::OptimizationMode::Release);
    let function = super::super::function(&lowered, scope);
    assert_eq!(
        runtime_count(function, lir::NoGcRuntimeFunction::ITableFind),
        1
    );
    assert_eq!(lookup_count(function), 0);
}
