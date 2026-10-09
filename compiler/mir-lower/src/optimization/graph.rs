//! Small, iterative SCC traversal shared by call-cycle and loop decisions.

use super::mir;

pub(super) fn cyclic_nodes(edges: &[Vec<usize>]) -> Vec<bool> {
    let mut reverse = vec![Vec::new(); edges.len()];
    for (from, targets) in edges.iter().enumerate() {
        for &to in targets {
            reverse[to].push(from);
        }
    }
    let mut seen = vec![false; edges.len()];
    let mut order = Vec::new();
    for root in 0..edges.len() {
        if seen[root] {
            continue;
        }
        seen[root] = true;
        let mut stack = vec![(root, 0)];
        while let Some((node, next)) = stack.last_mut() {
            if let Some(&target) = edges[*node].get(*next) {
                *next += 1;
                if !seen[target] {
                    seen[target] = true;
                    stack.push((target, 0));
                }
            } else {
                order.push(*node);
                stack.pop();
            }
        }
    }
    seen.fill(false);
    let mut cyclic = vec![false; edges.len()];
    for root in order.into_iter().rev() {
        if seen[root] {
            continue;
        }
        let mut component = Vec::new();
        let mut pending = vec![root];
        seen[root] = true;
        while let Some(node) = pending.pop() {
            component.push(node);
            for &target in &reverse[node] {
                if !seen[target] {
                    seen[target] = true;
                    pending.push(target);
                }
            }
        }
        if component.len() > 1 || edges[root].contains(&root) {
            for node in component {
                cyclic[node] = true;
            }
        }
    }
    cyclic
}

pub(super) fn block_edges(body: &mir::Body) -> Vec<Vec<usize>> {
    body.blocks
        .iter()
        .map(|(_, block)| {
            let mut targets = Vec::new();
            if let Some(unwind) = block.unwind {
                targets.push(index(unwind));
            }
            match &block.terminator {
                mir::Terminator::Goto(target) => targets.push(index(*target)),
                mir::Terminator::Branch {
                    then_block,
                    else_block,
                    ..
                } => {
                    targets.push(index(*then_block));
                    targets.push(index(*else_block));
                }
                mir::Terminator::Throw {
                    unwind: Some(target),
                    ..
                }
                | mir::Terminator::Rethrow {
                    unwind: Some(target),
                } => targets.push(index(*target)),
                _ => {}
            }
            targets
        })
        .collect()
}

pub(super) fn index<T>(id: la_arena::Idx<T>) -> usize {
    id.into_raw().into_u32() as usize
}
