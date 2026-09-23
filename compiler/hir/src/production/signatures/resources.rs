//! Shared allocation preflight for sealed HIR signature projection.

use crate::{ExportHir, Type, TypeId, TypeParamBounds, TypeParamDecl};
use scoop_wire::{BudgetMeter, WireError, WirePath};

/// Accounts the sealed type tree before the shared signature projector
/// allocates its signature and recursion bookkeeping.
pub(in crate::production) fn ty(
    export: &ExportHir,
    id: TypeId,
    binders: usize,
    depth: u64,
    meter: &mut BudgetMeter,
) -> Result<(), WireError> {
    let path = WirePath::root();
    meter.check_semantic_depth(depth, &path)?;
    meter.charge_nodes(1, &path)?;
    meter.charge_work(binders as u64 + depth + 1, &path)?;
    meter.charge_collection_slots(3, &path)?;
    let children: &[TypeId] = match &export.types[id] {
        Type::Unit
        | Type::Integer(_)
        | Type::Boolean
        | Type::String
        | Type::Any
        | Type::Param(_) => &[],
        Type::Struct(id) => &export.struct_applications[*id].arguments,
        Type::Enum(id) => &export.enum_applications[*id].arguments,
        Type::Interface(id) => &export.interface_applications[*id].arguments,
        Type::Class(id) => {
            meter.charge_work(export.objects.len() as u64, &path)?;
            &export.class_applications[*id].arguments
        }
        Type::Tuple(elements) => elements,
        Type::Ptr(pointee) => std::slice::from_ref(pointee),
        Type::Function(id) | Type::FunPtr(id) => {
            let function = &export.function_types[*id];
            ty(export, function.return_type, binders, depth + 1, meter)?;
            &function.parameter_types
        }
    };
    meter.check_table_entries(children.len() as u64, &path)?;
    for child in children {
        ty(export, *child, binders, depth + 1, meter)?;
    }
    Ok(())
}

pub(in crate::production) fn name(value: &str, meter: &mut BudgetMeter) -> Result<(), WireError> {
    let path = WirePath::root();
    meter.check_semantic_leaf(value.len() as u64, &path)?;
    meter.charge_owned_bytes((value.len() as u64).saturating_mul(2), &path)?;
    meter.charge_work((value.len() as u64).saturating_mul(2), &path)
}

pub(in crate::production) fn binders(
    export: &ExportHir,
    parameters: &[TypeParamDecl],
    visible: usize,
    meter: &mut BudgetMeter,
) -> Result<(), WireError> {
    let path = WirePath::root();
    meter.check_table_entries(parameters.len() as u64, &path)?;
    meter.charge_collection_slots((parameters.len() as u64).saturating_mul(4), &path)?;
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
                meter.check_table_entries(count, &path)?;
                meter.charge_work(
                    count.saturating_mul(u64::from(count.max(1).ilog2()) + 1),
                    &path,
                )?;
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
