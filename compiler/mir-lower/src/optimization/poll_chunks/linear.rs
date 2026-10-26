//! Straight-line regions produced by ordinary inlining.

use std::collections::BTreeSet;

use super::mir;
use crate::optimization::graph::{block_edges, index};

pub(super) fn predecessors(body: &mir::Body) -> Vec<Vec<mir::BlockId>> {
    let mut incoming = vec![Vec::new(); body.blocks.len()];
    for (from, edges) in body.blocks.iter().map(|(id, _)| id).zip(block_edges(body)) {
        for target in edges {
            if !incoming[target].contains(&from) {
                incoming[target].push(from);
            }
        }
    }
    incoming
}

pub(super) fn chain(
    body: &mir::Body,
    start: mir::BlockId,
    stop: mir::BlockId,
    incoming: &[Vec<mir::BlockId>],
) -> Option<Vec<mir::BlockId>> {
    let mut chain = Vec::new();
    let mut seen = BTreeSet::new();
    let mut current = start;
    loop {
        if !seen.insert(current) || body.blocks[current].unwind.is_some() {
            return None;
        }
        chain.push(current);
        let mir::Terminator::Goto(next) = body.blocks[current].terminator else {
            return Some(chain);
        };
        if next == stop {
            return Some(chain);
        }
        if next == body.entry
            || incoming[index(next)] != [current]
            || body.loop_header_polls.iter().any(|p| p.header() == next)
        {
            return None;
        }
        current = next;
    }
}

pub(super) fn merge(body: &mut mir::Body, chain: &[mir::BlockId]) {
    let head = chain[0];
    for &next in &chain[1..] {
        let statements = std::mem::take(&mut body.blocks[next].statements);
        let terminator = std::mem::replace(
            &mut body.blocks[next].terminator,
            mir::Terminator::Unreachable,
        );
        body.blocks[head].statements.extend(statements);
        body.blocks[head].terminator = terminator;
    }
}
