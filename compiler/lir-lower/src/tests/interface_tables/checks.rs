use super::*;

mod control;
mod loops;
mod temporaries;

fn guarded(negated: bool) -> (mir::Module, mir::FunctionId, mir::BlockId) {
    let (mut module, scope) = fixture();
    let function = &mut module.functions[scope];
    let source = current(function);
    let body = &mut function.body;
    let entry = body.entry;
    let calls = body.blocks[entry].statements.split_off(1);
    let mir::StatementKind::Call(mir::CallEffect::Unit(mir::Call {
        target: mir::CallTarget::Interface { interface, .. },
        ..
    })) = &calls[0].kind
    else {
        panic!("interface fixture")
    };
    let interface = *interface;
    let success = body.blocks.alloc(mir::BasicBlock {
        name: "matched".into(),
        statements: calls,
        terminator: body.blocks[entry].terminator.clone(),
        unwind: None,
    });
    let failure = body.blocks.alloc(mir::BasicBlock {
        name: "missing".into(),
        statements: vec![],
        terminator: body.blocks[entry].terminator.clone(),
        unwind: None,
    });
    let mut cond = mir::Expr::new(
        mir::Type::Boolean,
        mir::ExprKind::IsInstance {
            operand: Box::new(local_expr(source, mir::Type::Any)),
            check_ty: Box::new(mir::Type::Interface(interface)),
        },
    );
    if negated {
        cond = mir::Expr::new(
            mir::Type::Boolean,
            mir::ExprKind::Unary {
                op: mir::UnOp::BoolNot,
                operand: Box::new(cond),
            },
        );
    }
    body.blocks[entry].terminator = mir::Terminator::Branch {
        cond,
        then_block: if negated { failure } else { success },
        else_block: if negated { success } else { failure },
    };
    (module, scope, success)
}

fn runtime_count(function: &lir::Function, operation: lir::NoGcRuntimeFunction) -> usize {
    function.blocks.iter().flat_map(|(_, block)| &block.instructions).filter(|instruction| {
        matches!(instruction, lir::Instruction::Call { site }
            if site.destination(&function.call_targets) == lir::CallDestination::Runtime(lir::RuntimeFunction::NoGc(operation)))
    }).count()
}

#[test]
fn success_and_negated_guards_share_one_query_without_extra_object_roots() {
    for negated in [false, true] {
        let mut roots = Vec::new();
        for mode in [lir::OptimizationMode::Debug, lir::OptimizationMode::Release] {
            let (module, scope, _) = guarded(negated);
            let lowered = physical(module, mode);
            let function = function(&lowered, scope);
            let release = mode == lir::OptimizationMode::Release;
            assert_eq!(
                runtime_count(function, lir::NoGcRuntimeFunction::ITableFind),
                usize::from(release)
            );
            assert_eq!(
                runtime_count(function, lir::NoGcRuntimeFunction::IsInstance),
                usize::from(!release)
            );
            assert_eq!(lookup_count(function), if release { 0 } else { 2 });
            roots.push(
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
                    .collect::<Vec<_>>(),
            );
            assert_eq!(
                function
                    .locals
                    .iter()
                    .filter(|(_, local)| local.ty() == &lir::MANAGED_PTR)
                    .count(),
                1
            );
        }
        assert_eq!(roots[0], roots[1]);
    }
}

#[test]
fn a_write_after_the_first_view_invalidates_the_checked_entry() {
    let (mut module, scope, success) = guarded(false);
    let function = &mut module.functions[scope];
    let source = current(function);
    let replacement = local_expr(function.params[1].local, mir::Type::Any);
    function.body.blocks[success]
        .statements
        .insert(1, assign(source, replacement));
    let lowered = physical(module, lir::OptimizationMode::Release);
    let function = super::function(&lowered, scope);
    assert_eq!(
        runtime_count(function, lir::NoGcRuntimeFunction::ITableFind),
        1
    );
    assert_eq!(lookup_count(function), 1);
}

#[test]
fn boolean_only_and_addressed_sources_keep_the_original_check() {
    for addressed in [false, true] {
        let (mut module, scope, success) = guarded(false);
        let function = &mut module.functions[scope];
        if addressed {
            let source = current(function);
            function.body.blocks[function.body.entry]
                .statements
                .push(expr_stmt(mir::Expr::new(
                    mir::Type::Ptr(Box::new(mir::Type::Any)),
                    mir::ExprKind::AddressOf {
                        local: source,
                        pointee: Box::new(mir::Type::Any),
                    },
                )));
        } else {
            function.body.blocks[success].statements.clear();
        }
        let lowered = physical(module, lir::OptimizationMode::Release);
        let function = super::function(&lowered, scope);
        assert_eq!(
            runtime_count(function, lir::NoGcRuntimeFunction::ITableFind),
            0
        );
        assert_eq!(
            runtime_count(function, lir::NoGcRuntimeFunction::IsInstance),
            1
        );
    }
}
