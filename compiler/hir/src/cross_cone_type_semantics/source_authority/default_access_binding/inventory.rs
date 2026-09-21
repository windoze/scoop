use super::*;
use scoop_identity::DefinitionOwnerAtom;

pub(super) fn bind<'f>(
    foundation: &BoundTypeFoundationSourcesV1<'f>,
    source: &CanonicalDefaultSourceAccessDeclarationsV1,
    required: &BTreeSet<Subject>,
    meter: &mut BudgetMeter,
) -> Result<BTreeMap<Subject, &'f SourceDeclarationKey>, Error> {
    let path = WirePath::root();
    binding_keys::charge_map(required.len(), meter, &path)?;
    meter.check_table_entries(source.records().len() as u64, &path)?;
    for subject in required {
        DefaultSourceAccessDeclarationV1::validate_subject(*subject)
            .map_err(|_| Error::InvalidSubject(*subject))?;
    }
    let available = keys::Available::new(foundation, meter)?;
    let mut pending = required.clone();
    let mut keys = BTreeMap::new();
    while let Some(subject) = pending.pop_first() {
        query(pending.len(), meter, &path)?;
        query(keys.len(), meter, &path)?;
        if keys.contains_key(&subject) {
            continue;
        }
        let key = available.get(subject, meter, &path)?;
        if key.origin() != foundation.source().entries().provider {
            return Err(Error::ForeignKey(subject));
        }
        query(source.records().len(), meter, &path)?;
        if source.get(subject).is_none() {
            return Err(Error::MissingRecord(subject));
        }
        let count = key.owners().owners().len() as u64;
        meter.check_semantic_depth(count + 1, &path)?;
        meter.charge_edges(count, &path)?;
        for atom in key.owners().owners() {
            let owner = match atom {
                DefinitionOwnerAtom::Type(id) => Subject::Type(*id),
                DefinitionOwnerAtom::GenericType(id) => Subject::GenericType(*id),
                _ => return Err(Error::NonNominalOwner(subject)),
            };
            query(keys.len(), meter, &path)?;
            query(pending.len(), meter, &path)?;
            if !keys.contains_key(&owner) && !pending.contains(&owner) {
                meter.check_table_entries(pending.len() as u64 + 1, &path)?;
                meter.charge_collection_slots(1, &path)?;
                query(pending.len(), meter, &path)?;
                pending.insert(owner);
            }
        }
        meter.check_table_entries(keys.len() as u64 + 1, &path)?;
        meter.charge_collection_slots(1, &path)?;
        meter.charge_nodes(1, &path)?;
        query(keys.len(), meter, &path)?;
        keys.insert(subject, key);
    }
    for record in source.records() {
        query(keys.len(), meter, &path)?;
        if !keys.contains_key(&record.subject()) {
            return Err(Error::UnexpectedRecord(record.subject()));
        }
    }
    Ok(keys)
}
