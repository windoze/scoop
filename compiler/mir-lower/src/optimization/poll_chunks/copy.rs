use std::collections::{BTreeMap, BTreeSet};

use scoop_identity::{CallableMaterialization, StructuralDefinitionSiteRole, SyntheticLocalRole};

use super::{LocalValueRegistry, mir, region::Region};

/// Duplicate the selected body, keeping loop-carried source locals.
pub(super) fn body(
    function: &mut mir::Function,
    region: &Region,
    source: Vec<(mir::BlockId, mir::BasicBlock)>,
    temporaries: &BTreeSet<mir::LocalId>,
    registry: &mut LocalValueRegistry,
    owner: CallableMaterialization,
    id: mir::FunctionId,
) -> Region {
    let mut locals = BTreeMap::new();
    for &local in temporaries {
        let mut definition = function.body.locals[local].clone();
        definition.name = format!(
            "$poll.full.{}.{}",
            function.body.blocks.len(),
            definition.name
        );
        let copy = function.body.locals.alloc(definition);
        locals.insert(local, copy);
        registry.record_generated_local(
            id,
            copy,
            owner,
            StructuralDefinitionSiteRole::SyntheticValue,
            SyntheticLocalRole::Temporary,
        );
    }
    let mut blocks = BTreeMap::new();
    for (original, mut block) in source {
        block.name = if original == region.entry {
            format!("poll.full.{}", function.body.blocks.len())
        } else {
            format!("poll.full.body.{}", function.body.blocks.len())
        };
        blocks.insert(original, function.body.blocks.alloc(block));
    }
    for &id in blocks.values() {
        let block = &mut function.body.blocks[id];
        mir::visit_block_exprs_mut(block, &mut |expression| {
            if let mir::ExprKind::Local(local) = &mut expression.kind {
                *local = locals.get(local).copied().unwrap_or(*local);
            }
        });
        for statement in &mut block.statements {
            if let mir::StatementKind::ValDecl { local, .. }
            | mir::StatementKind::Assign { local, .. }
            | mir::StatementKind::Call(mir::CallEffect::Value {
                destination: local, ..
            }) = &mut statement.kind
            {
                *local = locals.get(local).copied().unwrap_or(*local);
            }
        }
        let remap = |target: &mut mir::BlockId| {
            *target = blocks.get(target).copied().unwrap_or(*target);
        };
        match &mut block.terminator {
            mir::Terminator::Goto(next) => remap(next),
            mir::Terminator::Branch {
                then_block,
                else_block,
                ..
            } => {
                remap(then_block);
                remap(else_block);
            }
            _ => unreachable!("selected acyclic region has only ordinary branches"),
        }
    }
    Region {
        entry: blocks[&region.entry],
        latch: blocks[&region.latch],
        blocks: region.blocks.iter().map(|block| blocks[block]).collect(),
    }
}
