//! Complete semantic section with independent source and terminal closure.

use super::*;
use scoop_identity::{ConeIdentity, PersistentInitializationUnitId, StrongCallableDefinitionOwner};

mod build;
mod closure;
mod context;
mod dependencies;
mod dependency;
mod error;
mod model;
mod replay;
mod selection;
mod source;
#[cfg(test)]
mod tests;
mod units;
mod view;
mod wire;

pub use dependency::MirTypeBridgeDependencyV1;
pub use error::*;
pub use model::*;
pub use selection::{SelectedDependencyMirTypeRefV1, SelectedDependencyMirTypeSetV1};
pub use source::*;
pub use units::{MirInitializationUnitProofKindV1, MirTypeBridgeInitializationUnitV1};
pub use wire::{
    DecodedCrossConeMirTypeBridgeSectionV1, TypeResolvedCrossConeMirTypeBridgeSectionV1,
};

fn reserve<T>(count: usize, meter: &mut BudgetMeter) -> Result<Vec<T>, WireError> {
    meter.check_table_entries(count as u64, &WirePath::root())?;
    let mut values = Vec::new();
    meter.try_reserve_collection_slots(&mut values, count, &WirePath::root())?;
    Ok(values)
}
fn sort_work(count: usize, meter: &mut BudgetMeter) -> Result<(), WireError> {
    meter.check_table_entries(count as u64, &WirePath::root())?;
    for _ in 0..usize::BITS - count.max(1).saturating_sub(1).leading_zeros() {
        meter.charge_work(count as u64, &WirePath::root())?;
    }
    Ok(())
}
