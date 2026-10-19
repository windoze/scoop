//! Inline small synchronous suspend bodies before adding the completion ABI.

use std::collections::HashMap;

use super::{CallerState, Choice, clone, graph, index, mir, policy};
use crate::Lowerer;
use crate::cfg::{CallSite, CallSiteKind};

mod sites;

struct Template {
    function: mir::Function,
    calls: Vec<CallSite>,
    cost: usize,
}

pub(in crate::optimization) fn run(lowerer: &mut Lowerer) {
    let templates = lowerer
        .suspend_sources
        .iter()
        .map(|source| {
            let function = lowerer.functions[source.function].clone();
            let calls = lowerer.coroutines.pre_coroutine_call_sites[&source.function].clone();
            let cost = policy::function_cost(&function);
            (
                source.function,
                Template {
                    function,
                    calls,
                    cost,
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
            remaining: (source.cost / 2 + 96).min(384),
            ancestry: vec![vec![id]; function.body.blocks.len()],
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
            let choice = Choice {
                block: site.block,
                statement: site.statement,
                callee,
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
    if state.remaining == 0 {
        return None;
    }
    let in_loop = graph::cyclic_nodes(&graph::block_edges(&function.body));
    for (order, site) in calls.iter().enumerate() {
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
        if !synchronous[index(callee)] || state.ancestry[index(site.block)].contains(&callee) {
            continue;
        }
        let source = &templates[&callee];
        let growth = source.cost.saturating_sub(6).max(1);
        if growth <= state.remaining
            && policy::worthwhile(
                source.cost,
                source.cost,
                in_loop[index(site.block)],
                &[],
                &source.function,
            )
        {
            state.remaining -= growth;
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
