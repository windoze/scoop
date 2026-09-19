use scoop_wire::{BudgetMeter, WireError, WirePath};

use super::{
    DecodedDefaultBoundCallableRefV1, DecodedDefaultBoundCallableSourceV1,
    DecodedDefaultCallableRefV1, DecodedOptionalSignatureType,
};

impl DecodedDefaultCallableRefV1 {
    /// Accounts for owned signature trees before aggregate resolution.
    pub fn charge_resolution(&self, meter: &mut BudgetMeter) -> Result<(), WireError> {
        let path = WirePath::root();
        meter.charge_nodes(1, &path)?;
        meter.charge_work(1, &path)?;
        if let DecodedOptionalSignatureType::Present(owner) = &self.owner {
            owner.charge_resolution(meter)?;
        }
        meter.charge_collection_slots(self.type_arguments.len() as u64, &path)?;
        for argument in &self.type_arguments {
            argument.charge_resolution(meter)?;
        }
        Ok(())
    }
}

impl DecodedDefaultBoundCallableRefV1 {
    /// Accounts for the bound source and instantiated signature with one budget.
    pub fn charge_resolution(&self, meter: &mut BudgetMeter) -> Result<(), WireError> {
        let path = WirePath::root();
        meter.charge_nodes(1, &path)?;
        meter.charge_work(1, &path)?;
        match &self.source {
            DecodedDefaultBoundCallableSourceV1::Class { bound, callable } => {
                bound.charge_resolution(meter)?;
                callable.charge_resolution(meter)?;
            }
            DecodedDefaultBoundCallableSourceV1::Interface { bound, .. } => {
                bound.charge_resolution(meter)?;
            }
        }
        self.instantiated_signature.charge_resolution(meter)
    }
}
