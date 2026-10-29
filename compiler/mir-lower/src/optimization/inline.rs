use std::cell::OnceCell;
use std::collections::{HashMap, HashSet};

use la_arena::Arena;
use scoop_identity::CallableMaterialization;

use super::observation::Observer;
pub(super) use policy::heats;

use super::{
    flow,
    graph::{self, index},
    mir, simplify,
};
use crate::local_values::LocalValueRegistry;
use mir::optimization_report::{InlineOutcome, OptimizationDecision, OptimizationSite};

mod class_fields;
mod clone;
mod exception_paths;
mod policy;
mod selection;
mod summary;
mod suspend;
pub(super) use suspend::run as inline_synchronous_coroutines;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Target {
    Local(mir::FunctionId),
    Imported(mir::ExternalCallableUseId),
}

impl Target {
    fn callee(self) -> mir::Callee {
        match self {
            Self::Local(id) => mir::Callee::User(id),
            Self::Imported(id) => mir::Callee::External(id),
        }
    }
}

struct Choice {
    block: mir::BlockId,
    statement: usize,
    callee: Target,
    call: mir::Call,
    blocks: Vec<(mir::BlockId, mir::BasicBlock)>,
}

pub(super) struct Session {
    templates: Arena<mir::Function>,
    protocol: Vec<bool>,
    executable: Vec<bool>,
    exceptional: HashSet<Target>,
    summaries: Vec<OnceCell<summary::Summary>>,
    imported_summaries: HashMap<mir::ExternalCallableUseId, OnceCell<summary::Summary>>,
    callers: Vec<CallerState>,
    required: super::intrinsics::Entries,
    ordinary: bool,
    allocation_elision: bool,
    imported: crate::imported_bodies::ImportedBodies,
}

