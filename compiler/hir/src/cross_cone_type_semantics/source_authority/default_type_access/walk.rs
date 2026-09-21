use super::*;
use DefaultSourceTypeAccessDemandV1 as Demand;
use DefaultSourceTypeAccessVisitError as Error;
use SignatureTypeKey as Type;

pub(super) fn visit<'t, E>(
    ty: &'t Type,
    meter: &mut BudgetMeter,
    path: &WirePath,
    depth: u64,
    visitor: &mut impl FnMut(Demand<'t>, &mut BudgetMeter, &WirePath) -> Result<(), E>,
) -> Result<(), Error<E>> {
    meter.check_semantic_depth(depth, path)?;
    meter.charge_nodes(1, path)?;
    meter.charge_work(1, path)?;
    let entries = match ty {
        Type::NominalApplication { arguments, .. } | Type::Tuple(arguments) => {
            arguments.as_slice().len()
        }
        Type::Function { parameters, .. } | Type::NativeFunctionPointer { parameters, .. } => {
            parameters.len()
        }
        Type::Nominal(_) | Type::RawPointer(_) | Type::Binder { .. } => 0,
    };
    meter.check_table_entries(entries as u64, path)?;
    let demand = match ty {
        Type::Nominal(id) => Some(Demand::Nominal(*id)),
        Type::NominalApplication { origin, arguments } => Some(Demand::NominalApplication {
            origin: *origin,
            arguments: arguments.as_slice(),
        }),
        Type::RawPointer(pointee) => Some(Demand::RawPointer { pointee }),
        Type::NativeFunctionPointer {
            calling_convention,
            parameters,
            result,
        } => Some(Demand::NativeFunctionPointer {
            calling_convention: *calling_convention,
            parameters,
            result,
        }),
        Type::Binder { depth, index } => Some(Demand::Binder {
            depth: *depth,
            index: *index,
        }),
        Type::Tuple(_) | Type::Function { .. } => None,
    };
    if let Some(demand) = demand {
        visitor(demand, meter, path).map_err(Error::Visitor)?;
    }
    match ty {
        Type::NominalApplication { arguments, .. } => {
            let path = field(meter, path, 2)?;
            sequence(arguments.as_slice(), meter, &path, depth, visitor)
        }
        Type::Tuple(elements) => {
            let path = field(meter, path, 1)?;
            sequence(elements.as_slice(), meter, &path, depth, visitor)
        }
        Type::Function {
            parameters, result, ..
        }
        | Type::NativeFunctionPointer {
            parameters, result, ..
        } => {
            let parameters_path = field(meter, path, 2)?;
            sequence(parameters, meter, &parameters_path, depth, visitor)?;
            let result_path = field(meter, path, 3)?;
            child(result, meter, &result_path, depth, visitor)
        }
        Type::RawPointer(pointee) => {
            let path = field(meter, path, 1)?;
            child(pointee, meter, &path, depth, visitor)
        }
        Type::Nominal(_) | Type::Binder { .. } => Ok(()),
    }
}
fn sequence<'t, E>(
    types: &'t [Type],
    meter: &mut BudgetMeter,
    path: &WirePath,
    depth: u64,
    visitor: &mut impl FnMut(Demand<'t>, &mut BudgetMeter, &WirePath) -> Result<(), E>,
) -> Result<(), Error<E>> {
    meter.check_table_entries(types.len() as u64, path)?;
    for (index, ty) in types.iter().enumerate() {
        charge_path(meter, path)?;
        child(ty, meter, &path.clone().index(index as u64), depth, visitor)?;
    }
    Ok(())
}
fn child<'t, E>(
    ty: &'t Type,
    meter: &mut BudgetMeter,
    path: &WirePath,
    depth: u64,
    visitor: &mut impl FnMut(Demand<'t>, &mut BudgetMeter, &WirePath) -> Result<(), E>,
) -> Result<(), Error<E>> {
    meter.charge_edges(1, path)?;
    visit(ty, meter, path, depth.saturating_add(1), visitor)
}
fn field(meter: &mut BudgetMeter, path: &WirePath, field: u32) -> Result<WirePath, WireError> {
    charge_path(meter, path)?;
    Ok(path.clone().field(field))
}
fn charge_path(meter: &mut BudgetMeter, path: &WirePath) -> Result<(), WireError> {
    let length = path.segments().len() as u64 + 1;
    meter.charge_work(length, path)?;
    meter.charge_owned_bytes(
        length.saturating_mul(4 * std::mem::size_of::<scoop_wire::PathSegment>() as u64),
        path,
    )
}
