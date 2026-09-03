//! Dynamic-dispatch construction.
//!
//! This module owns concrete vtable/itable assembly, interface-variance and
//! function-variance bridges, plus boxed value adjust thunks. Every semantic
//! target and slot identity has already been fixed by concrete HIR.

use super::*;

mod boxing;
mod class_tables;
mod function_bridges;
mod variance;
