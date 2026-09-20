use super::*;

pub(super) type Members = BTreeMap<SourceNominalId, BTreeSet<ProtectedDeclarationRefV1>>;
pub(super) fn collect(
    bound: &BoundNominalDispatchSourcesV1<'_, '_, '_, '_, '_>,
    meter: &mut BudgetMeter,
) -> Result<Members, Error> {
    let sources = bound.parameters.members();
    let path = WirePath::root();
    let count = sources.callables().records().len() as u64
        + sources.properties().records().len() as u64
        + sources.nominals.table().records().len() as u64;
    meter.charge_work(count, &path)?;
    meter.charge_nodes(count, &path)?;
    let mut members = BTreeMap::new();
    for source in sources.callables().records() {
        if source.declaration_access().declared_visibility() == DeclaredVisibilityV1::Protected {
            let reference = ProtectedCallableDeclarationRefV1::try_new(source.declaration())
                .map_err(|_| Error::ProtectedOwner)?;
            insert(
                &mut members,
                source.payload().owner(),
                ProtectedDeclarationRefV1::Callable(reference),
                meter,
            )?;
        }
    }
    for source in sources.properties().records() {
        if source.declaration_access().declared_visibility() == DeclaredVisibilityV1::Protected {
            insert(
                &mut members,
                source.owner(),
                ProtectedDeclarationRefV1::Property(source.declaration()),
                meter,
            )?;
        }
    }
    for nominal in sources.nominals.table().records() {
        query(sources.nominals.table().records().len(), meter)?;
        let access = sources
            .nominals
            .foundation
            .nominal_source(nominal.owner())
            .map_err(NominalNestedBindingError::from)?
            .access();
        if access.declared_visibility() == DeclaredVisibilityV1::Protected {
            let owner = access
                .lexical_owners()
                .last()
                .copied()
                .ok_or(Error::ProtectedOwner)?;
            insert(
                &mut members,
                owner,
                ProtectedDeclarationRefV1::NestedNominal(nominal.owner()),
                meter,
            )?;
        }
    }
    Ok(members)
}
fn insert(
    members: &mut Members,
    owner: SourceNominalId,
    declaration: ProtectedDeclarationRefV1,
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    let path = WirePath::root();
    query(members.len(), meter)?;
    let length = members.len();
    let members = match members.entry(owner) {
        std::collections::btree_map::Entry::Occupied(entry) => entry.into_mut(),
        std::collections::btree_map::Entry::Vacant(entry) => {
            meter.check_table_entries(length as u64 + 1, &path)?;
            meter.charge_collection_slots(1, &path)?;
            entry.insert(BTreeSet::new())
        }
    };
    query(members.len(), meter)?;
    meter.check_table_entries(members.len() as u64 + 1, &path)?;
    meter.charge_collection_slots(1, &path)?;
    if !members.insert(declaration) {
        return Err(Error::ProtectedOwner);
    }
    Ok(())
}
