use scoop_wire::{BudgetMeter, WireError, WirePath};

use super::{CanonicalReexportRoutesV1, ReexportRouteHopV1};

impl CanonicalReexportRoutesV1 {
    /// Shared by public re-export validation and dependency binding witnesses.
    /// Charge each comparison before visiting potentially long common prefixes.
    pub(crate) fn contains_exact_suffix_metered(
        &self,
        suffix: &[ReexportRouteHopV1],
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<bool, WireError> {
        meter.charge_work(1, path)?;
        meter.check_table_entries(self.routes.len() as u64, path)?;
        meter.check_table_entries(suffix.len() as u64, path)?;
        meter.check_semantic_depth(suffix.len() as u64, path)?;
        let Some(first) = suffix.first() else {
            return Ok(false);
        };
        for route in &self.routes {
            meter.charge_work(1, path)?;
            if route.immediate_provider() != first.exporter() || route.hops().len() != suffix.len()
            {
                continue;
            }
            let mut equal = true;
            for (actual, expected) in route.hops().iter().zip(suffix) {
                meter.charge_work(1, path)?;
                if actual != expected {
                    equal = false;
                    break;
                }
            }
            if equal {
                return Ok(true);
            }
        }
        Ok(false)
    }
}
