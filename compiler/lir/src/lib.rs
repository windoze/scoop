//! LIR definitions and LIR meta: the data channel between LIR and codegen.
//!
//! See `docs/specs/SCOOP-IMPL-SPEC.md` section 2.4. The crate exposes one
//! flat public data model while keeping identity/types, module entities,
//! metadata, functions, instructions and calls in separate source modules.

use std::num::NonZeroU32;

use la_arena::{Arena, Idx};

mod types;
pub use types::*;

mod target;
pub use target::*;

mod backend;
pub use backend::*;

mod runtime_abi;
pub use runtime_abi::*;

mod c_bridge_toolchain;
pub use c_bridge_toolchain::*;

mod c_bridge_invocation;
pub use c_bridge_invocation::*;

mod target_selection;
pub use target_selection::*;

mod abi;
pub use abi::*;

mod externs;
pub use externs::*;

mod calls;
pub use calls::*;

mod core_external;
pub use core_external::*;

mod dependency_external;
pub use dependency_external::*;

mod external_callable_abi;

mod external_type_descriptor;
pub use external_type_descriptor::*;

mod module;
pub use module::*;

mod metadata;
pub use metadata::*;

mod scan;
pub use scan::*;

mod value_layout;
pub use value_layout::*;

mod exact_layout;
pub use exact_layout::*;

mod exact_abi;
pub use exact_abi::*;

mod exact_descriptor;
pub use exact_descriptor::*;

mod exact_dispatch;
pub use exact_dispatch::*;

mod exact_shape_support;
pub use exact_shape_support::*;

mod layout_abi;
pub use layout_abi::*;

mod layout_external;
pub use layout_external::*;

mod shape_link;
pub use shape_link::*;

mod type_descriptor;
pub use type_descriptor::*;

mod identity_metadata;
pub use identity_metadata::*;

mod generated_bridge;
pub use generated_bridge::*;

mod materialization;
pub use materialization::*;

mod safepoint;
pub use safepoint::*;

mod foundation;
pub use foundation::*;

mod strong_output;
pub use strong_output::*;

mod production;
pub use production::*;

mod cross_cone_bridge;
pub use cross_cone_bridge::*;

mod function;
pub use function::*;

mod boxing;
pub use boxing::*;

mod integer;
pub use integer::*;

mod instruction;
pub use instruction::*;

mod dump;
pub use dump::dump;

#[cfg(test)]
mod tests;
