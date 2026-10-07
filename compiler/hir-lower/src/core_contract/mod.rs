//! Validation and typed registration of compiler-required core declarations.
//!
//! This is the sole HIR boundary that recognizes well-known intrinsic and
//! runtime-facing core contracts. Successful validation emits typed identities;
//! later stages never recover them from declaration names.

use super::*;

mod arrays;
mod callbacks;
mod characters;
mod common;
mod context;
mod coroutines;
mod data_borrow;
mod exceptions;
mod ffi;
mod floating;
mod gc_control;
mod intrinsics;
mod iteration;
mod operators;
mod option;
mod pointers;
mod source_location;
