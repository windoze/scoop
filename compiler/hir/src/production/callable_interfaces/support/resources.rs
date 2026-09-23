//! Preflight allocations made by the shared support signature projector.

use crate::production::signatures::resources::ty;
use crate::{
    ExportHir, ExportParameterCalling, ExportParameterOwner, FunctionId, TypeId, TypeParamDecl,
};
use scoop_wire::{BudgetMeter, WireError, WirePath};

pub(super) fn function(
    export: &ExportHir,
    id: FunctionId,
    meter: &mut BudgetMeter,
) -> Result<(), WireError> {
    let function = &export.functions[id];
    let visible = function.type_param_count();
    meter.charge_collection_slots(visible as u64 * 4, &WirePath::root())?;
    for parameter in function.type_params() {
        binder(export, parameter, visible, meter)?;
    }
    parameters(export, ExportParameterOwner::Function(id), visible, meter)?;
    ty(export, function.return_ty, visible, 1, meter)
}

pub(super) fn nominal(
    export: &ExportHir,
    binders: &[TypeParamDecl],
    owner: ExportParameterOwner,
    result: TypeId,
    meter: &mut BudgetMeter,
) -> Result<(), WireError> {
    meter.charge_collection_slots(binders.len() as u64 * 4, &WirePath::root())?;
    for parameter in binders {
        binder(export, parameter, binders.len(), meter)?;
    }
    parameters(export, owner, binders.len(), meter)?;
    ty(export, result, binders.len(), 1, meter)
}

fn parameters(
    export: &ExportHir,
    owner: ExportParameterOwner,
    visible: usize,
    meter: &mut BudgetMeter,
) -> Result<(), WireError> {
    let path = WirePath::root();
    meter.charge_work(export.source_parameter_interfaces.len() as u64, &path)?;
    for interface in export
        .source_parameter_interfaces
        .iter()
        .filter(|interface| interface.owner == owner)
    {
        meter.check_table_entries(interface.parameters.len() as u64, &path)?;
        meter.charge_collection_slots(interface.parameters.len() as u64 * 3, &path)?;
        for parameter in &interface.parameters {
            name(&parameter.name, meter)?;
            let value = match parameter.calling {
                ExportParameterCalling::Required { value_type }
                | ExportParameterCalling::Default { value_type, .. } => value_type,
                ExportParameterCalling::Vararg { parameter_type, .. } => {
                    export.export_vararg_parameter_types[parameter_type].array_type
                }
            };
            // The projector maps the HIR type and copies its matching key type.
            ty(export, value, visible, 1, meter)?;
            ty(export, value, visible, 1, meter)?;
        }
    }
    Ok(())
}

fn binder(
    export: &ExportHir,
    parameter: &TypeParamDecl,
    visible: usize,
    meter: &mut BudgetMeter,
) -> Result<(), WireError> {
    name(&parameter.name, meter)?;
    if let Some(bound) = parameter.class_bound() {
        ty(
            export,
            export.class_applications[bound.application].canonical_type,
            visible,
            1,
            meter,
        )?;
    }
    let bounds = parameter.interface_bounds();
    let count = bounds.len() as u64;
    meter.charge_work(
        count.saturating_mul(u64::from(count.max(1).ilog2()) + 1),
        &WirePath::root(),
    )?;
    for bound in bounds {
        ty(
            export,
            export.interface_applications[bound.application].canonical_type,
            visible,
            1,
            meter,
        )?;
    }
    Ok(())
}

fn name(value: &str, meter: &mut BudgetMeter) -> Result<(), WireError> {
    let path = WirePath::root();
    meter.check_semantic_leaf(value.len() as u64, &path)?;
    meter.charge_owned_bytes(value.len() as u64 * 2, &path)?;
    meter.charge_work(value.len() as u64 * 2, &path)
}
