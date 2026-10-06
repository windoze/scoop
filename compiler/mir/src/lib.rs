//! MIR definitions and MIR meta: the data channel between MIR and LIR.
//!
//! See `docs/specs/SCOOP-IMPL-SPEC.md` section 2.3 and
//! `docs/milestone2/DESIGN.md` section 2.3.
//!
//! Structural equality on aggregates is already expanded by mir-lower into
//! primitive comparisons and runtime calls. Source integers use the closed,
//! kind-carrying integer operation nodes; [`BinOp`] remains only for Boolean
//! and compiler-owned machine-scalar operations. Since M10 every emitted
//! function body is a CFG and calls are explicit effect statements; [`Expr`]
//! cannot contain a call.

pub use scoop_identity::{
    FloatBinaryOperator, FloatConstant as MirFloatConstant, FloatKind, FloatUnaryOperator,
};

use la_arena::{Arena, Idx};

pub use scoop_identity::{
    CallableOwner, ConeIdentity, CoreImportedCallableKind, ImmortalObjectKey, ImmortalObjectOwner,
    OdrGroupId, PersistentExactTypeId, PersistentFieldId, PersistentInitializationUnitId,
    PropertyOwner, SourceNativeExternalContractRecord, SourceSpan, StructuralDefinitionPath,
    StructuralDefinitionSiteRole, StructuralPathSegment,
};

mod ids;
pub use ids::*;

mod integer;
pub use integer::*;

mod types;
pub use types::*;

mod function_adapter;
pub use function_adapter::*;

mod exact_owner;
pub use exact_owner::*;

mod source_exact_types;
pub use source_exact_types::*;
mod cone_input;
pub use cone_input::*;

mod dependency_output;
pub use dependency_output::*;

mod external_callable;
pub use external_callable::*;

mod generated_exact_types;
pub use generated_exact_types::*;

mod generated_callables;
pub use generated_callables::*;

mod source_callable_materializations;
pub use source_callable_materializations::*;

mod local_values;
pub use local_values::*;
mod release;
pub use release::*;

mod closure_environment;
pub use closure_environment::*;

mod function_bridge;
pub use function_bridge::*;

mod static_callback_bridge;
pub use static_callback_bridge::*;

mod coroutine_shape;
pub use coroutine_shape::*;

mod coroutine_support;
pub use coroutine_support::*;

mod coroutine_state_machine;
pub use coroutine_state_machine::*;

mod continuation_adapter;
pub use continuation_adapter::*;

mod boxed_value;
pub use boxed_value::*;

mod boxing_adjust;
pub use boxing_adjust::*;

mod identity_metadata;
pub use identity_metadata::*;

mod callable_signatures;
pub use callable_signatures::*;

mod foundation;
pub use foundation::*;

mod production;
pub use production::*;

mod cross_cone_type_bridge;
pub use cross_cone_type_bridge::*;

mod cross_cone_bridge;
pub use cross_cone_bridge::*;

mod module;
pub use module::*;

mod validation;
pub use validation::*;

mod context;
mod control_flow;
pub use context::{
    ContextKey, ContextOperation, ContextStorageRole, ContextStorageType, context_fields,
    context_type_representation,
};
pub use control_flow::*;

mod dump;
pub use dump::{dump, type_name};

pub type MirFloatConversion = scoop_identity::FloatConversion<IntegerKind>;
