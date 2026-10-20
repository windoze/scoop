use super::*;

mod uses;
mod work;
use uses::{
    expr_uses, pending_uses, raw, statement_use_def, terminator_successors, terminator_uses,
};
use work::WorkingStatement;

pub(super) fn analyze_sites(
    lowerer: &Lowerer,
    body: &mir::Body,
    calls: &[crate::cfg::CallSite],
) -> Vec<SuspendSite> {
    let work = work::collect(body, calls);
    let block_count = body.blocks.len();
    let mut uses = vec![HashSet::new(); block_count];
    let mut defs = vec![HashSet::new(); block_count];
    let mut unwind_defs = vec![None; block_count];
    for (block_id, block) in body.blocks.iter() {
        let index = raw(block_id);
        for statement in &work.blocks[index] {
            // Every call in this block has the same handler. Definitions only
            // accumulate, so the first call determines which handler inputs
            // are already available without reading the block's entry values.
            if block.unwind.is_some() && statement.can_unwind() && unwind_defs[index].is_none() {
                unwind_defs[index] = Some(defs[index].clone());
            }
            let (statement_uses, statement_defs) = working_use_def(statement);
            for local in statement_uses {
                if !defs[index].contains(&local) {
                    uses[index].insert(local);
                }
            }
            defs[index].extend(statement_defs);
        }
        for local in terminator_uses(&block.terminator) {
            if !defs[index].contains(&local) {
                uses[index].insert(local);
            }
        }
    }

    let mut live_in = vec![HashSet::new(); block_count];
    let mut live_out = vec![HashSet::new(); block_count];
    loop {
        let mut changed = false;
        for (block_id, block) in body.blocks.iter().rev() {
            let index = raw(block_id);
            let mut out = HashSet::new();
            for successor in terminator_successors(block) {
                out.extend(live_in[raw(successor)].iter().copied());
            }
            let mut input = uses[index].clone();
            input.extend(out.iter().filter(|local| !defs[index].contains(local)));
            if let (Some(unwind), Some(prior_defs)) = (block.unwind, &unwind_defs[index]) {
                input.extend(live_in[raw(unwind)].difference(prior_defs).copied());
            }
            if out != live_out[index] || input != live_in[index] {
                live_out[index] = out;
                live_in[index] = input;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }

    let mut sites = Vec::new();
    for (block_id, block) in body.blocks.iter() {
        let mut live = live_out[raw(block_id)].clone();
        live.extend(terminator_uses(&block.terminator));
        for statement in work.blocks[raw(block_id)].iter().rev() {
            let exceptional = block
                .unwind
                .filter(|_| statement.can_unwind())
                .map(|unwind| &live_in[raw(unwind)]);
            let suspension = match statement {
                WorkingStatement::Mir {
                    position,
                    statement,
                } => suspend_effect(lowerer, body, statement).map(|(call, destination, result)| {
                    let (order, pending) = work.calls[&(block_id, *position)];
                    (
                        *position,
                        order,
                        destination,
                        result,
                        SuspendKind::Call(call),
                        pending.clone(),
                    )
                }),
                WorkingStatement::Suspend {
                    position,
                    order,
                    call,
                    pending,
                } => {
                    let result = call
                        .destination
                        .map(|local| body.locals[local].ty.clone())
                        .unwrap_or(mir::Type::Unit);
                    Some((
                        *position,
                        *order,
                        call.destination,
                        result,
                        SuspendKind::Intrinsic((*call).clone()),
                        (*pending).clone(),
                    ))
                }
            };
            if let Some((position, order, destination, result, kind, pending)) = suspension {
                pending_uses(&pending, &mut live);
                let mut saved = live.clone();
                if let Some(exceptional) = exceptional {
                    saved.extend(exceptional.iter().copied());
                }
                let mut live_after: Vec<_> = saved.into_iter().collect();
                live_after.sort_by_key(|local| raw(*local));
                sites.push(DiscoveredSuspendSite {
                    order,
                    block: block_id,
                    statement: position,
                    destination,
                    result,
                    kind,
                    live_after,
                    pending,
                });
            }
            let (statement_uses, statement_defs) = working_use_def(statement);
            for local in statement_defs {
                live.remove(&local);
            }
            // A call result is defined only on success. The handler observes
            // the values from before the call, including later-overwritten locals.
            if let Some(exceptional) = exceptional {
                live.extend(exceptional.iter().copied());
            }
            live.extend(statement_uses);
        }
    }
    let mut semantic_order = sites.iter().map(|site| site.order).collect::<Vec<_>>();
    semantic_order.sort_unstable();
    let ordinals = semantic_order
        .into_iter()
        .enumerate()
        .map(|(ordinal, order)| {
            (
                order,
                u32::try_from(ordinal).expect("coroutine suspension-site count fits u32"),
            )
        })
        .collect::<HashMap<_, _>>();
    sites.sort_by_key(|site| (raw(site.block), site.statement, site.order));
    sites
        .into_iter()
        .enumerate()
        .map(|(index, site)| {
            let identity_ordinal = ordinals[&site.order];
            let one_based = index
                .checked_add(1)
                .expect("coroutine suspension-site count fits usize");
            SuspendSite {
                kind: site.kind,
                resume: ResumeSite {
                    block: site.block,
                    statement: site.statement,
                    destination: site.destination,
                    result: site.result,
                    live_after: site.live_after,
                    pending: site.pending,
                    state: mir::CoroutineSuspendStateId::new(
                        u32::try_from(one_based).expect("coroutine suspension-site count fits u32"),
                    )
                    .expect("coroutine suspension states are one-based"),
                    identity_path: hir::StructuralDefinitionPath::from_first(
                        hir::StructuralPathSegment::new(
                            hir::StructuralDefinitionSiteRole::CoroutineTransform,
                            identity_ordinal,
                        ),
                        [],
                    ),
                },
            }
        })
        .collect()
}

fn working_use_def(
    statement: &WorkingStatement<'_>,
) -> (HashSet<mir::LocalId>, HashSet<mir::LocalId>) {
    match statement {
        WorkingStatement::Mir { statement, .. } => statement_use_def(statement),
        WorkingStatement::Suspend { call, .. } => {
            let mut uses = HashSet::new();
            expr_uses(&call.receiver, &mut uses);
            (uses, call.destination.into_iter().collect())
        }
    }
}

pub(crate) fn suspend_effect(
    lowerer: &Lowerer,
    body: &mir::Body,
    statement: &mir::Statement,
) -> Option<(mir::Call, Option<mir::LocalId>, mir::Type)> {
    let mir::StatementKind::Call(effect) = &statement.kind else {
        return None;
    };
    let (call, destination) = match effect {
        mir::CallEffect::Unit(call) => (call, None),
        mir::CallEffect::Value { destination, call } => (call, Some(*destination)),
    };
    let function = match &call.target {
        mir::CallTarget::Direct(mir::Callee::User(function))
        | mir::CallTarget::Virtual {
            callee: mir::DispatchCallee::User(function),
            ..
        }
        | mir::CallTarget::Interface {
            callee: mir::DispatchCallee::User(function),
            ..
        } => *function,
        mir::CallTarget::Direct(mir::Callee::Monomorphized(instance))
        | mir::CallTarget::Virtual {
            callee: mir::DispatchCallee::Monomorphized(instance),
            ..
        }
        | mir::CallTarget::Interface {
            callee: mir::DispatchCallee::Monomorphized(instance),
            ..
        } => lowerer.instances.meta[*instance].function,
        mir::CallTarget::Closure { function_type, .. }
        | mir::CallTarget::FunctionBridge { function_type, .. } => {
            let signature = &lowerer.shell.function_types[*function_type];
            return signature
                .is_suspend
                .then(|| (call.clone(), destination, signature.return_type.clone()));
        }
        mir::CallTarget::Direct(mir::Callee::External(external))
        | mir::CallTarget::Virtual {
            callee: mir::DispatchCallee::External(external),
            ..
        }
        | mir::CallTarget::Interface {
            callee: mir::DispatchCallee::External(external),
            ..
        } => {
            let suspends = lowerer
                .imported_dependency_callable_map
                .values()
                .any(|target| {
                    target.external() == Some(*external)
                        && target.semantic_signature.effect() == scoop_identity::Effect::Suspend
                });
            return suspends.then(|| {
                (
                    call.clone(),
                    destination,
                    destination
                        .map(|local| body.locals[local].ty.clone())
                        .unwrap_or(mir::Type::Unit),
                )
            });
        }
        mir::CallTarget::Direct(mir::Callee::Extern(_) | mir::Callee::Runtime(_)) => return None,
    };
    // Source signatures are available before ABI conversion; afterwards the
    // same semantic result lives in the complete coroutine metadata.
    let result = lowerer
        .suspend_sources
        .iter()
        .find(|source| source.function == function)
        .map(|source| source.source_return.clone())
        .or_else(|| {
            lowerer
                .coroutines
                .functions
                .iter()
                .find_map(|(_, coroutine)| {
                    (coroutine.function == function).then(|| coroutine.source_return.clone())
                })
        });
    result.map(|result| (call.clone(), destination, result))
}
