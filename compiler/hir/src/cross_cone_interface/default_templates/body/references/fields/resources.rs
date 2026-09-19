use scoop_wire::{BudgetMeter, WireError, WirePath};

use super::DecodedDefaultFieldRefV1;

impl DecodedDefaultFieldRefV1 {
    /// Accounts for the complete owner signature before aggregate resolution.
    pub fn charge_resolution(&self, meter: &mut BudgetMeter) -> Result<(), WireError> {
        let path = WirePath::root();
        meter.charge_nodes(1, &path)?;
        meter.charge_work(1, &path)?;
        match self {
            Self::Struct { owner_type, .. } | Self::Class { owner_type, .. } => {
                owner_type.charge_resolution(meter)
            }
            Self::Tuple { .. } => Ok(()),
        }
    }
}
