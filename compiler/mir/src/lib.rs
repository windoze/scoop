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

use la_arena::{Arena, Idx};
use scoop_ast::Span;

mod symbols;
pub use symbols::*;

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

mod source_callable_materializations;
pub use source_callable_materializations::*;

mod source_local_values;
pub use source_local_values::*;

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

mod boxed_value;
pub use boxed_value::*;

mod boxing_adjust;
pub use boxing_adjust::*;

mod identity_metadata;
pub use identity_metadata::*;

mod foundation;
pub use foundation::*;

mod module;
pub use module::*;

mod validation;
pub use validation::*;

mod control_flow;
pub use control_flow::*;

mod dump;
pub use dump::{dump, type_name};
