//! MIR-facing stable identity and symbol facade.
//!
//! Foundational hash/key primitives and the shared ABI/private manglers remain
//! in `scoopc_ids`. Type-aware semantic keys still come from `scoopc_hir` while
//! P9 is splitting the stages; this module is the MIR-owned import surface for
//! materialization and later symbol selection.

pub use scoopc_hir::stable_id::*;
