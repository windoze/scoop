use scoop_wire::{BudgetMeter, WireError};

use super::charge;
use crate::{
    CanonicalTemplateLocalTableV1, DecodedCanonicalBinderUseListV1,
    DecodedCanonicalTemplateLocalTableV1, DecodedCanonicalTemplateValueParametersV1,
    DecodedExportDefaultBodyV1, DecodedOptionalTemplateReceiverV1,
};

impl DecodedExportDefaultBodyV1 {
    /// Preflights the resolved body and its intrinsic local-index projection.
    pub fn charge_resolution(
        &self,
        locals: &CanonicalTemplateLocalTableV1,
        meter: &mut BudgetMeter,
    ) -> Result<(), WireError> {
        charge(self, Some(locals), 2, meter)
    }
}
impl DecodedOptionalTemplateReceiverV1 {
    pub fn charge_resolution(
        &self,
        locals: &CanonicalTemplateLocalTableV1,
        meter: &mut BudgetMeter,
    ) -> Result<(), WireError> {
        charge(self, Some(locals), 2, meter)
    }
}
impl DecodedCanonicalTemplateValueParametersV1 {
    pub fn charge_resolution(
        &self,
        locals: &CanonicalTemplateLocalTableV1,
        meter: &mut BudgetMeter,
    ) -> Result<(), WireError> {
        charge(self, Some(locals), 2, meter)
    }
}
impl DecodedCanonicalTemplateLocalTableV1 {
    /// Includes owned signatures and adjacent canonical-selector comparisons.
    pub fn charge_resolution(&self, meter: &mut BudgetMeter) -> Result<(), WireError> {
        charge(self, None, 2, meter)
    }
}
impl DecodedCanonicalBinderUseListV1 {
    pub fn charge_resolution(&self, meter: &mut BudgetMeter) -> Result<(), WireError> {
        charge(self, None, 1, meter)
    }
}
