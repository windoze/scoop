use super::*;

fn looping(parameter: bool) -> (mir::Module, mir::FunctionId, mir::BlockId, mir::BlockId) {
    let (mut module, scope, success) = guarded(false);
    let function = &mut module.functions[scope];
    let source = current(function);
    let parameter_local = function.params[0].local;
    let body = &mut function.body;
    if parameter {
        for (_, block) in body.blocks.iter_mut() {
            mir::visit_block_exprs_mut(block, &mut |expression| {
                if matches!(expression.kind, mir::ExprKind::Local(id) if id == source) {
                    expression.kind = mir::ExprKind::Local(parameter_local);
                }
            });
        }
    }
    let condition = body.blocks[body.entry].terminator.clone();
    let mir::Terminator::Branch { else_block, .. } = condition else {
        panic!("guarded fixture")
    };
    let query = body.blocks.alloc(mir::BasicBlock {
        terminator_origin: Default::default(),
        name: "query".into(),
        statements: Vec::new(),
        terminator: condition,
        unwind: None,
    });
    body.blocks[body.entry].terminator = mir::Terminator::Goto(query);
    body.blocks[success].terminator = mir::Terminator::Goto(query);
    body.blocks[else_block].terminator = mir::Terminator::Goto(query);
    (module, scope, query, success)
}

fn cached_entries(function: &lir::Function) -> Vec<lir::LocalId> {
    function.blocks[function.entry]
        .instructions
        .iter()
        .filter_map(|instruction| {
            if let lir::Instruction::Store {
                local,
                value: lir::Value::NullPointer(lir::PointerKind::Metadata),
            } = instruction
            {
                Some(*local)
            } else {
                None
            }
        })
        .collect()
}

fn managed_roots(function: &lir::Function) -> Vec<usize> {
    function
        .blocks
        .iter()
        .flat_map(|(_, block)| &block.instructions)
        .filter_map(|instruction| match instruction {
            lir::Instruction::Call {
                site: lir::CallSite::Managed(site),
            } => Some(site.live.as_slice().len()),
            _ => None,
        })
        .collect()
}

#[test]
fn stable_loop_queries_share_the_checked_entry_without_extra_roots() {
    for parameter in [false, true] {
        let mut roots = Vec::new();
        for mode in [lir::OptimizationMode::Debug, lir::OptimizationMode::Release] {
            let (module, scope, _, _) = looping(parameter);
            let lowered = physical(module, mode);
            let function = function(&lowered, scope);
            let release = mode == lir::OptimizationMode::Release;
            assert_eq!(cached_entries(function).len(), usize::from(release));
            assert_eq!(
                runtime_count(function, lir::NoGcRuntimeFunction::ITableFind),
                usize::from(release)
            );
            assert_eq!(
                runtime_count(function, lir::NoGcRuntimeFunction::IsInstance),
                usize::from(!release)
            );
            assert_eq!(lookup_count(function), if release { 0 } else { 2 });
            assert_eq!(
                function
                    .locals
                    .iter()
                    .filter(|(_, local)| local.ty() == &lir::MANAGED_PTR)
                    .count(),
                1
            );
            roots.push(managed_roots(function));
        }
        assert_eq!(roots[0], roots[1]);
    }
}

#[test]
fn boolean_and_empty_queries_keep_misses_distinct_from_uninitialized_state() {
    for empty in [false, true] {
        let (mut module, scope, query, success) = looping(true);
        module.functions[scope].body.blocks[success]
            .statements
            .clear();
        let mir::Terminator::Branch { cond, .. } =
            &module.functions[scope].body.blocks[query].terminator
        else {
            unreachable!()
        };
        let mir::ExprKind::IsInstance { check_ty, .. } = &cond.kind else {
            unreachable!()
        };
        let mir::Type::Interface(interface) = **check_ty else {
            unreachable!()
        };
        if empty {
            module.interfaces[interface].methods.clear();
        }
        let lowered = physical(module, lir::OptimizationMode::Release);
        let function = function(&lowered, scope);
        let entries = cached_entries(function);
        assert_eq!(entries.len(), 1);
        assert_eq!(
            runtime_count(function, lir::NoGcRuntimeFunction::ITableFind),
            1
        );
        assert_eq!(
            runtime_count(function, lir::NoGcRuntimeFunction::IsInstance),
            0
        );
        assert_eq!(lookup_count(function), 0);
        let initialized = function.blocks[function.entry]
            .instructions
            .iter()
            .find_map(|instruction| match instruction {
                lir::Instruction::Store {
                    local,
                    value: lir::Value::BoolConst(false),
                } => Some(*local),
                _ => None,
            })
            .unwrap();
        let (_, query_block) = function.blocks.iter().find(|(_, block)| block.instructions.iter().any(|instruction| {
            matches!(instruction, lir::Instruction::Call { site } if site.destination(&function.call_targets)
                == lir::CallDestination::Runtime(lir::RuntimeFunction::NoGc(lir::NoGcRuntimeFunction::ITableFind)))
        })).unwrap();
        assert!(query_block.instructions.iter().any(|instruction| {
            matches!(instruction, lir::Instruction::Store { local, value: lir::Value::BoolConst(true) } if *local == initialized)
        }));
        assert!(function.blocks.iter().any(|(_, block)| matches!(block.terminator,
            lir::Terminator::CondBr { cond: lir::Value::Local(local), .. } if local == initialized)));
        assert!(function.blocks.iter().flat_map(|(_, block)| &block.instructions).any(|instruction| {
            matches!(instruction, lir::Instruction::BinOp { op: lir::BinOp::Ne, lhs: lir::Value::Local(local),
                rhs: lir::Value::NullPointer(lir::PointerKind::Metadata), .. } if *local == entries[0])
        }));
    }
}

