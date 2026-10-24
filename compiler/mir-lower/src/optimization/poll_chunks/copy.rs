use std::collections::{BTreeMap, BTreeSet};

use scoop_identity::{CallableMaterialization, StructuralDefinitionSiteRole, SyntheticLocalRole};

use super::{LocalValueRegistry, mir};

/// Duplicate the selected body, keeping loop-carried source locals.
pub(super) fn body(
    function: &mut mir::Function,
    source: mir::BlockId,
    temporaries: &BTreeSet<mir::LocalId>,
    registry: &mut LocalValueRegistry,
    owner: CallableMaterialization,
    id: mir::FunctionId,
) -> mir::BlockId {
    let mut block = function.body.blocks[source].clone();
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
    mir::visit_block_exprs_mut(&mut block, &mut |expression| {
        if let mir::ExprKind::Local(local) = &mut expression.kind {
            *local = locals.get(local).copied().unwrap_or(*local);
        }
    });
    for statement in &mut block.statements {
        if let mir::StatementKind::ValDecl { local, .. }
        | mir::StatementKind::Assign { local, .. } = &mut statement.kind
        {
            *local = locals.get(local).copied().unwrap_or(*local);
        }
    }
    block.name = format!("poll.full.{}", function.body.blocks.len());
    function.body.blocks.alloc(block)
}
