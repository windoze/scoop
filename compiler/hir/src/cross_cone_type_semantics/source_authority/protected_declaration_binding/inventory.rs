use super::*;

pub(super) fn validate(
    authority: &BoundNominalParameterProtocolsV1<'_, '_, '_, '_>,
    table: &CanonicalProtectedDeclarationInterfacesV1,
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    let path = WirePath::root();
    meter.check_semantic_depth(1, &path)?;
    let members = authority.members();
    let visits = members.callables().records().len() as u64
        + members.properties().records().len() as u64
        + authority.constructors().table().records().len() as u64;
    meter.charge_work(visits, &path)?;
    let mut required = BTreeSet::new();
    for source in authority.members().callables().records() {
        meter.charge_nodes(1, &path)?;
        if protected(source.declaration_access()) {
            let reference = ProtectedCallableDeclarationRefV1::try_new(source.declaration())
                .map_err(|_| Error::Inventory)?;
            insert(
                &mut required,
                ProtectedDeclarationRefV1::Callable(reference),
                meter,
            )?;
        }
    }
    for source in authority.members().properties().records() {
        meter.charge_nodes(1, &path)?;
        if protected(source.declaration_access()) {
            insert(
                &mut required,
                ProtectedDeclarationRefV1::Property(source.declaration()),
                meter,
            )?;
        }
    }
    for source in authority.constructors().table().records() {
        meter.charge_nodes(1, &path)?;
        if protected(source.declaration_access()) {
            insert(
                &mut required,
                ProtectedDeclarationRefV1::Constructor(source.declaration()),
                meter,
            )?;
        }
    }
    let nominals = authority.members().nominals;
    for nominal in nominals.table().records() {
        meter.charge_nodes(1, &path)?;
        query(nominals.table().records().len(), meter)?;
        let source = nominals
            .foundation
            .nominal_source(nominal.owner())
            .map_err(NominalNestedBindingError::from)?;
        if protected(source.access()) {
            insert(
                &mut required,
                ProtectedDeclarationRefV1::NestedNominal(nominal.owner()),
                meter,
            )?;
        }
    }
    meter.charge_work(
        (required.len() as u64)
            .saturating_add(table.records().len() as u64)
            .saturating_mul(128),
        &path,
    )?;
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
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    let path = WirePath::root();
    query(required.len(), meter)?;
    meter.check_table_entries(required.len() as u64 + 1, &path)?;
    meter.charge_collection_slots(1, &path)?;
    if !required.insert(reference) {
        return Err(Error::Inventory);
    }
    Ok(())
}