#[test]
fn rewritten_addressed_and_loop_defined_sources_keep_per_use_queries() {
    for case in 0..3 {
        let (mut module, scope, query, success) = looping(false);
        let function = &mut module.functions[scope];
        let source = current(function);
        let replacement = local_expr(function.params[1].local, mir::Type::Any);
        let body = &mut function.body;
        match case {
            0 => body.blocks[success]
                .statements
                .push(assign(source, replacement)),
            1 => {
                let initializer = body.blocks[body.entry].statements.remove(0);
                body.blocks[query].statements.push(initializer);
            }
            2 => body.blocks[body.entry]
                .statements
                .push(expr_stmt(mir::Expr::new(
                    mir::Type::Ptr(Box::new(mir::Type::Any)),
                    mir::ExprKind::AddressOf {
                        local: source,
                        pointee: Box::new(mir::Type::Any),
                    },
                ))),
            _ => unreachable!(),
        }
        let lowered = physical(module, lir::OptimizationMode::Release);
        assert!(cached_entries(super::super::function(&lowered, scope)).is_empty());
    }
}

#[test]
fn exceptional_retries_reuse_parameters_but_not_repeated_initializers() {
    for parameter in [false, true] {
        let (mut module, scope, _, success) = looping(parameter);
        let body = &mut module.functions[scope].body;
        let retry = body.blocks.alloc(mir::BasicBlock {
            terminator_origin: Default::default(),
            name: "retry".into(),
            statements: vec![
                stmt(mir::StatementKind::Eh(mir::EhStatement::LandingPad {
                    cleanup: false,
                })),
                stmt(mir::StatementKind::Eh(mir::EhStatement::BeginCatch)),
                stmt(mir::StatementKind::Eh(mir::EhStatement::EndCatch)),
            ],
            terminator: mir::Terminator::Goto(body.entry),
            unwind: None,
        });
        body.blocks[success].unwind = Some(retry);
        let lowered = physical(module, lir::OptimizationMode::Release);
        assert_eq!(
            cached_entries(function(&lowered, scope)).len(),
            usize::from(parameter)
        );
    }
}

#[test]
fn exact_interfaces_use_independent_cache_entries() {
    let (mut module, scope, query, _) = looping(false);
    let other = module
        .interfaces
        .iter()
        .find(|(_, interface)| interface.name == "Other")
        .unwrap()
        .0;
    let source = current(&module.functions[scope]);
    module.functions[scope].body.blocks[query]
        .statements
        .push(expr_stmt(mir::Expr::new(
            mir::Type::Boolean,
            mir::ExprKind::IsInstance {
                operand: Box::new(local_expr(source, mir::Type::Any)),
                check_ty: Box::new(mir::Type::Interface(other)),
            },
        )));
    let lowered = physical(module, lir::OptimizationMode::Release);
    let function = function(&lowered, scope);
    assert_eq!(cached_entries(function).len(), 2);
    assert_eq!(
        runtime_count(function, lir::NoGcRuntimeFunction::ITableFind),
        2
    );
}

#[test]
fn a_mir_entry_backedge_does_not_reset_the_cache() {
    let (mut module, scope, query, _) = looping(true);
    module.functions[scope].body.entry = query;
    let lowered = physical(module, lir::OptimizationMode::Release);
    let function = function(&lowered, scope);
    assert_eq!(cached_entries(function).len(), 1);
    assert!(
        matches!(function.blocks[function.entry].terminator, lir::Terminator::Br(target) if target != function.entry)
    );
    for (_, block) in function.blocks.iter() {
        match block.terminator {
            lir::Terminator::Br(target) => assert_ne!(target, function.entry),
            lir::Terminator::CondBr {
                then_block,
                else_block,
                ..
            } => {
                assert_ne!(then_block, function.entry);
                assert_ne!(else_block, function.entry);
            }
            _ => {}
        }
    }
}
