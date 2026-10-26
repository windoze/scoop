//! Select synchronous suspend call sites before adding the completion ABI.

use std::collections::HashMap;

use super::{CallerState, Choice, clone, graph, index, mir, policy, summary};
use crate::Lowerer;
use crate::cfg::{CallSite, CallSiteKind};

mod sites;

struct Template {
    function: mir::Function,
    calls: Vec<CallSite>,
    summary: summary::Summary,
}

pub(in crate::optimization) fn run(lowerer: &mut Lowerer) {
    let templates = lowerer
        .suspend_sources
        .iter()
        .map(|source| {
            let function = lowerer.functions[source.function].clone();
            let calls = lowerer.coroutines.pre_coroutine_call_sites[&source.function].clone();
            let summary = summary::Summary::from_blocks(
                &lowerer.structs.defs,
                &function,
                function
                    .body
                    .blocks
                    .iter()
                    .map(|(id, block)| (id, block.clone()))
                    .collect(),
            );
            (
                source.function,
                Template {
                    function,
                    calls,
                    summary,
                },
            )
        })
        .collect::<HashMap<_, _>>();
    let synchronous = synchronous_sources(lowerer, &templates);
    let callers = lowerer
        .suspend_sources
        .iter()
        .map(|source| (source.function, source.materialization))
        .collect::<Vec<_>>();
    for (id, owner) in callers {
        let source = &templates[&id];
        let mut function = source.function.clone();
        let mut calls = source.calls.clone();
        let mut state = CallerState {
            growth: policy::Growth::new(source.summary.original.size, 96),
            ancestry: vec![vec![super::Target::Local(id)]; function.body.blocks.len()],
        };
        while let Some((order, callee)) = choose(
            lowerer,
            &templates,
            &synchronous,
            &function,
            &calls,
            &mut state,
        ) {
            let site = calls[order].clone();
            let template = &templates[&callee];
            let mir::StatementKind::Call(effect) =
                &function.body.blocks[site.block].statements[site.statement].kind
            else {
                unreachable!("a selected call-site record identifies its CFG call");
            };
            let choice = Choice {
                block: site.block,
                statement: site.statement,
                callee: super::Target::Local(callee),
                call: super::super::call(effect).clone(),
                blocks: template
                    .function
                    .body
                    .blocks
                    .iter()
                    .map(|(id, block)| (id, block.clone()))
                    .collect(),
            };
            let expansion = clone::apply(
                &mut function,
                &template.function,
                choice,
                &mut lowerer.local_values,
                owner,
                id,
                &mut state.ancestry,
            );
            sites::splice(&mut calls, order, &template.calls, &expansion);
        }
        lowerer.functions[id] = function;
        lowerer
            .coroutines
            .pre_coroutine_call_sites
            .insert(id, calls);
    }
}

fn synchronous_sources(
    lowerer: &Lowerer,
    templates: &HashMap<mir::FunctionId, Template>,
) -> Vec<bool> {
    let mut allowed = vec![false; lowerer.functions.len()];
    let mut dependencies = vec![Vec::new(); lowerer.functions.len()];
    for (&id, template) in templates {
        allowed[index(id)] = policy::supported(&template.function)
            && template
                .function
                .body
                .blocks
                .iter()
                .all(|(_, block)| block.unwind.is_none())
            && template.calls.iter().all(|site| site.pending.0.is_empty());
        for site in &template.calls {
            if matches!(site.kind, CallSiteKind::Suspend(_)) {
                allowed[index(id)] = false;
                continue;
            }
            let body = &template.function.body;
            let statement = &body.blocks[site.block].statements[site.statement];
            if let Some((call, _, _)) = crate::coroutine::suspend_effect(lowerer, body, statement) {
                if let Some(callee) =
                    direct_target(lowerer, &call).filter(|id| templates.contains_key(id))
                {
                    dependencies[index(id)].push(index(callee));
                } else {
                    allowed[index(id)] = false;
                }
            }
        }
    }
    let recursive = graph::cyclic_nodes(&dependencies);
    let mut known = vec![false; allowed.len()];
    loop {
        let mut changed = false;
        for id in 0..allowed.len() {
            if allowed[id]
                && !recursive[id]
                && !known[id]
                && dependencies[id].iter().all(|&target| known[target])
            {
                known[id] = true;
                changed = true;
            }
        }
        if !changed {
            return known;
        }
    }
}

fn choose(
    lowerer: &Lowerer,
    templates: &HashMap<mir::FunctionId, Template>,
    synchronous: &[bool],
    function: &mir::Function,
    calls: &[CallSite],
    state: &mut CallerState,
) -> Option<(usize, mir::FunctionId)> {
    let heats = policy::heats(&function.body);
    let caller_size = policy::function_cost(&lowerer.structs.defs, function).size;
    let mut reads = std::collections::HashSet::new();
    for (_, block) in function.body.blocks.iter() {
        mir::visit_block_exprs(block, &mut |expr| {
            if let mir::ExprKind::Local(local) | mir::ExprKind::AddressOf { local, .. } = expr.kind
            {
                reads.insert(local);
            }
        });
    }
    let mut order = calls.iter().enumerate().collect::<Vec<_>>();
    order.sort_by_key(|(_, site)| {
        let heat = heats[index(site.block)];
        std::cmp::Reverse(heat.numerator * 8 / heat.denominator)
    });
    for (order, site) in order {
        if !matches!(site.kind, CallSiteKind::Call) || state.ancestry[index(site.block)].len() > 6 {
            continue;
        }
        let mir::StatementKind::Call(effect) =
            &function.body.blocks[site.block].statements[site.statement].kind
        else {
            unreachable!("a call-site record identifies its CFG call");
        };
        let call = super::super::call(effect);
        let Some(callee) = direct_target(lowerer, call) else {
            continue;
        };
        if !synchronous[index(callee)]
            || state.ancestry[index(site.block)].contains(&super::Target::Local(callee))
        {
            continue;
        }
        let source = &templates[&callee];
        let context = policy::Site {
            heat: heats[index(site.block)],
            caller_size,
            result_used: match effect {
                mir::CallEffect::Value { destination, .. } => reads.contains(destination),
                mir::CallEffect::Unit(_) => false,
            },
        };
        let cost = policy::assess(
            &source.summary,
            source.summary.baseline,
            summary::complexity(&source.summary.blocks),
            0,
            0,
            &context,
        );
        if cost.growth <= state.growth.remaining(context.heat)
            && cost.weighted_benefit >= cost.penalty
        {
            state.growth.charge(cost.growth, context.heat);
            return Some((order, callee));
        }
    }
    None
}

fn direct_target(lowerer: &Lowerer, call: &mir::Call) -> Option<mir::FunctionId> {
    match call.target {
        mir::CallTarget::Direct(mir::Callee::User(id)) => Some(id),
        mir::CallTarget::Direct(mir::Callee::Monomorphized(id)) => {
            Some(lowerer.instances.meta[id].function)
        }
        _ => None,
    }
}
