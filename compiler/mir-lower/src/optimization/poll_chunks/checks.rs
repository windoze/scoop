//! Fold checked branches while preserving acyclic data-dependent control flow.

use std::collections::{BTreeMap, BTreeSet};

use super::{Loop, header, linear, mir, operations, recognize, region, values::Length};
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
    incoming: &[Vec<mir::BlockId>],
) -> Option<Projection> {
    let mut state = facts::State::new(head);
    for &id in &head.chain {
        for statement in &function.body.blocks[id].statements {
            state.statement(statement);
        }
    }
    let region = region::iteration(&function.body, target, head, incoming)?;
    let members = region.blocks.iter().copied().collect::<BTreeSet<_>>();
    let mut states = BTreeMap::from([(head.inner, state)]);
    let mut branches = Vec::new();
    let mut guards = BTreeSet::new();
    let mut array_access = false;
    for current in region.blocks {
        let Some(mut state) = states.remove(&current) else {
            continue;
        };
        let block = &function.body.blocks[current];
        if !operations::supported(module, block, true) {
            return None;
        }
        for statement in &block.statements {
            array_access |= matches!(statement.kind, mir::StatementKind::ArraySet { .. });
            mir::visit_statement_exprs(statement, &mut |expr| {
                array_access |= matches!(expr.kind, mir::ExprKind::ArrayGet { .. });
            });
            state.statement(statement);
        }
        let next = match &block.terminator {
            mir::Terminator::Goto(next) => vec![*next],
            mir::Terminator::Branch {
                cond,
                then_block,
                else_block,
            } => {
                if let facts::Value::Boolean(value) = state.expression(cond) {
                    let next = if value { *then_block } else { *else_block };
                    branches.push((current, next));
                    vec![next]
                } else {
                    region::successors(&block.terminator)?
                }
            }
            _ => return None,
        };
        guards.extend(&state.guards);
        for next in next {
            if members.contains(&next) {
                facts::propagate(&mut states, next, &state);
            }
        }
    }
    if !array_access
        || branches.is_empty()
        || (!guards.is_empty() && head.kind != mir::IntegerKind::SIGNED_64)
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
        guards: guards.into_iter().collect(),
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
