//! Remove unused pure view definitions, including newly dead copy chains.

use std::collections::{HashMap, HashSet, VecDeque};

use super::{index, mir, pure_local};

pub(super) fn unused(body: &mut mir::Body, addressed: &HashSet<mir::LocalId>) -> bool {
    let mut reads = vec![0_usize; body.locals.len()];
    let mut definitions: HashMap<mir::LocalId, Vec<_>> = HashMap::new();
    for (block_id, block) in body.blocks.iter() {
        mir::visit_block_exprs(block, &mut |expression| {
            if let mir::ExprKind::Local(local) | mir::ExprKind::AddressOf { local, .. } =
                expression.kind
            {
                reads[index(local)] += 1;
            }
        });
        for (position, statement) in block.statements.iter().enumerate() {
            if let mir::StatementKind::ValDecl { local, init } = &statement.kind
                && !addressed.contains(local)
                && let Some(source) = pure_local(init)
                && !addressed.contains(&source)
            {
                definitions
                    .entry(*local)
                    .or_default()
                    .push((block_id, position, source));
            }
        }
    }
    let mut pending = definitions
        .keys()
        .copied()
        .filter(|local| reads[index(*local)] == 0)
        .collect::<VecDeque<_>>();
    let mut removed = HashSet::new();
    while let Some(local) = pending.pop_front() {
        let Some(definitions) = definitions.remove(&local) else {
            continue;
        };
        for (block, position, source) in definitions {
            removed.insert((block, position));
            reads[index(source)] -= 1;
            if reads[index(source)] == 0 {
                pending.push_back(source);
            }
        }
    }
    if removed.is_empty() {
        return false;
    }
    for (block, body) in body.blocks.iter_mut() {
        let mut position = 0;
        body.statements.retain(|_| {
            let keep = !removed.contains(&(block, position));
            position += 1;
            keep
        });
    }
    true
}
