//! Dynamic-dispatch construction.
//!
//! This module owns concrete vtable/itable assembly, function-type variance
//! bridges, and boxed value adjust thunks. Every semantic target and slot
//! identity has already been fixed by concrete HIR.

use super::*;

mod boxing;
mod class_tables;
mod function_bridges;
mod variance;
