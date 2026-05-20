//! Migration adapter for compiler-wide type-system foundations.
//!
//! `scoopc_types` is the authoritative owner. This module keeps the historical
//! `scoopc::ty` path available while downstream code migrates imports.

pub use scoopc_types::*;
