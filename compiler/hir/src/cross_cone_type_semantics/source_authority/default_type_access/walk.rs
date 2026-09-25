use super::*;
use DefaultSourceTypeAccessDemandV1 as Demand;
use SignatureTypeKey as Type;

pub(super) fn visit<'t, E>(
    ty: &'t Type,
    path: &WirePath,
    visitor: &mut impl FnMut(Demand<'t>, &WirePath) -> Result<(), E>,
) -> Result<(), E> {
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
        visitor(demand, path)?;
    }
    match ty {
        Type::NominalApplication { arguments, .. } => {
            let path = (path).clone().field(2);
            sequence(arguments.as_slice(), &path, visitor)
        }
        Type::Tuple(elements) => {
            let path = (path).clone().field(1);
            sequence(elements.as_slice(), &path, visitor)
        }
        Type::Function {
            parameters, result, ..
        }
        | Type::NativeFunctionPointer {
            parameters, result, ..
        } => {
            let parameters_path = (path).clone().field(2);
            sequence(parameters, &parameters_path, visitor)?;
            let result_path = (path).clone().field(3);
            visit(result, &result_path, visitor)
        }
        Type::RawPointer(pointee) => {
            let path = (path).clone().field(1);
            visit(pointee, &path, visitor)
        }
        Type::Nominal(_) | Type::Binder { .. } => Ok(()),
    }
}
fn sequence<'t, E>(
    types: &'t [Type],
    path: &WirePath,
    visitor: &mut impl FnMut(Demand<'t>, &WirePath) -> Result<(), E>,
) -> Result<(), E> {
    for (index, ty) in types.iter().enumerate() {
        visit(ty, &path.clone().index(index as u64), visitor)?;
    }
    Ok(())
}
