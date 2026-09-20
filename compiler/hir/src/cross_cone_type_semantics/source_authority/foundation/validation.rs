use super::*;

pub(super) fn validate(
    entries: &TypeFoundationSourceEntriesV1,
    meter: &mut BudgetMeter,
) -> Result<(), TypeFoundationSourceError> {
    let path = WirePath::root();
    meter.check_semantic_depth(1, &path)?;
    meter.charge_nodes(1, &path)?;
    for (field, count) in [
        (2, entries.exact_keys.values().len()),
        (3, entries.sources.records().len()),
        (4, entries.representations.records().len()),
        (5, entries.generated_nominals.values().len()),
        (6, entries.accessor_keys.values().len()),
        (7, entries.definition_sources.sources().len()),
        (8, entries.source_roots.values().len()),
        (9, entries.local_exact_facts.values().len()),
        (10, entries.dependency_facts.records().len()),
        (11, entries.local_inheritance_edges.records().len()),
        (12, entries.fact_shapes.records().len()),
        (13, entries.representation_owners.values().len()),
    ] {
        let at = path.clone().field(field);
        meter.check_table_entries(count as u64, &at)?;
        meter.charge_work(count as u64, &at)?;
    }
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
        meter.charge_work(
            u64::from(local.len().max(1).ilog2()) + 1,
            &path.clone().field(10),
        )?;
        if local.binary_search(&dependency.exact).is_ok() {
            return Err(TypeFoundationSourceError::FactOwnershipOverlap(
                dependency.exact,
            ));
        }
    }

    let mut inheritance_owners = Vec::new();
    let count = entries.representations.records().len();
    let at = path.clone().field(11);
    meter.try_reserve_collection_slots(&mut inheritance_owners, count, &at)?;
    meter.charge_work(
        (count as u64).saturating_mul(u64::from(count.max(1).ilog2()) + 1),
        &at,
    )?;
    for representation in entries.representations.records() {
        let owner = representation.owner();
        meter.charge_work(
            u64::from(entries.sources.records().len().max(1).ilog2()) + 1,
            &at,
        )?;
        let source = entries
            .sources
            .get(SourceNominalId::Concrete(owner))
            .ok_or(TypeFoundationSourceError::MissingSourceOwner(owner))?;
        // Equality can traverse both owner chains and inline logical paths.
        for access in [source.access(), representation.declaration_access()] {
            let path_bytes = access
                .definition_origin()
                .origin()
                .source()
                .logical_path()
                .as_str()
                .len() as u64;
            let owner_bytes = (access.lexical_owners().len() as u64).saturating_mul(33);
            meter.charge_work(
                path_bytes.saturating_add(owner_bytes).saturating_add(128),
                &at,
            )?;
        }
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
