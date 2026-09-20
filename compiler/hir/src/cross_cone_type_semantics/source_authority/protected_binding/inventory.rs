use super::*;

pub(super) fn validate(
    properties: &BoundInheritancePropertySourcesV1<'_, '_>,
    callables: &CanonicalInheritanceSourceProtectedCallablesV1,
    meter: &mut BudgetMeter,
) -> Result<(), InheritanceProtectedCallableBindingError> {
    use InheritanceProtectedCallableBindingError as Error;
    let mut required = BTreeMap::new();
    for source in properties.inventory.records() {
        meter.charge_work(1, &WirePath::root())?;
        for member in source.protected_members().values() {
            meter.charge_work(1, &WirePath::root())?;
            let ProtectedDeclarationRefV1::Callable(declaration) = member else {
                continue;
            };
            let declaration = declaration.declaration();
            query(required.len(), meter)?;
            meter.check_table_entries(required.len() as u64 + 1, &WirePath::root())?;
            meter.charge_collection_slots(1, &WirePath::root())?;
            if required.insert(declaration, source.owner()).is_some() {
                return Err(Error::RepeatedOwner(declaration));
            }
        }
    }
    meter.charge_work(callables.records().len() as u64, &WirePath::root())?;
    if !required.keys().copied().eq(callables
        .records()
        .iter()
        .map(ProtectedCallableInterfaceV1::declaration))
    {
        return Err(Error::Inventory);
    }
    for record in callables.records() {
        query(required.len(), meter)?;
        let owner = required[&record.declaration()];
        query(
            properties
                .foundation
                .source()
                .entries()
                .exact_keys
                .values()
                .len(),
            meter,
        )?;
        let ExactTypeKey::Nominal(owner) = properties.foundation.exact_type_key(owner)? else {
            return Err(Error::Owner(record.declaration()));
        };
        if record.payload().owner() != SourceNominalId::Concrete(*owner) {
            return Err(Error::Owner(record.declaration()));
        }
    }
    Ok(())
}
