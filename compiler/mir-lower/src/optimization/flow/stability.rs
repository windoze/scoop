//! Storage that is assigned once outside cycles can name equivalent copies.

use std::collections::HashSet;

use super::mir;
use crate::optimization::graph::{self, index};

pub(super) fn locals(function: &mir::Function, addressed: &HashSet<mir::LocalId>) -> Vec<bool> {
    let body = &function.body;
    let cyclic = graph::cyclic_nodes(&graph::block_edges(body));
    let mut writes = vec![0_u32; body.locals.len()];
    let mut stable = vec![true; body.locals.len()];
    for parameter in &function.params {
        writes[index(parameter.local)] += 1;
    }
    for (id, block) in body.blocks.iter() {
        for statement in &block.statements {
            let local = match statement.kind {
                mir::StatementKind::ValDecl { local, .. }
                | mir::StatementKind::Assign { local, .. }
                | mir::StatementKind::Call(mir::CallEffect::Value {
                    destination: local, ..
                }) => local,
                _ => continue,
            };
            writes[index(local)] = writes[index(local)].saturating_add(1);
            stable[index(local)] &= !cyclic[index(id)];
        }
    }
    for (local, _) in body.locals.iter() {
        stable[index(local)] &= writes[index(local)] == 1 && !addressed.contains(&local);
    }
    stable
}
