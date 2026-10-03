//! Explicit safepoint placement and complete root-plan construction.
//!
//! This pass runs after one function's CFG and physical LIR types are final.
//! It inserts managed entry/explicit-loop-header polls, computes backward liveness once,
//! and fills every protocol-specific root plan. Codegen consumes these plans;
//! it never rediscovers CFG safepoints or source liveness.

use std::collections::HashSet;

use la_arena::{Arena, Idx};
use scoop_lir as lir;

use super::function::{LoweredFunction, MappedLoopHeaderPollTarget};
use super::metadata::{repr_shape, sequence};
use super::{LoweringContext, StorageLoweringError, StorageResult};

mod cfg;
mod constants;
mod dataflow;
mod identity;
mod layout;
mod plans;
mod release;
mod roots;
mod scans;
pub(crate) use release::validate as validate_release_bodies;

#[cfg(test)]
mod tests;

use cfg::*;
use constants::fold_constant_branches;
use dataflow::*;
use identity::*;
pub(crate) use layout::lir_size_align;
use plans::*;
use roots::{
    caller_roots, exceptional_root_set, include_managed_operands, live_value_ty,
    statepoint_live_set,
};
pub(crate) use scans::root_scan;

#[derive(Debug, Default)]
pub(super) struct PendingSafepointSites {
    roles: Vec<lir::SafepointSiteRole>,
}

impl PendingSafepointSites {
    pub(super) fn allocate(&mut self, role: lir::SafepointSiteRole) -> lir::SafepointSiteRef {
        let index = u32::try_from(self.roles.len())
            .expect("one LIR function cannot contain more than u32::MAX safepoints");
        self.roles.push(role);
        lir::SafepointSiteRef::from_u32(index)
    }

    fn role(&self, site: lir::SafepointSiteRef) -> lir::SafepointSiteRole {
        self.roles[site.into_u32() as usize]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum LiveValue {
    Param(u32),
    Local(lir::LocalId),
    Temp(lir::TempId),
}

impl LiveValue {
    fn from_value(value: lir::Value) -> Option<Self> {
        match value {
            lir::Value::Param(index) => Some(Self::Param(index)),
            lir::Value::Local(id) => Some(Self::Local(id)),
            lir::Value::Temp(id) => Some(Self::Temp(id)),
            lir::Value::CArgumentStorage(storage) => Some(Self::Local(storage.local())),
            lir::Value::IntegerConst(_)
            | lir::Value::MachineScalar(_)
            | lir::Value::BoolConst(_)
            | lir::Value::NullPointer(_)
            | lir::Value::TypeDescriptor(_)
            | lir::Value::RootScan(_)
            | lir::Value::Global(_)
            | lir::Value::InitializationUnit(_) => None,
        }
    }

    fn source(self) -> lir::CallerRootSource {
        match self {
            Self::Param(index) => lir::CallerRootSource::Param(index),
            Self::Local(id) => lir::CallerRootSource::Local(id),
            Self::Temp(id) => lir::CallerRootSource::Temp(id),
        }
    }

    fn sort_key(self) -> (u8, u32) {
        match self {
            Self::Param(index) => (0, index),
            Self::Local(id) => (1, id.into_raw().into_u32()),
            Self::Temp(id) => (2, id.into_raw().into_u32()),
        }
    }
}

pub(super) fn complete_function(
    context: &LoweringContext,
    mut lowered: LoweredFunction,
    structs: &lir::StructDefs,
    enums: &lir::EnumDefs,
) -> StorageResult<lir::Function> {
    fold_constant_branches(&mut lowered.function);
    prune_unreachable_blocks(&mut lowered);
    insert_polls(
        &mut lowered.function,
        &lowered.loop_header_polls,
        &mut lowered.pending_safepoints,
    );
    annotate_root_plans(context, &mut lowered.function, structs, enums)?;
    assign_safepoint_identities(&mut lowered.function, &lowered.pending_safepoints);
    Ok(lowered.function)
}

fn arena_index<T>(id: Idx<T>) -> usize {
    id.into_raw().into_u32() as usize
}
