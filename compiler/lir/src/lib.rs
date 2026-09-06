//! LIR definitions and LIR meta: the data channel between LIR and codegen.
//!
//! See `docs/specs/SCOOP-IMPL-SPEC.md` section 2.4. The crate exposes one
//! flat public data model while keeping identity/types, module entities,
//! metadata, functions, instructions and calls in separate source modules.

use std::num::{NonZeroU32, NonZeroU64};

use la_arena::{Arena, Idx};

mod types;
pub use types::*;

mod target;
pub use target::*;

mod abi;
pub use abi::*;

mod externs;
pub use externs::*;

mod calls;
pub use calls::*;

mod module;
pub use module::*;

mod metadata;
pub use metadata::*;

mod function;
pub use function::*;

mod integer;
pub use integer::*;

mod instruction;
pub use instruction::*;

mod dump;
pub use dump::dump;

#[cfg(test)]
mod tests;
