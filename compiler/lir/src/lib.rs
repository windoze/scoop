//! LIR definitions and LIR meta: the data channel between LIR and codegen.
//!
//! See `docs/specs/SCOOP-IMPL-SPEC.md` section 2.4. The crate exposes one
//! flat public data model while keeping identity/types, module entities,
//! metadata, functions, instructions and calls in separate source modules.

pub use scoop_identity::{
    AbiArrayElement, AbiCarrier, AbiCoercion, AbiCoercionError, AbiPart,
    AtomicCompareExchangeOrder, AtomicCompareExchangeResult, AtomicLoadOrder, AtomicMemoryOrder,
    AtomicRmwOperation, AtomicStoreOrder, AtomicValueKind, CAbiCallMode, CResultAdaptation,
    FloatBinaryOperator, FloatConstant as LirFloatConstant, FloatKind, FloatUnaryOperator,
    PointerKind,
};

use std::num::NonZeroU32;

use la_arena::{Arena, Idx};

mod types;
pub use types::*;

mod atomic;
pub use atomic::*;

mod target;
pub use target::*;

mod optimization;
pub use optimization::OptimizationMode;

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

mod c_call_plan;
pub use c_call_plan::*;

mod calls;
pub use calls::*;

pub use scoop_identity::ConeIdentity;

mod callable_abi;
pub use callable_abi::*;

mod external_callable;
pub use external_callable::*;

mod external_callable_abi;

mod external_type_descriptor;
pub use external_type_descriptor::*;

mod module;
pub use module::*;

mod metadata;
pub use metadata::*;

mod type_relations;
pub use type_relations::*;

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

mod cone_output;
pub use cone_output::*;

mod canonical_callable;
pub use canonical_callable::*;
mod canonical_type;

mod canonical_shape;
pub use canonical_shape::*;

mod production;
pub use production::*;

mod cross_cone_bridge;
pub use cross_cone_bridge::*;

mod gateway;
pub use gateway::{GatewayValidationError, validate_startup_gateways};

mod function;
pub use function::*;

mod release;
pub use release::*;

mod boxing;
pub use boxing::*;

mod integer;
pub use integer::*;

mod instruction;
pub use instruction::*;

mod dump;
pub use dump::{dump, dump_initialization_dependencies};

mod link_data;
pub use link_data::LinkDataError;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod test_support;

mod task_context;
pub use task_context::{
    CallableContextKeyCellV1, ContextKey, context_key_cell_atom, context_key_table_atom,
    function_context_keys,
};

pub type LirFloatConversion = scoop_identity::FloatConversion<IntegerKind>;
