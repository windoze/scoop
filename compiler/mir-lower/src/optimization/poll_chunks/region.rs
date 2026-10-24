//! Acyclic iteration bodies with a single entry and latch.

use std::collections::{BTreeMap, BTreeSet};

use super::{header::Header, linear, mir};
use crate::optimization::graph::index;

pub(super) struct Region {
    pub entry: mir::BlockId,
    pub latch: mir::BlockId,
    pub blocks: Vec<mir::BlockId>,
}

/// Keep only paths that can complete another iteration. Exit code stays shared
/// with the original body, including calls, allocation and exception handling.
pub(super) fn iteration(
    body: &mir::Body,
    header: mir::BlockId,
    head: &Header,
    incoming: &[Vec<mir::BlockId>],
) -> Option<Region> {
    let latch = *incoming[index(header)]
        .iter()
        .find(|&&block| block != head.preheader)?;
    if !matches!(body.blocks[latch].terminator, mir::Terminator::Goto(next) if next == header)
        || incoming[index(head.inner)] != [*head.chain.last()?]
    {
        return None;
    }
    let mut members = BTreeSet::new();
    let mut pending = vec![latch];
    while let Some(id) = pending.pop() {
        if head.chain.contains(&id) || !members.insert(id) {
            continue;
        }
        let block = &body.blocks[id];
        if id == body.entry
            || id == head.exit
            || !head.invariants.dominates(header, id)
            || block.unwind.is_some()
            || body
                .loop_header_polls
                .iter()
                .any(|poll| poll.header() == id)
        {
            return None;
        }
        pending.extend(&incoming[index(id)]);
    }
    if !members.contains(&head.inner) {
        return None;
    }
    let mut stops = BTreeSet::from([header]);
    for &id in &members {
        if incoming[index(id)].iter().any(|predecessor| {
            !members.contains(predecessor)
                && !(id == head.inner && head.chain.last() == Some(predecessor))
        }) {
            return None;
        }
        for next in successors(&body.blocks[id].terminator)? {
            if head.chain.contains(&next) && !(id == latch && next == header) {
                return None;
            }
            if !members.contains(&next) {
                stops.insert(next);
            }
        }
    }
    let blocks = order(body, head.inner, &stops.into_iter().collect::<Vec<_>>())?;
    if blocks.len() != members.len() {
        return None;
    }
    Some(Region {
        entry: head.inner,
        latch,
        blocks,
    })
}

impl Region {
    pub fn merge(&mut self, body: &mut mir::Body) {
        let incoming = linear::predecessors(body);
        let mut owners = self
            .blocks
            .iter()
            .map(|&block| (block, block))
            .collect::<BTreeMap<_, _>>();
        for &block in &self.blocks {
            if block == self.entry || incoming[index(block)].len() != 1 {
                continue;
            }
            let Some(&previous) = owners.get(&incoming[index(block)][0]) else {
                continue;
            };
            if matches!(body.blocks[previous].terminator, mir::Terminator::Goto(next) if next == block)
            {
                linear::merge(body, &[previous, block]);
                owners.insert(block, previous);
                if self.latch == block {
                    self.latch = previous;
                }
            }
        }
        self.blocks.retain(|block| owners[block] == *block);
    }
}

pub(super) fn successors(terminator: &mir::Terminator) -> Option<Vec<mir::BlockId>> {
    match terminator {
        mir::Terminator::Goto(next) => Some(vec![*next]),
        mir::Terminator::Branch {
            then_block,
            else_block,
            ..
        } => Some(if then_block == else_block {
            vec![*then_block]
        } else {
            vec![*then_block, *else_block]
        }),
        _ => None,
    }
}

/// Topological order before folding checked branches. Terminal cold paths may
/// be present here; callers validate only paths that actually remain reachable.
pub(super) fn order(
    body: &mir::Body,
    start: mir::BlockId,
    stops: &[mir::BlockId],
) -> Option<Vec<mir::BlockId>> {
    let mut colors = vec![0_u8; body.blocks.len()];
    let mut pending = vec![(start, false)];
    let mut order = Vec::new();
    while let Some((block, finish)) = pending.pop() {
        if stops.contains(&block) {
            continue;
        }
        if finish {
            colors[index(block)] = 2;
            order.push(block);
            continue;
        }
        match colors[index(block)] {
            1 => return None,
            2 => continue,
            _ => {}
        }
        colors[index(block)] = 1;
        pending.push((block, true));
        if let Some(next) = successors(&body.blocks[block].terminator) {
            pending.extend(next.into_iter().map(|next| (next, false)));
        }
    }
    order.reverse();
    Some(order)
}
