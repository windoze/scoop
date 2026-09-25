//! Meter the existing producer-unit reconstruction before any allocation.

use super::*;
use scoop_wire::{BudgetMeter, encode_canonical_temporary_with_meter};

pub(crate) fn producer_units(
    foundation: &OdrFreeLirFoundation,
    bridges: &scoop_lir::GeneratedBridgePlanSetV1,
    meter: &mut BudgetMeter,
) -> Result<StrongProducerUnitPartitionV1, StrongLinkMaterializationError> {
    let path = WirePath::root().field(1);
    let count = foundation.definition_plan_count() as u64;
    meter.check_table_entries(count, &path)?;
    meter.charge_nodes(count, &path)?;
    // Includes the atom/unit indexes and the final owned definition vectors.
    meter.charge_collection_slots(count.saturating_mul(4), &path)?;
    meter.charge_owned_bytes(count.saturating_mul(4 * 64), &path)?;
    meter.charge_work(
        count.saturating_mul(4 * (1 + u64::from(count.max(1).ilog2()))),
        &path,
    )?;
    let bridge_bytes = encode_canonical_temporary_with_meter(bridges, meter, &path)?;
    let bytes = bridge_bytes.len() as u64;
    meter.charge_owned_bytes(bytes.saturating_mul(2), &path)?;
    meter.charge_collection_slots(bytes, &path)?;
    meter.charge_work(
        bytes.saturating_mul(4 * (1 + u64::from(bytes.max(1).ilog2()))),
        &path,
    )?;
    StrongProducerUnitPartitionV1::from_odr_free_foundation(foundation)
        .map_err(StrongLinkMaterializationError::ProducerUnits)
}

impl From<WireError> for StrongLinkMaterializationError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
