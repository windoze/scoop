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

pub(super) struct Session {
    templates: Arena<mir::Function>,
    protocol: Vec<bool>,
    supported: Vec<bool>,
    costs: Vec<usize>,
    callers: Vec<CallerState>,
}

struct CallerState {
    remaining: usize,
    ancestry: Vec<Vec<mir::FunctionId>>,
}

impl Session {
    pub(super) fn new(module: &mir::Module) -> Self {
        // Stable templates avoid counting an already expanded body as one level.
        let templates = module.functions.clone();
        let costs = templates
            .iter()
            .map(|(_, function)| policy::function_cost(function))
            .collect::<Vec<_>>();
        let callers = templates
            .iter()
            .map(|(id, function)| CallerState {
                remaining: (costs[index(id)] / 2 + 96).min(384),
                ancestry: vec![vec![id]; function.body.blocks.len()],
            })
            .collect();
        Self {
            protocol: policy::protocol_entries(module),
            supported: templates
                .iter()
                .map(|(_, function)| policy::supported(function))
                .collect(),
            templates,
            costs,
            callers,
        }
    }

    pub(super) fn run(
        &mut self,
        module: &mut mir::Module,
        active: &[bool],
        registry: &mut LocalValueRegistry,
    ) -> Vec<bool> {
        // Devirtualization can reveal cycles that were absent in earlier rounds.
        let recursive = policy::recursive(module);
        let eligible = self
            .supported
            .iter()
            .enumerate()
            .map(|(id, supported)| *supported && !recursive[id] && !self.protocol[id])
            .collect::<Vec<_>>();
        let mut changed = vec![false; module.functions.len()];
        for (id, _) in self.templates.iter() {
            if !active[index(id)] || self.protocol[index(id)] {
                continue;
            }
            let Some(owner) = owner(module, id) else {
                continue;
            };
            let context = Context {
                module,
                templates: &self.templates,
                eligible: &eligible,
                costs: &self.costs,
            };
            let (body, rewritten) =
                context.optimize(id, owner, registry, &mut self.callers[index(id)]);
            module.functions[id].body = body;
            changed[index(id)] = rewritten;
        }
        changed
    }
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
        owner: CallableMaterialization,
        registry: &mut LocalValueRegistry,
        state: &mut CallerState,
    ) -> (mir::Body, bool) {
        let mut function = self.module.functions[id].clone();
        let mut changed = false;
        loop {
            let choices = self.choose(&function, &state.ancestry, &mut state.remaining);
            if choices.is_empty() {
                break;
            }
            changed = true;
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
                    &mut state.ancestry,
                );
            }
        }
        let (body, simplified) = simplify::body(self.module, &function);
        (body, changed || simplified)
    }

    fn choose(
        &self,
        function: &mir::Function,
        ancestry: &[Vec<mir::FunctionId>],
        remaining: &mut usize,
    ) -> Vec<Choice> {
        let analysis = flow::Analysis::new(self.module, function);
        let incoming = analysis.solve();
        let in_loop = graph::cyclic_nodes(&graph::block_edges(&function.body));
        let mut choices = Vec::new();
        for (block_id, block) in function.body.blocks.iter() {
            if ancestry[index(block_id)].len() > 6 {
                continue;
            }
            let Some(mut state) = incoming[index(block_id)].clone() else {
                continue;
            };
            for (statement_index, statement) in block.statements.iter().enumerate() {
                if *remaining == 0 {
                    return choices;
                }
                // Release publication is adjacent to its outer initializer
                // and allocation. Keep that call when copying the sequence.
                let publishes_release =
                    block
                        .statements
                        .get(statement_index + 1)
                        .is_some_and(|next| {
                            matches!(next.kind, mir::StatementKind::PublishReleaseReady { .. })
                        });
                if let mir::StatementKind::Call(effect) = &statement.kind
                    && !publishes_release
                {
                    let call = super::call(effect);
                    if let Some(callee) = policy::local_target(self.module, call)
                        && self.eligible[index(callee)]
                        && !ancestry[index(block_id)].contains(&callee)
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

pub(super) fn owner(
    module: &mir::Module,
    function: mir::FunctionId,
) -> Option<CallableMaterialization> {
    if let Some(source) = module.meta.source_callable_materializations.get(function) {
        return Some(source.materialization());
    }
    let generated = module.meta.generated_callables.get(function)?;
    Some(CallableMaterialization::new(
        CallableTemplateOwner::Generated(generated.identity_record().id()),
        CallableMaterializationContext::NoSubstitution,
    ))
}
