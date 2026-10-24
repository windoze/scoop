use std::collections::BTreeSet;

use super::{
    CHUNK, Loop,
    checks::facts::{State, Value},
    header, mir, operations, region,
    values::{defined_local, definition, integer},
};
use crate::optimization::graph::index;

pub(super) fn counted(
    module: &mir::Module,
    function: &mir::Function,
    header: mir::BlockId,
    head: &header::Header,
    incoming: &[Vec<mir::BlockId>],
) -> Option<Loop> {
    let counted = shape(function, header, head, incoming)?;
    if counted
        .body
        .blocks
        .iter()
        .any(|id| !operations::supported(module, &function.body.blocks[*id], true))
        || integer(head.invariants.resolve(&counted.bound))
            .is_some_and(|end| end - counted.start <= i128::from(CHUNK))
    {
        return None;
    }
    Some(counted)
}

/// Establish induction independently of whether the body may contain calls.
pub(super) fn shape(
    function: &mir::Function,
    header: mir::BlockId,
    head: &header::Header,
    incoming: &[Vec<mir::BlockId>],
) -> Option<Loop> {
    let body = &function.body;
    let blocks = region::order(body, head.inner, &[header])?;
    let members = blocks.iter().copied().collect::<BTreeSet<_>>();
    let mut latch = None;
    for &id in &blocks {
        let block = &body.blocks[id];
        if id == body.entry
            || id == head.exit
            || head.chain.contains(&id)
            || block.unwind.is_some()
            || body
                .loop_header_polls
                .iter()
                .any(|poll| poll.header() == id)
            || incoming[index(id)].iter().any(|predecessor| {
                !members.contains(predecessor)
                    && !(id == head.inner && head.chain.last() == Some(predecessor))
            })
        {
            return None;
        }
        if matches!(block.terminator, mir::Terminator::Goto(next) if next == header) {
            if latch.replace(id).is_some() {
                return None;
            }
        } else if region::successors(&block.terminator)?
            .iter()
            .any(|next| !members.contains(next))
        {
            return None;
        }
    }
    let latch = latch?;
    if incoming[index(head.inner)] != [*head.chain.last()?] || latch == head.preheader {
        return None;
    }
    let mut temporaries = blocks
        .iter()
        .flat_map(|id| &body.blocks[*id].statements)
        .filter_map(defined_local)
        .collect::<BTreeSet<_>>();
    for param in &function.params {
        temporaries.remove(&param.local);
        if head.locals.contains(&param.local) {
            return None;
        }
    }
    let mut writes = Vec::new();
    for (id, candidate) in body.blocks.iter() {
        for (position, statement) in candidate.statements.iter().enumerate() {
            if let Some(local) = defined_local(statement) {
                if !members.contains(&id) {
                    temporaries.remove(&local);
                }
                if local == head.index {
                    writes.push((id, position));
                }
                if !head.chain.contains(&id) && head.locals.contains(&local) {
                    return None;
                }
            }
        }
    }
    let mut valid = true;
    for (id, candidate) in body.blocks.iter() {
        mir::visit_block_exprs(candidate, &mut |expr| {
            if matches!(expr.kind, mir::ExprKind::Local(local)
                if (!members.contains(&id) && temporaries.contains(&local))
                    || (!head.chain.contains(&id) && head.locals.contains(&local)))
                || matches!(expr.kind, mir::ExprKind::AddressOf { local, .. }
                    if local == head.index || head.locals.contains(&local) || temporaries.contains(&local))
            {
                valid = false;
            }
        });
    }
    if !valid || writes.len() != 2 {
        return None;
    }
    let initial = writes.iter().find(|(id, _)| *id == head.preheader)?;
    let (_, init) = definition(&body.blocks[initial.0].statements[initial.1])?;
    let start = integer(init)?;
    if start < 0 {
        return None;
    }
    let update = *writes.iter().find(|(id, _)| members.contains(id))?;
    if !head.invariants.dominates(update.0, latch) {
        return None;
    }
    let mut state = State::before(function, head, &blocks, update)?;
    let (_, value) = definition(&body.blocks[update.0].statements[update.1])?;
    if state.expression(value) != Value::NextIndex {
        return None;
    }
    Some(Loop {
        header,
        body: region::Region {
            entry: head.inner,
            latch,
            blocks,
        },
        index: head.index,
        bound: head.bound.clone(),
        kind: head.kind,
        start,
        header_chain: head.chain.clone(),
        temporaries,
    })
}
