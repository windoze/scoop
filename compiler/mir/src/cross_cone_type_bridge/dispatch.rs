//! Typed dispatch tables and reference-receiver adaptation. Complete source
//! selection remains an enclosing HIR/MIR production join.

use super::*;
use scoop_identity::{
    DispatchDeclarationOwner, PersistentDispatchSlotId, StrongCallableDefinitionOwner,
};

mod error;
mod graph;
mod implementation_wire;
mod model;
mod validation;
mod wire;

pub use error::MirDispatchSchemaError;
pub use model::*;
pub use wire::{DecodedCanonicalMirDispatchSchemasV1, DecodedParamFreeMirDispatchSchemaV1};

fn reserve<T>(count: usize, meter: &mut BudgetMeter) -> Result<Vec<T>, WireError> {
    meter.check_table_entries(count as u64, &WirePath::root())?;
    let mut values = Vec::new();
    meter.try_reserve_collection_slots(&mut values, count, &WirePath::root())?;
    Ok(values)
}
fn charge_sort(length: usize, meter: &mut BudgetMeter) -> Result<(), WireError> {
    meter.check_table_entries(length as u64, &WirePath::root())?;
    for _ in 0..usize::BITS - length.max(1).saturating_sub(1).leading_zeros() {
        meter.charge_work(length as u64, &WirePath::root())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