struct CallerState {
    growth: policy::Growth,
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
                growth: policy::Growth::new(costs[index(id)].size, 128),
                ancestry: vec![vec![Target::Local(id)]; function.body.blocks.len()],
            })
            .collect();
        Self {
            protocol: policy::protocol_entries(module),
            executable: templates
                .iter()
                .map(|(_, function)| policy::executable(function))
                .collect(),
            exceptional: templates
                .iter()
                .map(|(id, function)| (Target::Local(id), function))
                .chain(
                    imported
                        .functions
                        .iter()
                        .map(|(id, function)| (Target::Imported(*id), function)),
                )
                .filter_map(|(target, function)| {
                    (!policy::supported_blocks(function.body.blocks.iter().map(|(_, b)| b)))
                        .then_some(target)
                })
                .collect(),
            summaries: (0..templates.len()).map(|_| OnceCell::new()).collect(),
            imported_summaries: imported
                .functions
                .keys()
                .map(|&id| (id, OnceCell::new()))
                .collect(),
            templates,
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
        selected: &mir::SelectedExternalMirSet,
        dispatch: &crate::imported_bodies::IntrinsicDispatch,
        active: &[bool],
        registry: &mut LocalValueRegistry,
        observer: Option<&Observer<'_>>,
    ) -> Vec<bool> {
        // Devirtualization can reveal cycles that were absent in earlier rounds.
        let recursive = policy::recursive(module);
        let exclusions = self
            .executable
            .iter()
            .enumerate()
            .map(|(id, supported)| {
                if !supported {
                    Some(InlineOutcome::UnsupportedBody)
                } else if recursive[id] {
                    Some(InlineOutcome::RecursiveCallee)
                } else if self.protocol[id] {
                    Some(InlineOutcome::ProtocolCallee)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        let mut changed = vec![false; module.functions.len()];
        for (id, _) in self.templates.iter() {
            if !active[index(id)] {
                continue;
            }
            if self.ordinary && self.protocol[index(id)] {
                if let Some(observer) = observer {
                    for (block_id, block) in module.functions[id].body.blocks.iter() {
                        for (statement, node) in block.statements.iter().enumerate() {
                            if let mir::StatementKind::Call(effect) = &node.kind {
                                observer.record(
                                    OptimizationSite {
                                        function: id,
                                        block: block_id,
                                        statement,
                                        span: node.span,
                                        inline_path: self.ancestry(id, block_id),
                                    },
                                    OptimizationDecision::Inline {
                                        target: super::observation::call_target(
                                            &super::call(effect).target,
                                        ),
                                        outcome: InlineOutcome::ProtocolCaller,
                                        estimate: None,
                                    },
                                );
                            }
                        }
                    }
                }
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
                selected,
                dispatch,
                templates: &self.templates,
                exceptional: &self.exceptional,
                exclusions: &exclusions,
                summaries: &self.summaries,
                imported_summaries: &self.imported_summaries,
                required: &self.required,
                ordinary: self.ordinary,
                allocation_elision: self.allocation_elision,
                imported: &self.imported,
                observer,
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
        selected: &mir::SelectedExternalMirSet,
        dispatch: &crate::imported_bodies::IntrinsicDispatch,
        active: &[bool],
        registry: &mut LocalValueRegistry,
    ) -> Vec<bool> {
        let ordinary = std::mem::replace(&mut self.ordinary, false);
        let changed = self.run(module, selected, dispatch, active, registry, None);
        self.ordinary = ordinary;
        changed
    }

    pub(super) fn ancestry(
        &self,
        function: mir::FunctionId,
        block: mir::BlockId,
    ) -> Vec<mir::Callee> {
        self.callers[index(function)].ancestry[index(block)]
            .iter()
            .map(|target| target.callee())
            .collect()
    }
}

struct Context<'a, 'report> {
    module: &'a mir::Module,
    selected: &'a mir::SelectedExternalMirSet,
    dispatch: &'a crate::imported_bodies::IntrinsicDispatch,
    templates: &'a Arena<mir::Function>,
    exceptional: &'a HashSet<Target>,
    exclusions: &'a [Option<InlineOutcome>],
    summaries: &'a [OnceCell<summary::Summary>],
    imported_summaries: &'a HashMap<mir::ExternalCallableUseId, OnceCell<summary::Summary>>,
    required: &'a super::intrinsics::Entries,
    ordinary: bool,
    allocation_elision: bool,
    imported: &'a crate::imported_bodies::ImportedBodies,
    observer: Option<&'a Observer<'report>>,
}

impl Context<'_, '_> {
    fn target(&self, call: &mir::Call) -> Result<Target, InlineOutcome> {
        if let Some(local) = policy::local_target(self.module, call) {
            return self.exclusions[index(local)].map_or(Ok(Target::Local(local)), Err);
        }
        if let mir::CallTarget::Direct(mir::Callee::External(id)) = call.target {
            return match self.imported.functions.get(&id) {
                Some(function) if policy::executable(function) => Ok(Target::Imported(id)),
                Some(_) => Err(InlineOutcome::UnsupportedBody),
                None => Err(InlineOutcome::NoBody),
            };
        }
        Err(if matches!(call.target, mir::CallTarget::Direct(_)) {
            InlineOutcome::NoBody
        } else {
            InlineOutcome::IndirectCall
        })
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

    fn summary(&self, target: Target, effects: &exception_paths::Effects<'_>) -> &summary::Summary {
        let summary = match target {
            Target::Local(id) => &self.summaries[index(id)],
            Target::Imported(id) => &self.imported_summaries[&id],
        };
        summary.get_or_init(|| summary::Summary::new(self.module, self.template(target), effects))
    }

    fn optimize(
        &self,
        id: mir::FunctionId,
        owner: CallableMaterialization,
        registry: &mut LocalValueRegistry,
        state: &mut CallerState,
    ) -> (mir::Body, bool) {
        let mut function = self.module.functions[id].clone();
        let mut debug_records = mir::DebugRecords::default();
        let mut changed = false;
        loop {
            let choices = self.choose(id, &function, &state.ancestry, &mut state.growth);
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
                    &mut debug_records,
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

pub(super) use crate::local_values::callable_owner as owner;
