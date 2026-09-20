use super::*;
use scoop_identity::SignatureTypeKey;

pub(in crate::production::type_semantics) fn insert<T: Ord>(
    set: &mut BTreeSet<T>,
    value: T,
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    let path = WirePath::root();
    work(meter, set.len())?;
    meter
        .check_table_entries(set.len() as u64 + 1, &path)
        .map_err(resource)?;
    meter.charge_collection_slots(1, &path).map_err(resource)?;
    if !set.insert(value) {
        return Err(invalid("duplicate declaration in nested source inventory"));
    }
    Ok(())
}
pub(in crate::production::type_semantics) fn push<T>(
    values: &mut Vec<T>,
    value: T,
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    let path = WirePath::root();
    meter
        .check_table_entries(values.len() as u64 + 1, &path)
        .map_err(resource)?;
    meter
        .try_reserve_collection_slots(values, 1, &path)
        .map_err(resource)?;
    values.push(value);
    Ok(())
}
pub(in crate::production::type_semantics) fn boxed<T>(
    value: &T,
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    meter
        .charge_owned_bytes(std::mem::size_of_val(value) as u64, &WirePath::root())
        .map_err(resource)
}
pub(in crate::production::type_semantics) fn canonical(
    count: usize,
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    let path = WirePath::root();
    let count = count as u64;
    meter.check_table_entries(count, &path).map_err(resource)?;
    // Canonical support sorting and the payload's exact reference closure both
    // allocate fixed-width typed keys and temporary collection entries.
    meter
        .charge_collection_slots(count.saturating_mul(6), &path)
        .map_err(resource)?;
    meter
        .charge_owned_bytes(count.saturating_mul(384), &path)
        .map_err(resource)?;
    meter
        .charge_work(
            count
                .saturating_mul(128)
                .saturating_mul(u64::from(count.max(1).ilog2()) + 1),
            &path,
        )
        .map_err(resource)
}
pub(in crate::production::type_semantics) fn signature_copy(
    ty: &SignatureTypeKey,
    meter: &mut BudgetMeter,
    depth: u64,
) -> Result<(), Error> {
    let path = WirePath::root();
    meter.check_semantic_depth(depth, &path).map_err(resource)?;
    meter.charge_nodes(1, &path).map_err(resource)?;
    meter.charge_work(1, &path).map_err(resource)?;
    boxed(ty, meter)?;
    let children = match ty {
        SignatureTypeKey::Nominal(_) | SignatureTypeKey::Binder { .. } => &[][..],
        SignatureTypeKey::NominalApplication { arguments, .. }
        | SignatureTypeKey::Tuple(arguments) => arguments.as_slice(),
        SignatureTypeKey::Function {
            parameters, result, ..
        }
        | SignatureTypeKey::NativeFunctionPointer {
            parameters, result, ..
        } => {
            signature_copy(result, meter, depth + 1)?;
            parameters.as_slice()
        }
        SignatureTypeKey::RawPointer(pointee) => std::slice::from_ref(pointee.as_ref()),
    };
    meter
        .check_table_entries(children.len() as u64, &path)
        .map_err(resource)?;
    for child in children {
        signature_copy(child, meter, depth + 1)?;
    }
    Ok(())
}
