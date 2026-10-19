use scoop_identity::{CallableMaterialization, StructuralDefinitionSiteRole, SyntheticLocalRole};

use super::{
    index, mir,
    origins::{Site, statement_alias},
    uses::{Plan, Replacement},
};
use crate::local_values::LocalValueRegistry;

pub(super) fn apply(
    module: &mir::Module,
    id: mir::FunctionId,
    owner: CallableMaterialization,
    registry: &mut LocalValueRegistry,
    plan: &Plan,
) -> (mir::Function, bool) {
    let mut function = module.functions[id].clone();
    let mut snapshots = vec![None; plan.removable.len()];
    for (candidate, creation) in plan.sources.allocations.iter() {
        if !plan.removable[index(candidate)] {
            continue;
        }
        let local = function.body.locals.alloc(mir::Local {
            name: format!("$box.payload.{}", index(creation.local)),
            ty: creation.payload.ty.clone(),
            mutable: false,
        });
        registry.record_generated_local(
            id,
            local,
            owner,
            StructuralDefinitionSiteRole::SyntheticValue,
            SyntheticLocalRole::Temporary,
        );
        snapshots[index(candidate)] = Some(local);
    }
    let mut changed = false;
    for (block_id, block) in function.body.blocks.iter_mut() {
        // Positions refer to the immutable analyzed body, even after removals.
        let old = std::mem::take(&mut block.statements);
        let terminator_site = Site {
            block: block_id,
            statement: old.len(),
        };
        for (statement_index, mut statement) in old.into_iter().enumerate() {
            let site = Site {
                block: block_id,
                statement: statement_index,
            };
            if let Some(local) = statement_alias(&statement)
                && let Some(candidate) = plan.sources.locals[index(local)].unique()
                && plan.removable[index(candidate)]
            {
                changed = true;
                continue;
            }
            if let mir::StatementKind::ValDecl { local, .. } = &statement.kind
                && let Some(candidate) = plan.sources.locals[index(*local)].unique()
                && let creation = &plan.sources.allocations[candidate]
                && creation.site == site
                && let Some(snapshot) = snapshots[index(candidate)]
            {
                statement.kind = mir::StatementKind::ValDecl {
                    local: snapshot,
                    init: creation.payload.clone(),
                };
                changed = true;
            }
            mir::visit_statement_exprs_mut(&mut statement, &mut |expr| {
                changed |= expression(module, plan, &snapshots, site, expr);
            });
            block.statements.push(statement);
        }
        mir::visit_terminator_exprs_mut(&mut block.terminator, &mut |expr| {
            changed |= expression(module, plan, &snapshots, terminator_site, expr);
        });
    }
    (function, changed)
}

fn expression(
    module: &mir::Module,
    plan: &Plan,
    snapshots: &[Option<mir::LocalId>],
    site: Site,
    expr: &mut mir::Expr,
) -> bool {
    let Some((_, replacement)) = plan.replacement(module, expr, site) else {
        return false;
    };
    match replacement {
        Replacement::Payload(candidate) => {
            let Some(snapshot) = snapshots[index(candidate)] else {
                return false;
            };
            *expr = mir::Expr::local(snapshot, expr.ty.clone());
        }
        Replacement::Boolean(value) => {
            *expr = mir::Expr::new(mir::Type::Boolean, mir::ExprKind::BoolLiteral(value))
        }
    }
    true
}
