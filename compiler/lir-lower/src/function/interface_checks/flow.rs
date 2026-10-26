//! Forward intersection of successful guards, with writes and unwind edges killed.

use std::collections::{VecDeque, hash_map::Entry};

use super::*;

pub(super) fn incoming(
    body: &mir::Body,
    checks: &HashMap<mir::BlockId, Check>,
) -> HashMap<mir::BlockId, Facts> {
    let mut inputs = HashMap::from([(body.entry, Facts::new())]);
    let mut pending = VecDeque::from([body.entry]);
    let mut queued = HashSet::from([body.entry]);
    while let Some(id) = pending.pop_front() {
        queued.remove(&id);
        let block = &body.blocks[id];
        let mut state = inputs[&id].clone();
        for statement in &block.statements {
            written(statement, &mut state);
        }
        let normal = match block.terminator {
            mir::Terminator::Goto(target) => vec![target],
            mir::Terminator::Branch {
                then_block,
                else_block,
                ..
            } => vec![then_block, else_block],
            _ => Vec::new(),
        };
        for target in normal {
            let mut next = state.clone();
            if let Some(check) = checks.get(&id) {
                next.remove(&check.source);
                if target == check.success {
                    next.insert(check.source, id);
                }
            }
            if merge(target, body.entry, next, &mut inputs) && queued.insert(target) {
                pending.push_back(target);
            }
        }
        let thrown = match block.terminator {
            mir::Terminator::Throw { unwind, .. } | mir::Terminator::Rethrow { unwind } => unwind,
            _ => None,
        };
        for target in [block.unwind, thrown].into_iter().flatten() {
            if merge(target, body.entry, Facts::new(), &mut inputs) && queued.insert(target) {
                pending.push_back(target);
            }
        }
    }
    inputs
}

fn merge(
    target: mir::BlockId,
    entry: mir::BlockId,
    next: Facts,
    inputs: &mut HashMap<mir::BlockId, Facts>,
) -> bool {
    // A re-entry cannot manufacture a fact before the first execution.
    if target == entry {
        return false;
    }
    match inputs.entry(target) {
        Entry::Vacant(entry) => {
            entry.insert(next);
            true
        }
        Entry::Occupied(mut entry) => {
            let old = entry.get_mut();
            let count = old.len();
            old.retain(|key, value| next.get(key) == Some(value));
            old.len() != count
        }
    }
}

pub(super) fn written(statement: &mir::Statement, facts: &mut Facts) {
    if let Some(local) = interface_tables::written_local(statement) {
        facts.retain(|(source, _), _| *source != local);
    }
}
