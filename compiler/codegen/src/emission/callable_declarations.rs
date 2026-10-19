//! The callable symbols needed by one physical LLVM module.

use std::collections::BTreeSet;

use super::*;
use scoop_lir::{
    CallDestination, CallbackBridgeId, ExternalCallableId, ForeignCallbackBridgeId,
    FunctionAddressTarget, LocalFunctionId, ManagedCallDestination, NoGcCallDestination,
    ProtocolCallTargets,
};

pub(super) struct CallableDeclarations<'a> {
    pub functions: Vec<&'a Function>,
    pub external: Vec<ExternalCallableId>,
    pub callbacks: Vec<CallbackBridgeId>,
    pub foreign_callbacks: Vec<ForeignCallbackBridgeId>,
}

impl<'a> CallableDeclarations<'a> {
    pub fn new(module: &'a Module, selection: StrongObjectEmissionSelection<'a>) -> Self {
        let StrongObjectEmissionSelection::CallableBody(function) = selection else {
            return Self {
                functions: module.callable_bodies().collect(),
                external: module
                    .meta
                    .external_callables
                    .iter()
                    .map(|(id, _)| id)
                    .collect(),
                callbacks: module.callback_bridges.iter().map(|(id, _)| id).collect(),
                foreign_callbacks: module
                    .foreign_callback_bridges
                    .iter()
                    .map(|(id, _)| id)
                    .collect(),
            };
        };
        let mut references = References::default();
        // Both ordinary calls and invokes select targets from these arenas.
        // C and native-borrowed targets are extern-only and declared at use.
        references.targets(
            &function.call_targets.managed_targets,
            ManagedCallDestination::view,
        );
        references.targets(
            &function.call_targets.no_gc_targets,
            NoGcCallDestination::view,
        );
        for instruction in function
            .blocks
            .iter()
            .flat_map(|(_, block)| &block.instructions)
        {
            match instruction {
                Instruction::FunctionAddress {
                    target: FunctionAddressTarget::Local(reference),
                    ..
                } => {
                    references.local.push(reference.declaration());
                }
                Instruction::FunctionAddress {
                    target: FunctionAddressTarget::CallbackTrampoline(bridge),
                    ..
                } => {
                    references.callbacks.insert(*bridge);
                }
                Instruction::ForeignCallbackRegister { bridge, .. } => {
                    references.foreign_callbacks.insert(*bridge);
                    references.local.push(
                        module.foreign_callback_bridges[*bridge]
                            .adapter
                            .declaration(),
                    );
                }
                _ => {}
            }
        }
        references.local.sort_unstable_by_key(|id| id.into_u32());
        references.local.dedup();
        let functions = std::iter::once(function)
            .chain(
                references
                    .local
                    .into_iter()
                    .map(|id| &module.functions[id.into_u32() as usize])
                    .filter(|target| target.callable_body.id() != function.callable_body.id()),
            )
            .collect();
        Self {
            functions,
            external: references.external.into_iter().collect(),
            callbacks: references.callbacks.into_iter().collect(),
            foreign_callbacks: references.foreign_callbacks.into_iter().collect(),
        }
    }
}

#[derive(Default)]
struct References {
    local: Vec<LocalFunctionId>,
    external: BTreeSet<ExternalCallableId>,
    callbacks: BTreeSet<CallbackBridgeId>,
    foreign_callbacks: BTreeSet<ForeignCallbackBridgeId>,
}

impl References {
    fn targets<D: Copy>(
        &mut self,
        targets: &ProtocolCallTargets<D>,
        view: fn(D) -> CallDestination,
    ) {
        let destinations = targets
            .void
            .iter()
            .map(|(_, target)| target.destination)
            .chain(
                targets
                    .elided_zst
                    .iter()
                    .map(|(_, target)| target.destination),
            )
            .chain(targets.direct.iter().map(|(_, target)| target.destination))
            .chain(
                targets
                    .indirect_result
                    .iter()
                    .map(|(_, target)| target.destination),
            );
        for destination in destinations {
            match view(destination) {
                CallDestination::Local(id) => {
                    self.local.push(id);
                }
                CallDestination::External(id) => {
                    self.external.insert(id);
                }
                CallDestination::Runtime(_)
                | CallDestination::Extern(_)
                | CallDestination::Dispatch { .. } => {}
            }
        }
    }
}
