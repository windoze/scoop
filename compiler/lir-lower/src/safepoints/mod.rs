//! Explicit safepoint placement and complete root-plan construction.
//!
//! This pass runs after one function's CFG and physical LIR types are final.
//! It inserts managed entry/explicit-loop-header polls, computes backward liveness once,
//! and fills every protocol-specific root plan. Codegen consumes these plans;
//! it never rediscovers CFG safepoints or source liveness.

use std::collections::HashSet;

use la_arena::{Arena, Idx};
use scoop_lir as lir;

use super::LoweringContext;
use super::function::{LoweredFunction, MappedLoopHeaderPollTarget};
use super::metadata::{repr_shape, sequence};

mod cfg;
mod dataflow;
mod plans;
mod scans;

use cfg::*;
use dataflow::*;
use plans::*;
use scans::{
    caller_roots, exceptional_root_set, include_managed_operands, live_value_ty,
    statepoint_live_set,
};
pub(crate) use scans::{lir_size_align, root_scan};

#[derive(Debug)]
pub(super) struct SafepointIds {
    next: u64,
}

impl Default for SafepointIds {
    fn default() -> Self {
        Self { next: 1 }
    }
}

impl SafepointIds {
    pub(super) fn allocate(&mut self) -> lir::SafepointId {
        let raw = self.next;
        self.next = raw
            .checked_add(1)
            .expect("a Scoop image cannot contain u64::MAX safepoints");
        lir::SafepointId::new(raw).expect("SafepointIds starts at one")
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
    lowered: &mut LoweredFunction,
    structs: &lir::StructDefs,
    enums: &lir::EnumDefs,
    ids: &mut SafepointIds,
) {
    fold_constant_branches(&mut lowered.function);
    prune_unreachable_blocks(lowered);
    insert_polls(&mut lowered.function, &lowered.loop_header_polls, ids);
    annotate_root_plans(context, &mut lowered.function, structs, enums);
}

fn arena_index<T>(id: Idx<T>) -> usize {
    id.into_raw().into_u32() as usize
}
