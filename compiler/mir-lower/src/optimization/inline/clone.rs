use std::collections::BTreeSet;

use scoop_identity::{CallableMaterialization, StructuralDefinitionSiteRole, SyntheticLocalRole};

use super::{Choice, LocalValueRegistry, index, mir};

pub(super) fn apply(
    caller: &mut mir::Function,
    callee: &mir::Function,
    choice: Choice,
    registry: &mut LocalValueRegistry,
    owner: CallableMaterialization,
    caller_id: mir::FunctionId,
    depth: &mut Vec<u8>,
) {
    let caller_depth = depth[index(choice.block)];
    let (statement, tail, terminator, unwind) = {
        let block = &mut caller.body.blocks[choice.block];
        let tail = block.statements.split_off(choice.statement + 1);
        let statement = block
            .statements
            .pop()
            .expect("selected call has a statement");
        let terminator = std::mem::replace(&mut block.terminator, mir::Terminator::Unreachable);
        (statement, tail, terminator, block.unwind)
    };
    let mir::StatementKind::Call(effect) = statement.kind else {
        unreachable!("only a selected call is expanded")
    };
    let (result, call) = match effect {
        mir::CallEffect::Unit(call) => (None, call),
        mir::CallEffect::Value { destination, call } => (Some(destination), call),
    };
    let continuation = caller.body.blocks.alloc(mir::BasicBlock {
        name: format!("inline.cont.{}", caller.body.blocks.len()),
        statements: tail,
        terminator,
        unwind,
    });
    depth.push(caller_depth);
    let result = result.map(|destination| {
        let mut definition = caller.body.locals[destination].clone();
        definition.name = format!("$inline.{}.result", index(continuation));
        definition.mutable = true;
        let ty = definition.ty.clone();
        let temporary = caller.body.locals.alloc(definition);
        registry.record_generated_local(
            caller_id,
            temporary,
            owner,
            StructuralDefinitionSiteRole::SyntheticValue,
            SyntheticLocalRole::Temporary,
        );
        caller.body.blocks[continuation].statements.insert(
            0,
            mir::Statement {
                kind: mir::StatementKind::ValDecl {
                    local: destination,
                    init: mir::Expr::new(ty, mir::ExprKind::Local(temporary)),
                },
                span: statement.span,
            },
        );
        temporary
    });

    let mut needed = BTreeSet::new();
    for parameter in &callee.params {
        needed.insert(parameter.local);
    }
    for (_, block) in &choice.blocks {
        mir::visit_block_exprs(block, &mut |expression| {
            if let mir::ExprKind::Local(local) | mir::ExprKind::AddressOf { local, .. } =
                expression.kind
            {
                needed.insert(local);
            }
        });
        for statement in &block.statements {
            if let mir::StatementKind::ValDecl { local, .. }
            | mir::StatementKind::Assign { local, .. }
            | mir::StatementKind::Call(mir::CallEffect::Value {
                destination: local, ..
            }) = statement.kind
            {
                needed.insert(local);
            }
        }
    }
    let mut locals = vec![None; callee.body.locals.len()];
    for local in needed {
        let mut definition = callee.body.locals[local].clone();
        definition.name = format!("$inline.{}.{}", index(continuation), definition.name);
        let cloned = caller.body.locals.alloc(definition);
        locals[index(local)] = Some(cloned);
        registry.record_generated_local(
            caller_id,
            cloned,
            owner,
            StructuralDefinitionSiteRole::SyntheticValue,
            SyntheticLocalRole::Temporary,
        );
    }
    let local =
        |old: mir::LocalId| locals[index(old)].expect("every referenced local has a caller copy");
    let mut blocks = vec![None; callee.body.blocks.len()];
    for (source, block) in &choice.blocks {
        let cloned = caller.body.blocks.alloc(mir::BasicBlock {
            name: format!("inline.{}.{}", index(continuation), block.name),
            statements: Vec::new(),
            terminator: mir::Terminator::Unreachable,
            unwind,
        });
        blocks[index(*source)] = Some(cloned);
        depth.push(caller_depth + 1);
    }
    let target =
        |old: mir::BlockId| blocks[index(old)].expect("a reachable edge keeps its destination");
    let prefix = &mut caller.body.blocks[choice.block];
    for (parameter, argument) in callee.params.iter().zip(call.args) {
        prefix.statements.push(mir::Statement {
            kind: mir::StatementKind::ValDecl {
                local: local(parameter.local),
                init: argument,
            },
            span: statement.span,
        });
    }
    prefix.terminator = mir::Terminator::Goto(target(callee.body.entry));

    for (old, mut block) in choice.blocks {
        mir::visit_block_exprs_mut(&mut block, &mut |expression| match &mut expression.kind {
            mir::ExprKind::Local(id) | mir::ExprKind::AddressOf { local: id, .. } => {
                *id = local(*id)
            }
            _ => {}
        });
        for statement in &mut block.statements {
            match &mut statement.kind {
                mir::StatementKind::ValDecl { local: id, .. }
                | mir::StatementKind::Assign { local: id, .. }
                | mir::StatementKind::Call(mir::CallEffect::Value {
                    destination: id, ..
                }) => *id = local(*id),
                _ => {}
            }
        }
        block.unwind = block.unwind.map(target).or(unwind);
        block.terminator = match block.terminator {
            mir::Terminator::Goto(id) => mir::Terminator::Goto(target(id)),
            mir::Terminator::Branch {
                cond,
                then_block,
                else_block,
            } => mir::Terminator::Branch {
                cond,
                then_block: target(then_block),
                else_block: target(else_block),
            },
            mir::Terminator::Return { value } => {
                if let Some(value) = value {
                    block.statements.push(mir::Statement {
                        kind: match result {
                            Some(destination) => mir::StatementKind::Assign {
                                local: destination,
                                value,
                            },
                            None => mir::StatementKind::Expr(value),
                        },
                        span: statement.span,
                    });
                }
                mir::Terminator::Goto(continuation)
            }
            mir::Terminator::Throw {
                exception,
                unwind: exit,
            } => mir::Terminator::Throw {
                exception,
                unwind: exit.map(target).or(unwind),
            },
            mir::Terminator::Rethrow { unwind: exit } => mir::Terminator::Rethrow {
                unwind: exit.map(target).or(unwind),
            },
            terminal @ (mir::Terminator::Resume
            | mir::Terminator::Trap { .. }
            | mir::Terminator::Unreachable) => terminal,
        };
        block.name = caller.body.blocks[target(old)].name.clone();
        caller.body.blocks[target(old)] = block;
    }
    if callee.gc_effect == mir::GcEffect::Managed {
        caller.body.loop_header_polls.extend(
            callee.body.loop_header_polls.iter().filter_map(|poll| {
                blocks[index(poll.header())].map(mir::LoopHeaderPollTarget::new)
            }),
        );
    }
}
