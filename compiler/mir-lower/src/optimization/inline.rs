use la_arena::Arena;
use scoop_identity::{
    CallableMaterialization, CallableMaterializationContext, CallableTemplateOwner,
};

use super::{
    flow,
    graph::{self, index},
    mir, simplify,
};
use crate::local_values::LocalValueRegistry;

mod clone;
mod policy;

struct Choice {
    block: mir::BlockId,
    statement: usize,
    callee: mir::FunctionId,
    blocks: Vec<(mir::BlockId, mir::BasicBlock)>,
}

pub(super) fn run(module: &mut mir::Module) {
    // All callers use the same original bodies. Earlier callers cannot
    // accidentally give later callers an already expanded nested chain.
    let templates = module.functions.clone();
    let recursive = policy::recursive(module);
    let protocol = policy::protocol_entries(module);
    let eligible = templates
        .iter()
        .map(|(id, function)| {
            !recursive[index(id)] && !protocol[index(id)] && policy::supported(function)
        })
        .collect::<Vec<_>>();
    let costs = templates
        .iter()
        .map(|(_, function)| policy::function_cost(function))
        .collect::<Vec<_>>();
    let mut registry = LocalValueRegistry::from_existing(&module.meta.local_values);
    for (id, original) in templates.iter() {
        if protocol[index(id)] {
            continue;
        }
        let Some(owner) = owner(module, id) else {
            continue;
        };
        let context = Context {
            module,
            templates: &templates,
            eligible: &eligible,
            costs: &costs,
        };
        let body = context.optimize(id, original, owner, &mut registry);
        module.functions[id].body = body;
    }
    module.meta.local_values = registry.finish();
}

struct Context<'a> {
    module: &'a mir::Module,
    templates: &'a Arena<mir::Function>,
    eligible: &'a [bool],
    costs: &'a [usize],
}

impl Context<'_> {
    fn optimize(
        &self,
        id: mir::FunctionId,
        original: &mir::Function,
        owner: CallableMaterialization,
        registry: &mut LocalValueRegistry,
    ) -> mir::Body {
        let mut function = original.clone();
        let mut depth = vec![0_u8; function.body.blocks.len()];
        let mut remaining = (self.costs[index(id)] / 2 + 96).min(384);
        loop {
            let choices = self.choose(&function, &depth, &mut remaining);
            if choices.is_empty() {
                break;
            }
            // Reverse application preserves every selected source position.
            // Selection and growth accounting above remain in stable order.
            for choice in choices.into_iter().rev() {
                let callee = &self.templates[choice.callee];
                clone::apply(
                    &mut function,
                    callee,
                    choice,
                    registry,
                    owner,
                    id,
                    &mut depth,
                );
            }
        }
        simplify::body(self.module, &function)
    }

    fn choose(&self, function: &mir::Function, depth: &[u8], remaining: &mut usize) -> Vec<Choice> {
        let analysis = flow::Analysis::new(self.module, function);
        let incoming = analysis.solve();
        let in_loop = graph::cyclic_nodes(&graph::block_edges(&function.body));
        let mut choices = Vec::new();
        for (block_id, block) in function.body.blocks.iter() {
            if depth[index(block_id)] >= 6 {
                continue;
            }
            let Some(mut state) = incoming[index(block_id)].clone() else {
                continue;
            };
            for (statement_index, statement) in block.statements.iter().enumerate() {
                if *remaining == 0 {
                    return choices;
                }
                if let mir::StatementKind::Call(effect) = &statement.kind {
                    let call = super::call(effect);
                    if let Some(callee) = policy::local_target(self.module, call)
                        && self.eligible[index(callee)]
                    {
                        let arguments = call
                            .args
                            .iter()
                            .map(|argument| analysis.expression(argument, &state))
                            .collect::<Vec<_>>();
                        let source = &self.templates[callee];
                        let blocks = simplify::blocks(self.module, source, &arguments);
                        let cost = policy::blocks_cost(blocks.iter().map(|(_, block)| block))
                            + policy::parameter_cost(source);
                        let growth = cost.saturating_sub(6).max(1);
                        if growth <= *remaining
                            && policy::worthwhile(
                                cost,
                                self.costs[index(callee)],
                                in_loop[index(block_id)],
                                &arguments,
                                source,
                            )
                        {
                            *remaining -= growth;
                            choices.push(Choice {
                                block: block_id,
                                statement: statement_index,
                                callee,
                                blocks,
                            });
                        }
                    }
                }
                analysis.statement(statement, &mut state);
            }
        }
        choices
    }
}

fn owner(module: &mir::Module, function: mir::FunctionId) -> Option<CallableMaterialization> {
    if let Some(source) = module.meta.source_callable_materializations.get(function) {
        return Some(source.materialization());
    }
    let generated = module.meta.generated_callables.get(function)?;
    Some(CallableMaterialization::new(
        CallableTemplateOwner::Generated(generated.identity_record().id()),
        CallableMaterializationContext::NoSubstitution,
    ))
}
