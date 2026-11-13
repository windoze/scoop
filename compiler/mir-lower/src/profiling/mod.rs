//! Establish logical positions before optional MIR transformations.

use std::collections::{HashMap, HashSet};

use la_arena::Arena;
use scoop_identity::CallableDefinitionOwner;
use scoop_mir as mir;
use scoop_profile::{
    BodyId, BodyKey, BodyLayout, Feedback, Instrumentation, Position, PositionKind,
};

use crate::cfg::{CallSite, CleanupTransfer};
use crate::imported_bodies::ImportedBodies;

mod fingerprint;
mod insert;

#[derive(Clone, Copy, Debug, Default)]
pub enum ProfilingInput<'a> {
    #[default]
    Off,
    Generate,
    Use(&'a Feedback),
}

impl ProfilingInput<'_> {
    pub(super) fn enabled(self) -> bool {
        !matches!(self, Self::Off)
    }

    pub(super) fn apply(self, profiling: &mut mir::MirProfiling) {
        profiling.instrumentation = if matches!(self, Self::Generate) {
            Instrumentation::Generate
        } else {
            Instrumentation::Off
        };
        if let Self::Use(feedback) = self {
            profiling.feedback = true;
            for (index, plan) in profiling.plans.iter().enumerate() {
                for (position, definition) in plan.positions.iter().enumerate() {
                    let site = mir::ProfileSite {
                        plan: mir::ProfilePlanId(index as u32),
                        position: position as u32,
                    };
                    let location = scoop_profile::Location {
                        body: plan.key,
                        position: position as u32,
                    };
                    profiling.counts.insert(site, feedback.count(location));
                    if matches!(definition.kind, PositionKind::Call | PositionKind::Block) {
                        profiling
                            .frequencies
                            .insert(site, feedback.frequency(location));
                    }
                }
            }
        }
    }
}

enum Owner {
    Local(mir::FunctionId),
    Imported(mir::ExternalCallableUseId),
    Release(mir::ReleaseHookId),
}

struct Snapshot {
    owner: Owner,
    function: mir::Function,
    order: Vec<mir::BlockId>,
    sites: Vec<CallSite>,
    positions: Vec<Position>,
}

pub(super) struct Planning {
    snapshots: Vec<Snapshot>,
    original_functions: HashSet<mir::FunctionId>,
}

impl Planning {
    pub(super) fn prepare(
        functions: &mut Arena<mir::Function>,
        calls: &mut HashMap<mir::FunctionId, Vec<CallSite>>,
        imported: &mut ImportedBodies,
    ) -> Self {
        let mut planning = Self {
            snapshots: Vec::new(),
            original_functions: functions.iter().map(|(id, _)| id).collect(),
        };
        for (id, function) in functions.iter_mut() {
            planning.register(
                Owner::Local(id),
                function,
                calls.get_mut(&id).map_or(&mut [], Vec::as_mut_slice),
            );
        }
        let mut imported_ids = imported.functions.keys().copied().collect::<Vec<_>>();
        imported_ids.sort();
        for id in imported_ids {
            planning.register(
                Owner::Imported(id),
                imported.functions.get_mut(&id).unwrap(),
                imported
                    .call_sites
                    .get_mut(&id)
                    .map_or(&mut [], Vec::as_mut_slice),
            );
        }
        planning
    }

    fn register(&mut self, owner: Owner, function: &mut mir::Function, sites: &mut [CallSite]) {
        if function.body.blocks.iter().all(|(_, block)| {
            block.statements.is_empty() && matches!(block.terminator, mir::Terminator::Unreachable)
        }) {
            return;
        }
        let original = function.clone();
        let order = order(&original.body, sites);
        let original_sites = sites.to_vec();
        let id = mir::ProfilePlanId(
            u32::try_from(self.snapshots.len()).expect("profile plans fit the MIR index width"),
        );
        let positions = insert::positions(id, function, &order, sites);
        self.snapshots.push(Snapshot {
            owner,
            function: original,
            order,
            sites: original_sites,
            positions,
        });
    }

    pub(super) fn finish(mut self, module: &mut mir::Module) -> mir::MirProfiling {
        for (id, function) in module.functions.iter_mut() {
            if !self.original_functions.contains(&id) {
                self.register(Owner::Local(id), function, &mut []);
            }
        }
        for (id, hook) in module.release_hooks.iter_mut() {
            self.register(Owner::Release(id), &mut hook.code, &mut []);
        }
        let plans = self
            .snapshots
            .into_iter()
            .map(|snapshot| {
                let body_key = match snapshot.owner {
                    Owner::Local(id) => CallableDefinitionOwner::try_from(
                        module
                            .meta
                            .callable_signature_subject(id)
                            .expect("a lowered callable retains its implementation subject"),
                    )
                    .expect("a concrete callable has a definition owner")
                    .body_key(),
                    Owner::Imported(id) => CallableDefinitionOwner::from(
                        module.meta.external_callables[id]
                            .reference()
                            .implementation(),
                    )
                    .body_key(),
                    Owner::Release(id) => {
                        let ty = mir::Type::Class(module.release_hooks[id].owner);
                        let owner = module
                            .meta
                            .source_exact_types
                            .get(&ty)
                            .expect("a release hook retains its source exact owner");
                        scoop_identity::CallableBodyKey::release_hook(owner.identity_record().id())
                    }
                };
                let fingerprint = fingerprint::function(
                    module,
                    &snapshot.function,
                    &snapshot.order,
                    &snapshot.sites,
                    &snapshot.positions,
                );
                BodyLayout {
                    key: BodyKey {
                        body: BodyId::from_key(&body_key),
                        fingerprint,
                    },
                    name: snapshot.function.name,
                    positions: snapshot.positions,
                }
            })
            .collect();
        mir::MirProfiling {
            plans,
            ..Default::default()
        }
    }
}

/// Deterministic DFS order from semantic entry and deferred coroutine roots.
/// Unreachable arena padding and allocator order do not define positions.
fn order(body: &mir::Body, sites: &[CallSite]) -> Vec<mir::BlockId> {
    let mut roots = vec![body.entry];
    for site in sites {
        roots.push(site.block);
        for transfer in &site.pending.0 {
            match transfer {
                CleanupTransfer::Fallthrough(target) => roots.push(target.block()),
                CleanupTransfer::Break(target) => roots.push(target.block()),
                CleanupTransfer::Continue(target) => roots.push(target.block()),
                CleanupTransfer::ManagedThrow {
                    unwind: Some(target),
                    ..
                } => roots.push(target.block()),
                CleanupTransfer::ReturnUnit
                | CleanupTransfer::ReturnValue(_)
                | CleanupTransfer::ManagedThrow { unwind: None, .. } => {}
            }
        }
    }
    let mut visited = HashSet::new();
    let mut result = Vec::new();
    for root in roots {
        let mut stack = vec![root];
        while let Some(id) = stack.pop() {
            if !visited.insert(id) {
                continue;
            }
            result.push(id);
            let block = &body.blocks[id];
            if let Some(unwind) = block.unwind {
                stack.push(unwind);
            }
            match block.terminator {
                mir::Terminator::Goto(target) => stack.push(target),
                mir::Terminator::Branch {
                    then_block,
                    else_block,
                    ..
                } => {
                    stack.push(else_block);
                    stack.push(then_block);
                }
                mir::Terminator::Throw {
                    unwind: Some(target),
                    ..
                }
                | mir::Terminator::Rethrow {
                    unwind: Some(target),
                } => stack.push(target),
                _ => {}
            }
        }
    }
    result
}
