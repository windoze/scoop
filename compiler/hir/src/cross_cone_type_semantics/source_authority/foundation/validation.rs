use super::*;

pub(super) fn validate(
    entries: &TypeFoundationSourceEntriesV1,
) -> Result<(), TypeFoundationSourceError> {
    let path = WirePath::root();

    if entries
        .sources
        .records()
        .iter()
        .map(TypeSourceNominalV1::owner)
        .ne(entries.source_roots.values().iter().copied())
    {
        return Err(TypeFoundationSourceError::Inventory { field: 8 });
    }
    if entries
        .representations
        .records()
        .iter()
        .map(NominalRepresentationSupportV1::owner)
        .ne(entries.representation_owners.values().iter().copied())
    {
        return Err(TypeFoundationSourceError::Inventory { field: 13 });
    }
    if entries
        .fact_shapes
        .records()
        .iter()
        .map(ExactTypeFactShapeRecordV1::exact)
        .ne(entries.local_exact_facts.values().iter().copied())
    {
        return Err(TypeFoundationSourceError::Inventory { field: 9 });
    }

    let local = entries.local_exact_facts.values();
    for dependency in entries.dependency_facts.records() {
        if local.binary_search(&dependency.exact).is_ok() {
            return Err(TypeFoundationSourceError::FactOwnershipOverlap(
                dependency.exact,
            ));
        }
    }

    let mut inheritance_owners = Vec::new();
    let count = entries.representations.records().len();
    let at = path.clone().field(11);
    scoop_wire::allocation::try_reserve(&mut inheritance_owners, count, &at)?;

    for representation in entries.representations.records() {
        let owner = representation.owner();

        let source = entries
            .sources
            .get(SourceNominalId::Concrete(owner))
            .ok_or(TypeFoundationSourceError::MissingSourceOwner(owner))?;
        if source.access() != representation.declaration_access() {
            return Err(TypeFoundationSourceError::RepresentationAccess(owner));
        }
        inheritance_owners.push(
            PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(owner))
                .map_err(|error| TypeFoundationSourceError::Identity(error.to_string()))?,
        );
    }
    inheritance_owners.sort_unstable();
    if entries
        .local_inheritance_edges
        .records()
        .iter()
        .map(NominalInheritanceEdgesV1::owner)
        .ne(inheritance_owners)
    {
        return Err(TypeFoundationSourceError::Inventory { field: 11 });
    }
    Ok(())
}
