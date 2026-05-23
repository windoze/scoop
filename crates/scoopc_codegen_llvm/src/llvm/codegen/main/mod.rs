//! `MainCodegen` implementation split into focused submodules.
//!
//! Each submodule extends `impl<'a, 'ctx> MainCodegen<'a, 'ctx>` with a
//! cohesive group of methods (frame management, call lowering, expression
//! lowering, …). The methods retain `pub(in crate::llvm::codegen)` visibility
//! so other codegen-tree modules (e.g. `mir_body/`, `effect_lowered/`)
//! continue to call them just as they did when the impl block lived in
//! `codegen/mod.rs` directly.

#![allow(dead_code)]

use super::*;

mod alloca;
mod boxing;
mod call;
mod coerce;
mod cone_init;
mod context;
mod declare;
mod expr_op;
mod expr_value;
mod frame;
mod function;
mod gc_locals;
mod globals;
mod identity;
mod immut_value;
mod literal;
mod numeric;
mod runtime_error;
