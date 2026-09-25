use super::*;

pub(super) fn required(
    foundation: &BoundTypeFoundationSourcesV1<'_>,
    inventory: &CanonicalSourceInheritanceInventoriesV1,
    constructors: &CanonicalInheritanceSourceConstructorsV1,
) -> Result<
    BTreeMap<PersistentConstructorId, PersistentExactTypeId>,
    InheritanceConstructorBindingError,
> {
    use InheritanceConstructorBindingError as Error;

    let edges = foundation
        .source()
        .entries()
        .local_inheritance_edges
        .records();

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

        for declaration in values {
            if required.insert(*declaration, source.owner()).is_some() {
                return Err(Error::RepeatedOwner(*declaration));
            }
        }
    }

    if !required.keys().copied().eq(constructors
        .records()
        .iter()
        .map(NominalSupportConstructorInterfaceV1::declaration))
    {
        return Err(Error::Inventory("constructor sources"));
    }
    Ok(required)
}
