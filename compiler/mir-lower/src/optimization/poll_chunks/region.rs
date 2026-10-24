//! Acyclic iteration bodies with a single entry and latch.

use std::collections::BTreeMap;

use super::{linear, mir};
use crate::optimization::graph::index;

pub(super) struct Region {
    pub entry: mir::BlockId,
    pub latch: mir::BlockId,
    pub blocks: Vec<mir::BlockId>,
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
