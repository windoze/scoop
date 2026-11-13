use std::collections::BTreeSet;

use scoop_identity::{CallableMaterialization, StructuralDefinitionSiteRole, SyntheticLocalRole};

use super::{Choice, LocalValueRegistry, exception_edges, index, mir};

pub(super) struct Expansion {
    pub(super) continuation: mir::BlockId,
    pub(super) blocks: Vec<Option<mir::BlockId>>,
    pub(super) locals: Vec<Option<mir::LocalId>>,
    pub(super) exception_states: Vec<mir::ExceptionStateId>,
    pub(super) unwind: Option<mir::BlockId>,
    pub(super) result_prefix: usize,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn apply(
    caller: &mut mir::Function,
    callee: &mir::Function,
    choice: Choice,
    registry: &mut LocalValueRegistry,
    debug_records: &mut mir::DebugRecords,
    owner: CallableMaterialization,
    caller_id: mir::FunctionId,
    ancestry: &mut Vec<Vec<super::Target>>,
    auxiliary_locals: &[mir::LocalId],
) -> Expansion {
    let caller_ancestry = ancestry[index(choice.block)].clone();
    let profile_site =
        mir::profile_call_before(&caller.body.blocks[choice.block], choice.statement).cloned();
    let profile_copy = u32::try_from(caller.body.blocks.len()).expect("MIR block indices fit u32");
    let (statement, tail, terminator, terminator_origin, unwind) = {
        let block = &mut caller.body.blocks[choice.block];
        let tail = block.statements.split_off(choice.statement + 1);
        let statement = block
            .statements
            .pop()
            .expect("selected call has a statement");
        let terminator = std::mem::replace(&mut block.terminator, mir::Terminator::Unreachable);
        let origin = std::mem::take(&mut block.terminator_origin);
        (statement, tail, terminator, origin, block.unwind)
    };
    let inline_site = debug_records.inline_site(
        &callee.name,
        callee.debug_origin.clone(),
        statement.debug_origin.clone(),
    );
    let has_exception_paths = choice.blocks.iter().any(|(_, block)| {
        block
            .statements
            .iter()
            .any(|statement| matches!(statement.kind, mir::StatementKind::Eh(_)))
            || matches!(block.terminator, mir::Terminator::Resume { .. })
    });
    let handler = has_exception_paths
        .then(|| unwind.map(|pad| exception_edges::handler(caller, pad)))
        .flatten();
    let exception_states = callee
        .body
        .exception_states
        .iter()
        .map(|(_, state)| caller.body.exception_states.alloc(state.clone()))
        .collect::<Vec<_>>();
    let exception_state = |state: mir::ExceptionStateId| exception_states[index(state)];
    let mir::StatementKind::Call(effect) = statement.kind else {
        unreachable!("only a selected call is expanded")
    };
    let result = match effect {
        mir::CallEffect::Unit(_) => None,
        mir::CallEffect::Value { destination, .. } => Some(destination),
    };
    let call = choice.call;
    let continuation = caller.body.blocks.alloc(mir::BasicBlock {
        terminator_origin,
        name: format!("inline.cont.{}", caller.body.blocks.len()),
        statements: tail,
        terminator,
        unwind,
    });
    ancestry.push(caller_ancestry.clone());
    let result = result.map(|destination| {
        let mut definition = caller.body.locals[destination].clone();
        definition.name = format!("$inline.{}.result", index(continuation));
        definition.mutable = true;
        definition.debug_variable = mir::DebugVariable::Generated;
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
                debug_origin: statement.debug_origin.clone(),
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
    needed.extend(auxiliary_locals.iter().copied());
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
        debug_records.inline_variable(&mut definition.debug_variable, &inline_site);
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
    let mut nested_ancestry = caller_ancestry;
    nested_ancestry.push(choice.callee);
    for (source, block) in &choice.blocks {
        let cloned = caller.body.blocks.alloc(mir::BasicBlock {
            terminator_origin: Default::default(),
            name: format!("inline.{}.{}", index(continuation), block.name),
            statements: Vec::new(),
            terminator: mir::Terminator::Unreachable,
            unwind,
        });
        blocks[index(*source)] = Some(cloned);
        ancestry.push(nested_ancestry.clone());
    }
    let target =
        |old: mir::BlockId| blocks[index(old)].expect("a reachable edge keeps its destination");
    let prefix = &mut caller.body.blocks[choice.block];
    for (parameter, argument) in callee.params.iter().zip(call.args) {
        prefix.statements.push(mir::Statement {
            debug_origin: statement.debug_origin.clone(),
            kind: mir::StatementKind::ValDecl {
                local: local(parameter.local),
                init: argument,
            },
            span: statement.span,
        });
    }
    prefix.terminator = mir::Terminator::Goto(target(callee.body.entry));
    prefix.terminator_origin = statement.debug_origin.clone();

    for (old, mut block) in choice.blocks {
        block.terminator_origin = debug_records.inlined(&block.terminator_origin, &inline_site);
        mir::visit_block_exprs_mut(&mut block, &mut |expression| {
            expression.debug_origin = debug_records.inlined(&expression.debug_origin, &inline_site);
        });
        mir::visit_block_exprs_mut(&mut block, &mut |expression| match &mut expression.kind {
            mir::ExprKind::Local(id) | mir::ExprKind::AddressOf { local: id, .. } => {
                *id = local(*id)
            }
            mir::ExprKind::CaughtException { state } => *state = exception_state(*state),
            _ => {}
        });
        for statement in &mut block.statements {
            statement.debug_origin = debug_records.inlined(&statement.debug_origin, &inline_site);
            match &mut statement.kind {
                mir::StatementKind::ProfileCounter(marker) => {
                    marker.inline_at(profile_site.as_ref(), profile_copy)
                }
                mir::StatementKind::ValDecl { local: id, .. }
                | mir::StatementKind::Assign { local: id, .. }
                | mir::StatementKind::Call(mir::CallEffect::Value {
                    destination: id, ..
                }) => *id = local(*id),
                mir::StatementKind::Eh(mir::EhStatement::LandingPad { cleanup, state }) => {
                    *state = exception_state(*state);
                    if let Some(handler) = handler {
                        *cleanup &= handler.cleanup;
                    }
                }
                mir::StatementKind::Eh(mir::EhStatement::BeginCatch { state }) => {
                    *state = exception_state(*state);
                }
                mir::StatementKind::Eh(mir::EhStatement::Forward {
                    source,
                    destination,
                }) => {
                    *source = exception_state(*source);
                    *destination = exception_state(*destination);
                }
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
                block.statements.push(mir::Statement {
                    debug_origin: block.terminator_origin.clone(),
                    kind: match result {
                        Some(destination) => mir::StatementKind::Assign {
                            local: destination,
                            value,
                        },
                        None => mir::StatementKind::Expr(value),
                    },
                    span: statement.span,
                });
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
            mir::Terminator::Resume { state } => {
                let source = exception_state(state);
                if let Some(handler) = handler {
                    block.statements.push(mir::Statement {
                        debug_origin: block.terminator_origin.clone(),
                        kind: mir::StatementKind::Eh(mir::EhStatement::Forward {
                            source,
                            destination: handler.state,
                        }),
                        span: statement.span,
                    });
                    mir::Terminator::Goto(handler.continuation)
                } else {
                    mir::Terminator::Resume { state: source }
                }
            }
            terminal @ (mir::Terminator::Trap { .. } | mir::Terminator::Unreachable) => terminal,
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
    Expansion {
        continuation,
        blocks,
        locals,
        exception_states,
        unwind,
        result_prefix: usize::from(result.is_some()),
    }
}
