//! Representation-independent cross-Cone type facts and inheritance contracts.
//!
//! These constituents belong to `cross-cone-type-semantics/18` and refer to
//! complete declarations in the shared HIR interface.

mod access;
mod facts;
mod inheritance;
mod representation;
mod representation_fields;
mod representation_policy;
mod section;
mod selected;
mod shared_foundation;
mod slot_contracts;
mod slot_schemas;
mod source_authority;
mod source_callable_selection;
mod wire;

pub use access::*;
pub use facts::*;
pub use inheritance::*;
pub use representation::*;
pub use representation_fields::*;
pub use representation_policy::*;
pub use section::*;
pub use selected::*;
pub use shared_foundation::{
    CheckedSharedTypeFoundationV1, SharedTypeMetadataError, SharedTypeMetadataV1,
};
pub use slot_contracts::*;
pub use slot_schemas::*;
pub use source_authority::*;
pub use source_callable_selection::{
    select_ordinary_source_callables, select_param_free_source_callables,
    select_param_free_source_constructors,
};
