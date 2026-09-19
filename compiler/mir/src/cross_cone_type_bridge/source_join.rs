//! Complete local source joins, independent of transport-table contents.
//!
//! Terminal provider and selected-use closure are separate section obligations.

use super::*;
use scoop_identity::{ConeIdentity, PersistentObjectValueId, StrongCallableDefinitionOwner};

mod model;
pub(in crate::cross_cone_type_bridge) mod validation;

pub use model::*;
pub use validation::MirTypeBridgeSourceJoinError;

#[cfg(test)]
mod tests;
