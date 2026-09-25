use super::*;

pub(super) fn validate(
    properties: &BoundInheritancePropertySourcesV1<'_, '_>,
    callables: &CanonicalInheritanceSourceProtectedCallablesV1,
) -> Result<(), InheritanceProtectedCallableBindingError> {
    use InheritanceProtectedCallableBindingError as Error;
    let mut required = BTreeMap::new();
    for source in properties.inventory.records() {
        for member in source.protected_members().values() {
            let ProtectedDeclarationRefV1::Callable(declaration) = member else {
                continue;
            };
            let declaration = declaration.declaration();

            if required.insert(declaration, source.owner()).is_some() {
                return Err(Error::RepeatedOwner(declaration));
            }
        }
    }

    if !required.keys().copied().eq(callables
        .records()
        .iter()
        .map(ProtectedCallableInterfaceV1::declaration))
    {
        return Err(Error::Inventory);
    }
    for record in callables.records() {
        let owner = required[&record.declaration()];

        let ExactTypeKey::Nominal(owner) = properties.foundation.exact_type_key(owner)? else {
            return Err(Error::Owner(record.declaration()));
        };
        if record.payload().owner() != SourceNominalId::Concrete(*owner) {
            return Err(Error::Owner(record.declaration()));
        }
    }
    Ok(())
}
