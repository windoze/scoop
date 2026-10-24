//! Report the existing allocation plan without repeating its analysis.

use std::collections::HashMap;

use super::{index, inline, mir, uses::Plan};
use crate::optimization::observation::Observer;
use mir::optimization_report::{
    AllocationKind, AllocationOutcome, OptimizationDecision, OptimizationSite,
};

pub(super) fn record(
    module: &mir::Module,
    function_id: mir::FunctionId,
    plan: &Plan,
    observer: &Observer<'_>,
    inliner: Option<&inline::Session>,
) {
    let outcomes = plan
        .sources
        .allocations
        .iter()
        .map(|(id, allocation)| {
            let outcome = if plan.removable[index(id)] {
                AllocationOutcome::Eliminated
            } else {
                plan.reasons.as_ref().unwrap()[index(id)]
                    .expect("retained candidates have a reason")
            };
            ((allocation.site.block, allocation.site.statement), outcome)
        })
        .collect::<HashMap<_, _>>();
    for (block_id, block) in module.functions[function_id].body.blocks.iter() {
        if !plan.reachable(block_id) {
            continue;
        }
        for (statement, node) in block.statements.iter().enumerate() {
            let expr = match &node.kind {
                mir::StatementKind::ValDecl { init, .. } => init,
                mir::StatementKind::Assign { value, .. }
                    if matches!(value.kind, mir::ExprKind::ClassAlloc { .. }) =>
                {
                    value
                }
                _ => continue,
            };
            let (kind, ty) = match &expr.kind {
                mir::ExprKind::Box(payload) => (AllocationKind::Box, &payload.ty),
                mir::ExprKind::ClosureAlloc { .. } => (AllocationKind::Closure, &expr.ty),
                mir::ExprKind::ClassAlloc { .. } => (AllocationKind::Class, &expr.ty),
                _ => continue,
            };
            let outcome = if let Some(outcome) = outcomes.get(&(block_id, statement)) {
                *outcome
            } else if let mir::ExprKind::ClassAlloc { class_id } = expr.kind {
                if module.classes[class_id].release_policy != mir::ReleasePolicy::None {
                    AllocationOutcome::ReleaseHook
                } else {
                    AllocationOutcome::Representation
                }
            } else {
                continue;
            };
            observer.record(
                OptimizationSite {
                    function: function_id,
                    block: block_id,
                    statement,
                    span: node.span,
                    inline_path: inliner.map_or_else(
                        || vec![mir::Callee::User(function_id)],
                        |inliner| inliner.ancestry(function_id, block_id),
                    ),
                },
                OptimizationDecision::Allocation {
                    kind,
                    ty: ty.clone(),
                    outcome,
                },
            );
        }
    }
}
