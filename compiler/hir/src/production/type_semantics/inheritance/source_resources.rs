use super::*;
use scoop_wire::{BudgetMeter, WirePath};

pub(in crate::production) fn name(value: &str, meter: &mut BudgetMeter) -> Result<(), Error> {
    crate::production::signatures::resources::name(value, meter).map_err(resource)
}

pub(in crate::production) fn ty(
    export: &ExportHir,
    id: TypeId,
    binders: usize,
    depth: u64,
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    crate::production::signatures::resources::ty(export, id, binders, depth, meter)
        .map_err(resource)
}

pub(in crate::production) fn binders(
    export: &ExportHir,
    parameters: &[TypeParamDecl],
    visible: usize,
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    crate::production::signatures::resources::binders(export, parameters, visible, meter)
        .map_err(resource)
}

pub(in crate::production) fn work(meter: &mut BudgetMeter, length: usize) -> Result<(), Error> {
    meter
        .charge_work(u64::from(length.max(1).ilog2()) + 1, &WirePath::root())
        .map_err(resource)
}
pub(in crate::production) fn resource(error: scoop_wire::WireError) -> Error {
    Error::SourceInventory(SourceInventoryError::Resource(error))
}
pub(in crate::production) fn invalid(reason: impl std::fmt::Display) -> Error {
    Error::InvalidSourceDeclaration(reason.to_string())
}
