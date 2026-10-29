use super::*;

fn stored_condition(function: &mut mir::Function, copies: usize) -> mir::LocalId {
    let body = &mut function.body;
    let entry = body.entry;
    let mir::Terminator::Branch { cond, .. } = &mut body.blocks[entry].terminator else {
        unreachable!()
    };
    let value = std::mem::replace(cond, mir::Expr::bool(false));
    let result = body.locals.alloc(local("checked", mir::Type::Boolean));
    body.blocks[entry].statements.push(val_decl(result, value));
    let mut source = result;
    for index in 0..copies {
        let target = body
            .locals
            .alloc(local(&format!("boolean_copy_{index}"), mir::Type::Boolean));
        body.blocks[entry].statements.push(assign(
            target,
            mir::Expr::new(
                mir::Type::Boolean,
                mir::ExprKind::Unary {
                    op: mir::UnOp::BoolNot,
                    operand: Box::new(local_expr(source, mir::Type::Boolean)),
                },
            ),
        ));
        source = target;
    }
    let mir::Terminator::Branch { cond, .. } = &mut body.blocks[entry].terminator else {
        unreachable!()
    };
    *cond = local_expr(source, mir::Type::Boolean);
    if !copies.is_multiple_of(2) {
        *cond = mir::Expr::new(
            mir::Type::Boolean,
            mir::ExprKind::Unary {
                op: mir::UnOp::BoolNot,
                operand: Box::new(cond.clone()),
            },
        );
    }
    result
}

#[test]
fn stored_boolean_checks_keep_the_original_evaluation_site() {
    for negated in [false, true] {
        for copies in [0, 1, 2] {
            let (mut module, scope, _) = guarded(negated);
            let function = &mut module.functions[scope];
            let result = stored_condition(function, copies);
            // Preserve another use before the branch; no check may be delayed.
            function.body.blocks[function.body.entry]
                .statements
                .push(expr_stmt(local_expr(result, mir::Type::Boolean)));
            let lowered = physical(module, lir::OptimizationMode::Release);
            let function = super::super::function(&lowered, scope);
            assert_eq!(
                runtime_count(function, lir::NoGcRuntimeFunction::ITableFind),
                1
            );
            assert_eq!(
                runtime_count(function, lir::NoGcRuntimeFunction::IsInstance),
                0
            );
            assert_eq!(lookup_count(function), 0);
            let instructions = &function.blocks[function.entry].instructions;
            let query = instructions.iter().position(|instruction| matches!(instruction,
                lir::Instruction::Call { site } if site.destination(&function.call_targets)
                    == lir::CallDestination::Runtime(lir::RuntimeFunction::NoGc(lir::NoGcRuntimeFunction::ITableFind))
            )).unwrap();
            assert!(instructions.iter().skip(query + 1).any(|instruction| matches!(instruction,
                lir::Instruction::Store { local, .. } if function.locals[*local].ty() == &lir::LirType::I1
            )));
        }
    }
}

#[test]
fn changed_objects_or_boolean_storage_do_not_prove_a_view() {
    for change in 0..3 {
        let (mut module, scope, _) = guarded(false);
        let function = &mut module.functions[scope];
        let source = current(function);
        let result = stored_condition(function, 0);
        let statement = match change {
            0 => assign(source, local_expr(function.params[1].local, mir::Type::Any)),
            1 => assign(result, mir::Expr::bool(true)),
            _ => expr_stmt(mir::Expr::new(
                mir::Type::Ptr(Box::new(mir::Type::Boolean)),
                mir::ExprKind::AddressOf {
                    local: result,
                    pointee: Box::new(mir::Type::Boolean),
                },
            )),
        };
        function.body.blocks[function.body.entry]
            .statements
            .push(statement);
        let lowered = physical(module, lir::OptimizationMode::Release);
        let function = super::super::function(&lowered, scope);
        assert_eq!(
            runtime_count(function, lir::NoGcRuntimeFunction::ITableFind),
            0
        );
        assert_eq!(
            runtime_count(function, lir::NoGcRuntimeFunction::IsInstance),
            1
        );
        assert_eq!(lookup_count(function), 1);
    }
}
