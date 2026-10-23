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

mod class_fields;
mod clone;
mod policy;
mod selection;
mod suspend;
pub(super) use suspend::run as inline_synchronous_coroutines;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Target {
    Local(mir::FunctionId),
    Imported(mir::ExternalCallableUseId),
}

struct Choice {
    block: mir::BlockId,
    statement: usize,
    callee: Target,
    blocks: Vec<(mir::BlockId, mir::BasicBlock)>,
}

pub(super) struct Session {
    templates: Arena<mir::Function>,
    protocol: Vec<bool>,
    supported: Vec<bool>,
    costs: Vec<policy::Cost>,
    callers: Vec<CallerState>,
    required: super::intrinsics::Entries,
    ordinary: bool,
    allocation_elision: bool,
    imported: crate::imported_bodies::ImportedBodies,
}

struct CallerState {
    remaining: usize,
    ancestry: Vec<Vec<Target>>,
}

impl Session {
    pub(super) fn new(
        module: &mir::Module,
        required: super::intrinsics::Entries,
        ordinary: bool,
        allocation_elision: bool,
        imported: crate::imported_bodies::ImportedBodies,
    ) -> Self {
        // Stable templates avoid counting an already expanded body as one level.
        let templates = module.functions.clone();
        let costs = templates
            .iter()
            .map(|(_, function)| policy::function_cost(&module.structs, function))
            .collect::<Vec<_>>();
        let callers = templates
            .iter()
            .map(|(id, function)| CallerState {
                remaining: (costs[index(id)].size / 2 + 96).min(384),
                ancestry: vec![vec![Target::Local(id)]; function.body.blocks.len()],
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
            required,
            ordinary,
            allocation_elision,
            imported,
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
            if !active[index(id)] || (self.ordinary && self.protocol[index(id)]) {
                continue;
            }
            if !self.ordinary
                && !module.functions[id].body.blocks.iter().any(|(_, block)| {
                    block.statements.iter().any(|statement| {
                        if let mir::StatementKind::Call(effect) = &statement.kind {
                            matches!(super::call(effect).target, mir::CallTarget::Direct(callee)
                                if self.required.contains(module, callee))
                        } else {
                            false
                        }
                    })
                })
            {
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
                required: &self.required,
                ordinary: self.ordinary,
                allocation_elision: self.allocation_elision,
                imported: &self.imported,
            };
            let (body, rewritten) =
                context.optimize(id, owner, registry, &mut self.callers[index(id)]);
            module.functions[id].body = body;
            changed[index(id)] = rewritten;
        }
        changed
    }

    pub(super) fn run_required(
        &mut self,
        module: &mut mir::Module,
        active: &[bool],
        registry: &mut LocalValueRegistry,
    ) -> Vec<bool> {
        let ordinary = std::mem::replace(&mut self.ordinary, false);
        let changed = self.run(module, active, registry);
        self.ordinary = ordinary;
        changed
    }
}

struct Context<'a> {
    module: &'a mir::Module,
    templates: &'a Arena<mir::Function>,
    eligible: &'a [bool],
    costs: &'a [policy::Cost],
    required: &'a super::intrinsics::Entries,
    ordinary: bool,
    allocation_elision: bool,
    imported: &'a crate::imported_bodies::ImportedBodies,
}

impl Context<'_> {
    fn target(&self, call: &mir::Call) -> Option<Target> {
        if let Some(local) = policy::local_target(self.module, call) {
            return self.eligible[index(local)].then_some(Target::Local(local));
        }
        if let mir::CallTarget::Direct(mir::Callee::External(id)) = call.target
            && self
                .imported
                .functions
                .get(&id)
                .is_some_and(policy::supported)
        {
            return Some(Target::Imported(id));
        }
        None
    }

    fn template(&self, target: Target) -> &mir::Function {
        match target {
            Target::Local(id) => &self.templates[id],
            Target::Imported(id) => &self.imported.functions[&id],
        }
    }

    fn required(&self, target: Target) -> bool {
        match target {
            Target::Local(id) => self.required.contains(self.module, mir::Callee::User(id)),
            Target::Imported(id) => self.required.external.contains(&id),
        }
    }

    fn cost(&self, target: Target) -> policy::Cost {
        match target {
            Target::Local(id) => self.costs[index(id)],
            Target::Imported(id) => {
                policy::function_cost(&self.module.structs, &self.imported.functions[&id])
            }
        }
    }

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
                let callee = self.template(choice.callee);
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
        if !self.ordinary && !changed {
            return (function.body, false);
        }
        let (body, simplified) = simplify::body(self.module, &function);
        (body, changed || simplified)
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
