//! Project only a path whose branches follow from the counted-loop range.

use std::collections::BTreeSet;

use super::{Loop, header, linear, mir, operations, recognize, values::Length};
use crate::optimization::graph::{block_edges, index};

pub(super) mod facts;

pub(super) struct Projection {
    pub counted: Loop,
    pub body: mir::Body,
    pub guards: Vec<Length>,
}

pub(super) fn project(
    module: &mir::Module,
    function: &mir::Function,
    target: mir::BlockId,
    head: &header::Header,
) -> Option<Projection> {
    let mut state = facts::State::new(head);
    for &id in &head.chain {
        for statement in &function.body.blocks[id].statements {
            state.statement(statement);
        }
    }
    let mut seen = head.chain.iter().copied().collect::<BTreeSet<_>>();
    let mut branches = Vec::new();
    let mut current = head.inner;
    let mut array_access = false;
    loop {
        if !seen.insert(current)
            || current == head.exit
            || function
                .body
                .loop_header_polls
                .iter()
                .any(|poll| poll.header() == current)
        {
            return None;
        }
        let block = &function.body.blocks[current];
        if block.unwind.is_some() || !operations::supported(module, block, true) {
            return None;
        }
        for statement in &block.statements {
            array_access |= matches!(statement.kind, mir::StatementKind::ArraySet { .. });
            mir::visit_statement_exprs(statement, &mut |expr| {
                array_access |= matches!(expr.kind, mir::ExprKind::ArrayGet { .. });
            });
            state.statement(statement);
        }
        current = match &block.terminator {
            mir::Terminator::Goto(next) => *next,
            mir::Terminator::Branch {
                cond,
                then_block,
                else_block,
            } => {
                let facts::Value::Boolean(value) = state.expression(cond) else {
                    return None;
                };
                let next = if value { *then_block } else { *else_block };
                branches.push((current, next));
                next
            }
            _ => return None,
        };
        if current == target {
            break;
        }
    }
    if !array_access
        || branches.is_empty()
        || (!state.guards.is_empty() && head.kind != mir::IntegerKind::SIGNED_64)
    {
        return None;
    }
    let mut projected = function.clone();
    for (block, next) in branches {
        projected.body.blocks[block].terminator = mir::Terminator::Goto(next);
    }
    prune(&mut projected.body);
    let incoming = linear::predecessors(&projected.body);
    let projected_head = header::analyze(module, &projected, target, &incoming)?;
    let counted = recognize::counted(module, &projected, target, &projected_head, &incoming)?;
    Some(Projection {
        counted,
        body: projected.body,
        guards: state.guards.into_iter().collect(),
    })
}

fn prune(body: &mut mir::Body) {
    let edges = block_edges(body);
    let mut reachable = vec![false; edges.len()];
    let mut pending = vec![index(body.entry)];
    while let Some(block) = pending.pop() {
        if std::mem::replace(&mut reachable[block], true) {
            continue;
        }
        pending.extend(&edges[block]);
    }
    for (id, block) in body.blocks.iter_mut() {
        if !reachable[index(id)] {
            block.statements.clear();
            block.terminator = mir::Terminator::Unreachable;
            block.unwind = None;
        }
    }
    body.loop_header_polls
        .retain(|poll| reachable[index(poll.header())]);
}
