use scoop_identity::CallableMaterialization;

use super::{
    index, mir,
    origins::{Site, defined_local, statement_alias},
    storage::{self, Initialization, ReplacementLocal},
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
    let values = storage::create(module, &mut function, id, owner, registry, plan);
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
            if let Some(local) = defined_local(&statement)
                && let Some(candidate) = plan.sources.locals[index(local)].unique()
                && let creation = &plan.sources.allocations[candidate]
                && creation.site == site
                && plan.removable[index(candidate)]
            {
                for storage in &values[index(candidate)] {
                    let Initialization::AllocationValue(value) = &storage.initialization else {
                        continue;
                    };
                    let mut init = value.clone();
                    mir::visit_expr_mut(&mut init, &mut |expr| {
                        expression(module, plan, &values, site, expr);
                    });
                    block.statements.push(mir::Statement {
                        debug_origin: statement.debug_origin.clone(),
                        kind: mir::StatementKind::ValDecl {
                            local: storage.local,
                            init,
                        },
                        span: statement.span,
                    });
                }
                changed = true;
                continue;
            }
            if let Some((candidate, projection)) = plan.field_write(module, &statement, site)
                && plan.removable[index(candidate)]
            {
                let local = values[index(candidate)]
                    .iter()
                    .find(|value| value.projection == projection)
                    .expect("a replaced field has typed storage")
                    .local;
                let mir::StatementKind::FieldSet { value, .. } = statement.kind else {
                    unreachable!()
                };
                statement.kind = mir::StatementKind::Assign { local, value };
                changed = true;
            }
            mir::visit_statement_exprs_mut(&mut statement, &mut |expr| {
                changed |= expression(module, plan, &values, site, expr);
            });
            block.statements.push(statement);
        }
        mir::visit_terminator_exprs_mut(&mut block.terminator, &mut |expr| {
            changed |= expression(module, plan, &values, terminator_site, expr);
        });
    }
    (function, changed)
}

fn expression(
    module: &mir::Module,
    plan: &Plan,
    values: &[Vec<ReplacementLocal>],
    site: Site,
    expr: &mut mir::Expr,
) -> bool {
    let Some((_, replacement)) = plan.replacement(module, expr, site) else {
        return false;
    };
    match replacement {
        Replacement::Value {
            candidate,
            projection,
        } => {
            let Some(storage) = values[index(candidate)]
                .iter()
                .find(|value| value.projection == projection)
            else {
                return false;
            };
            *expr = mir::Expr::local(storage.local, expr.ty.clone())
                .with_debug_origin(expr.debug_origin.clone());
        }
        Replacement::Boolean(value) => {
            *expr = mir::Expr::new(mir::Type::Boolean, mir::ExprKind::BoolLiteral(value))
                .with_debug_origin(expr.debug_origin.clone())
        }
    }
    true
}
