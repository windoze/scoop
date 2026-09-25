use super::*;

pub(super) fn collect(
    authority: &BoundNominalParameterProtocolsV1<'_, '_, '_, '_>,
) -> Result<BTreeSet<ProtectedDeclarationRefV1>, Error> {
    let mut required = BTreeSet::new();
    for source in authority.members().callables().records() {
        if protected(source.declaration_access()) {
            let reference = ProtectedCallableDeclarationRefV1::try_new(source.declaration())
                .map_err(|_| Error::Inventory)?;
            insert(
                &mut required,
                ProtectedDeclarationRefV1::Callable(reference),
            )?;
        }
    }
    for source in authority.members().properties().records() {
        if protected(source.declaration_access()) {
            insert(
                &mut required,
                ProtectedDeclarationRefV1::Property(source.declaration()),
            )?;
        }
    }
    for source in authority.constructors().table().records() {
        if protected(source.declaration_access()) {
            insert(
                &mut required,
                ProtectedDeclarationRefV1::Constructor(source.declaration()),
            )?;
        }
    }
    let nominals = authority.members().nominals;
    for nominal in nominals.table().records() {
        let source = nominals
            .foundation
            .nominal_source(nominal.owner())
            .map_err(NominalNestedBindingError::from)?;
        if protected(source.access()) {
            insert(
                &mut required,
                ProtectedDeclarationRefV1::NestedNominal(nominal.owner()),
            )?;
        }
    }
    Ok(required)
}

pub(super) fn validate(
    authority: &BoundNominalParameterProtocolsV1<'_, '_, '_, '_>,
    table: &CanonicalProtectedDeclarationInterfacesV1,
) -> Result<(), Error> {
    let required = collect(authority)?;

    if !required.into_iter().eq(table
        .records()
        .iter()
        .map(ProtectedDeclarationInterfaceV1::reference))
    {
        return Err(Error::Inventory);
    }
    Ok(())
}

fn protected(access: &DeclarationAccessSourceV1) -> bool {
    access.declared_visibility() == DeclaredVisibilityV1::Protected
}
fn insert(
    required: &mut BTreeSet<ProtectedDeclarationRefV1>,
    reference: ProtectedDeclarationRefV1,
) -> Result<(), Error> {
    if !required.insert(reference) {
        return Err(Error::Inventory);
    }
    Ok(())
}
