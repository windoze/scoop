use super::*;

pub(super) fn required(
    foundation: &BoundTypeFoundationSourcesV1<'_>,
    inventory: &CanonicalSourceInheritanceInventoriesV1,
    constructors: &CanonicalInheritanceSourceConstructorsV1,
    meter: &mut BudgetMeter,
) -> Result<
    BTreeMap<PersistentConstructorId, PersistentExactTypeId>,
    InheritanceConstructorBindingError,
> {
    use InheritanceConstructorBindingError as Error;
    let path = WirePath::root();
    let edges = foundation
        .source()
        .entries()
        .local_inheritance_edges
        .records();
    binding_keys::charge_map(edges.len(), meter, &path)?;
    binding_keys::charge_map(inventory.records().len(), meter, &path)?;
    if !inventory
        .owners()
        .values()
        .iter()
        .copied()
        .eq(edges.iter().map(NominalInheritanceEdgesV1::owner))
    {
        return Err(Error::Inventory("inheritance owners"));
    }
    let mut required = BTreeMap::new();
    for source in inventory.records() {
        let values = source.constructors().values();
        binding_keys::charge_map(values.len(), meter, &path)?;
        for declaration in values {
            meter.check_table_entries(required.len() as u64 + 1, &path)?;
            charge_queries(1, required.len(), meter)?;
            if required.insert(*declaration, source.owner()).is_some() {
                return Err(Error::RepeatedOwner(*declaration));
            }
        }
    }
    charge_queries(required.len(), required.len(), meter)?;
    binding_keys::charge_map(constructors.records().len(), meter, &path)?;
    if !required.keys().copied().eq(constructors
        .records()
        .iter()
        .map(NominalSupportConstructorInterfaceV1::declaration))
    {
        return Err(Error::Inventory("constructor sources"));
    }
    Ok(required)
}
