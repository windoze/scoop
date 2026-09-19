use scoop_wire::{BudgetMeter, WireError, WireErrorKind, WirePath, encoded_length};

use super::ResourceChildren;
use crate::CanonicalTemplateLocalTableV1;

fn selector_length(value: &impl scoop_wire::WireEncode) -> Result<u64, WireError> {
    encoded_length(value)
        .map_err(|_| WireError::new(WireErrorKind::IntegerOutOfRange, WirePath::root(), None))
}

pub(super) fn selector_bound(
    locals: Option<&CanonicalTemplateLocalTableV1>,
    meter: &mut BudgetMeter,
) -> Result<u64, WireError> {
    let mut largest = 0;
    if let Some(locals) = locals {
        for record in locals.records() {
            let bytes = selector_length(record.selector())?;
            meter.charge_work(bytes, &WirePath::root())?;
            largest = largest.max(bytes);
        }
    }
    Ok(largest)
}

impl ResourceChildren<'_, '_> {
    pub(crate) fn local_index(&mut self, index: u32) -> Result<(), WireError> {
        let Some(locals) = self.locals else {
            return Ok(());
        };
        let path = WirePath::root();
        self.meter.charge_work(1, &path)?;
        if let Some(record) = locals.records().get(index as usize) {
            self.push(record.selector())?;
            // The intrinsic indexed projection performs a binary search in the
            // canonical local table. Bound every compared selector's full path.
            let steps = u64::from(u32::BITS - locals.len_u32().leading_zeros()) + 1;
            self.meter
                .charge_work(self.selector_bytes.saturating_mul(steps), &path)?;
        }
        Ok(())
    }
}
