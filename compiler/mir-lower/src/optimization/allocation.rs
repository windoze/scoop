//! Allocation origins and their value-replacement consumers, before GC roots.

use super::{
    graph::{self, index},
    inline, mir, simplify,
};
use crate::local_values::LocalValueRegistry;

mod control;
mod origins;
mod rewrite;
mod uses;

pub(super) fn run(
    module: &mut mir::Module,
    active: &[bool],
    registry: &mut LocalValueRegistry,
) -> Vec<bool> {
    let mut changed = vec![false; module.functions.len()];
    let functions = module
        .functions
        .iter()
        .map(|(id, _)| id)
        .collect::<Vec<_>>();
    for id in functions {
        if !active[index(id)] || !supported(&module.functions[id]) {
            continue;
        }
        let Some(owner) = inline::owner(module, id) else {
            continue;
        };
        let plan = uses::Plan::new(module, &module.functions[id]);
        let (function, rewritten) = rewrite::apply(module, id, owner, registry, &plan);
        if rewritten {
            module.functions[id].body = simplify::body(module, &function).0;
            changed[index(id)] = true;
        }
    }
    changed
}

fn supported(function: &mir::Function) -> bool {
    let mut has_candidate = false;
    for (_, block) in function.body.blocks.iter() {
        if block.unwind.is_some()
            || matches!(
                block.terminator,
                mir::Terminator::Resume
                    | mir::Terminator::Rethrow { .. }
                    | mir::Terminator::Throw {
                        unwind: Some(_),
                        ..
                    }
            )
        {
            return false;
        }
        for statement in &block.statements {
            match &statement.kind {
                mir::StatementKind::ValDecl {
                    init:
                        mir::Expr {
                            kind: mir::ExprKind::Box(_),
                            ..
                        },
                    ..
                } => has_candidate = true,
                mir::StatementKind::Eh(_) => return false,
                _ => {}
            }
        }
    }
    has_candidate
}
