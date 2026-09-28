//! Typed dispatch tables and reference-receiver adaptation. Complete source
//! selection remains an enclosing HIR/MIR production join.

use super::*;
use scoop_identity::{DispatchDeclarationOwner, PersistentDispatchSlotId};

mod error;
mod graph;
mod implementation_wire;
mod model;
mod slot_wire;
mod validation;
mod wire;

pub use error::MirDispatchSchemaError;
pub use model::*;
pub use wire::{DecodedCanonicalMirDispatchSchemasV1, DecodedParamFreeMirDispatchSchemaV1};

fn reserve<T>(count: usize) -> Result<Vec<T>, WireError> {
    let mut values = Vec::new();
    scoop_wire::allocation::try_reserve(&mut values, count, &WirePath::root())?;
    Ok(values)
}

#[cfg(test)]
pub(in crate::cross_cone_type_bridge) mod tests;

#[cfg(test)]
use scoop_identity::StrongCallableDefinitionOwner;
