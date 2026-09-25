use crate::{RefScan, TypeInstanceShapeV1};
use scoop_wire::{BudgetMeter, WireError, WirePath};

pub(super) fn shape(value: &TypeInstanceShapeV1, meter: &mut BudgetMeter) -> Result<(), WireError> {
    scan(value.object_scan(), meter, 1)?;
    scan(value.inline_scan(), meter, 1)
}

pub(crate) fn scan(value: &RefScan, meter: &mut BudgetMeter, depth: u64) -> Result<(), WireError> {
    let path = WirePath::root();
    meter.check_semantic_depth(depth, &path)?;
    meter.charge_work(1, &path)?;
    meter.charge_nodes(1, &path)?;
    match value {
        RefScan::None => Ok(()),
        RefScan::References(offsets) => {
            meter.charge_work(offsets.len() as u64, &path)?;
            meter.charge_collection_slots(offsets.len() as u64, &path)?;
            meter.charge_owned_bytes(
                (offsets.len() as u64).saturating_mul(std::mem::size_of::<u64>() as u64),
                &path,
            )
        }
        RefScan::Sequence(parts) => {
            meter.charge_collection_slots(parts.len() as u64, &path)?;
            meter.charge_owned_bytes(
                (parts.len() as u64).saturating_mul(std::mem::size_of::<RefScan>() as u64),
                &path,
            )?;
            for part in parts {
                scan(part, meter, depth + 1)?;
            }
            Ok(())
        }
        RefScan::Array { element, .. } => {
            meter
                .charge_owned_bytes(std::mem::size_of::<crate::NonEmptyRefScan>() as u64, &path)?;
            scan(element.as_ref_scan(), meter, depth + 1)
        }
    }
}
