use super::CrossConeTypeSemanticsProductionError as Error;
use super::inheritance::source_resources::{self as resources, invalid, resource};
use crate::production::{callable_interfaces, signatures::HirInterfaceSignatureProjector};
use crate::*;
use scoop_identity::SignatureTypeKey;
use scoop_wire::{BudgetMeter, WirePath};

pub(super) fn project(
    export: &ExportHir,
    signatures: &HirInterfaceSignatureProjector<'_>,
    owner: ExportParameterOwner,
    binders: &[HirSignatureBinder],
    expected: &[SignatureTypeKey],
    meter: &mut BudgetMeter,
) -> Result<CanonicalSourceParameterShapesV1, Error> {
    let path = WirePath::root();
    meter
        .charge_work(export.source_parameter_interfaces.len() as u64 * 2, &path)
        .map_err(resource)?;
    for interface in &export.source_parameter_interfaces {
        if interface.owner != owner {
            continue;
        }
        meter
            .check_table_entries(interface.parameters.len() as u64, &path)
            .map_err(resource)?;
        meter
            .charge_collection_slots(interface.parameters.len() as u64 * 3, &path)
            .map_err(resource)?;
        for parameter in &interface.parameters {
            resources::name(&parameter.name, meter)?;
            let ty = match parameter.calling {
                ExportParameterCalling::Required { value_type }
                | ExportParameterCalling::Default { value_type, .. } => value_type,
                ExportParameterCalling::Vararg { parameter_type, .. } => {
                    export.export_vararg_parameter_types[parameter_type].array_type
                }
            };
            resources::ty(export, ty, binders.len(), 3, meter)?;
        }
    }
    callable_interfaces::source_parameter_shapes(export, signatures, owner, binders, expected)
        .map_err(invalid)
}
