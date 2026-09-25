use super::*;

pub(super) type Members = BTreeMap<SourceNominalId, BTreeSet<ProtectedDeclarationRefV1>>;
pub(super) fn collect(
    bound: &BoundNominalDispatchSourcesV1<'_, '_, '_, '_, '_>,
) -> Result<Members, Error> {
    let sources = bound.parameters.members();

    let mut members = BTreeMap::new();
    for source in sources.callables().records() {
        if source.declaration_access().declared_visibility() == DeclaredVisibilityV1::Protected {
            let reference = ProtectedCallableDeclarationRefV1::try_new(source.declaration())
                .map_err(|_| Error::ProtectedOwner)?;
            insert(
                &mut members,
                source.payload().owner(),
                ProtectedDeclarationRefV1::Callable(reference),
            )?;
        }
    }
    for source in sources.properties().records() {
        if source.declaration_access().declared_visibility() == DeclaredVisibilityV1::Protected {
            insert(
                &mut members,
                source.owner(),
                ProtectedDeclarationRefV1::Property(source.declaration()),
            )?;
        }
    }
    for nominal in sources.nominals.table().records() {
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
            )?;
        }
    }
    Ok(members)
}
fn insert(
    members: &mut Members,
    owner: SourceNominalId,
    declaration: ProtectedDeclarationRefV1,
) -> Result<(), Error> {
    let members = match members.entry(owner) {
        std::collections::btree_map::Entry::Occupied(entry) => entry.into_mut(),
        std::collections::btree_map::Entry::Vacant(entry) => entry.insert(BTreeSet::new()),
    };

    if !members.insert(declaration) {
        return Err(Error::ProtectedOwner);
    }
    Ok(())
}
