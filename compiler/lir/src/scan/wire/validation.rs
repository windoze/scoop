use scoop_wire::{BudgetMeter, WireError, WireErrorKind, WirePath};

use super::*;

impl DecodedRefScanV1 {
    /// Charges the additional typed scan and canonical-byte allocations made
    /// during refinement, separately from the raw decoder's allocations.
    pub fn validate_metered(
        self,
        meter: &mut BudgetMeter,
    ) -> Result<CheckedRefScanV1, MeteredScanValidationError> {
        let path = WirePath::root();
        let bytes = charge(&self, meter, &path, 1)?;
        meter.charge_owned_bytes(bytes, &path)?;
        self.validate().map_err(MeteredScanValidationError::Scan)
    }
}

fn charge(
    scan: &DecodedRefScanV1,
    meter: &mut BudgetMeter,
    path: &WirePath,
    depth: u64,
) -> Result<u64, WireError> {
    meter.check_semantic_depth(depth, path)?;
    meter.charge_work(1, path)?;
    meter.charge_nodes(1, path)?;
    match &scan.0 {
        RawScan::None => Ok(4),
        RawScan::References(offsets) => {
            let count = offsets.len() as u64;
            meter.charge_work(count, path)?;
            meter.charge_collection_slots(count, path)?;
            count
                .checked_mul(8)
                .and_then(|bytes| bytes.checked_add(12))
                .ok_or_else(|| overflow(path))
        }
        RawScan::Sequence(parts) => {
            meter.charge_collection_slots(parts.len() as u64, path)?;
            let mut bytes = 12_u64;
            for part in parts {
                bytes = bytes
                    .checked_add(charge(part, meter, path, depth + 1)?)
                    .ok_or_else(|| overflow(path))?;
            }
            Ok(bytes)
        }
        RawScan::Array { element, .. } => charge(element, meter, path, depth + 1)?
            .checked_add(28)
            .ok_or_else(|| overflow(path)),
    }
}

fn overflow(path: &WirePath) -> WireError {
    WireError::new(WireErrorKind::IntegerOutOfRange, path.clone(), None)
}

#[derive(Debug)]
pub enum MeteredScanValidationError {
    Scan(RefScanValidationError),
    Resource(WireError),
}
impl From<WireError> for MeteredScanValidationError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl std::fmt::Display for MeteredScanValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid metered scan: {self:?}")
    }
}
impl std::error::Error for MeteredScanValidationError {}
