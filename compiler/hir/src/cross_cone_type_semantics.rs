//! Representation-independent cross-Cone type facts and inheritance contracts.
//!
//! These constituents belong to `cross-cone-type-semantics/3`; neither the
//! public lookup interface nor the native-boundary witness grants this authority.

mod access;
mod definition_sources;
mod facts;
mod inheritance;
mod protected_defaults;
mod protected_interfaces;
mod protected_source_interfaces;
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
pub use definition_sources::*;
pub use facts::*;
pub use inheritance::*;
pub use protected_defaults::*;
pub use protected_interfaces::*;
pub use protected_source_interfaces::*;
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
