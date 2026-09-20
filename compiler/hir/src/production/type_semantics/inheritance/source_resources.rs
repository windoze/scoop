use super::*;
use scoop_wire::{BudgetMeter, WirePath};

pub(in crate::production) fn name(value: &str, meter: &mut BudgetMeter) -> Result<(), Error> {
    let path = WirePath::root();
    meter
        .check_semantic_leaf(value.len() as u64, &path)
        .map_err(resource)?;
    meter
        .charge_owned_bytes((value.len() as u64).saturating_mul(2), &path)
        .map_err(resource)?;
    meter
        .charge_work((value.len() as u64).saturating_mul(2), &path)
        .map_err(resource)
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
    let path = WirePath::root();
    meter
        .check_table_entries(parameters.len() as u64, &path)
        .map_err(resource)?;
    meter
        .charge_collection_slots((parameters.len() as u64).saturating_mul(4), &path)
        .map_err(resource)?;
    for parameter in parameters {
        name(&parameter.name, meter)?;
        name(&parameter.name, meter)?;
        match &parameter.bounds {
            TypeParamBounds::Unconstrained
            | TypeParamBounds::Value { .. }
            | TypeParamBounds::Ref { .. } => continue,
            TypeParamBounds::Nominal(bounds) => {
                if let Some(class) = &bounds.class {
                    ty(
                        export,
                        export.class_applications[class.application].canonical_type,
                        visible,
                        3,
                        meter,
                    )?;
                }
                let count = bounds.interfaces.len() as u64;
                meter.check_table_entries(count, &path).map_err(resource)?;
                meter
                    .charge_work(
                        count.saturating_mul(u64::from(count.max(1).ilog2()) + 1),
                        &path,
                    )
                    .map_err(resource)?;
                for interface in &bounds.interfaces {
                    ty(
                        export,
                        export.interface_applications[interface.application].canonical_type,
                        visible,
                        3,
                        meter,
                    )?;
                }
            }
        }
    }
    Ok(())
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
