//! Representation-independent cross-Cone type facts and inheritance contracts.
//!
//! These constituents belong to `cross-cone-type-semantics/1`; neither the
//! public lookup interface nor the native-boundary witness grants this authority.

mod access;
mod facts;
mod inheritance;
mod representation;
mod representation_fields;
mod representation_policy;
mod slot_schemas;
mod wire;

pub use access::*;
pub use facts::*;
pub use inheritance::*;
pub use representation::*;
pub use representation_fields::*;
pub use representation_policy::*;
pub use slot_schemas::*;
