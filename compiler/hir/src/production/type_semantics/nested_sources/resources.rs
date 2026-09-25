use super::*;

pub(in crate::production::type_semantics) fn insert<T: Ord>(
    set: &mut BTreeSet<T>,
    value: T,
) -> Result<(), Error> {
    if !set.insert(value) {
        return Err(invalid("duplicate declaration in nested source inventory"));
    }
    Ok(())
}
pub(in crate::production::type_semantics) fn push<T>(
    values: &mut Vec<T>,
    value: T,
) -> Result<(), Error> {
    let path = WirePath::root();

    scoop_wire::allocation::try_reserve(values, 1, &path).map_err(resource)?;
    values.push(value);
    Ok(())
}
