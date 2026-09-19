use scoop_wire::{BudgetMeter, WireError, WirePath};

use super::DecodedExportDefaultCallableTargetV1;

impl DecodedExportDefaultCallableTargetV1 {
    /// Accounts for all signature constituents before resolving this typed target.
    pub fn charge_resolution(&self, meter: &mut BudgetMeter) -> Result<(), WireError> {
        let path = WirePath::root();
        meter.charge_nodes(1, &path)?;
        meter.charge_work(1, &path)?;
        match self {
            Self::Callable(value) => value.charge_resolution(meter),
            Self::Bound(value) => value.charge_resolution(meter),
            Self::DerivedEquality { owner_type } => owner_type.charge_resolution(meter),
            Self::LocalFunction { .. }
            | Self::Lambda { .. }
            | Self::AnonymousFunction { .. }
            | Self::CallableReference { .. }
            | Self::FunctionAddress { .. } => Ok(()),
        }
    }
}
