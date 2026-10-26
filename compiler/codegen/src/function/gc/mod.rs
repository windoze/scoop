//! Complete managed allocation, heap writes, publication and poll emission.
//! Collector-specific fast paths stay here; roots and call ABI remain shared.

use super::*;

mod allocation;
mod allocation_context;
mod allocation_initialization;
mod allocation_layout;
mod allocation_metadata;
mod arrays;
mod atomics;
mod barrier;
mod card;
mod fields;
mod poll;
mod writes;
mod young_initialization;

pub(super) use allocation_layout::AllocationLayout;
pub(in crate::function) use fields::FieldBarrierCache;
pub(in crate::function) use young_initialization::{YoungAllocation, single_predecessor_blocks};
