//! Complete managed allocation, heap writes, publication and poll emission.
//! Collector-specific fast paths stay here; roots and call ABI remain shared.

use super::*;

mod allocation;
mod allocation_layout;
mod allocation_metadata;
mod arrays;
mod atomics;
mod barrier;
mod card;
mod fields;
mod poll;
mod writes;

use allocation_layout::AllocationLayout;
pub(in crate::function) use fields::FieldBarrierCache;
