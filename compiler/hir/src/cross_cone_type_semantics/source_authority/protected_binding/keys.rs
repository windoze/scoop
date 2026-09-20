use super::*;
use scoop_identity::PropertyOwner;

pub(super) fn bind<'f>(
    properties: &BoundInheritancePropertySourcesV1<'_, 'f>,
    callables: &CanonicalInheritanceSourceProtectedCallablesV1,
    meter: &mut BudgetMeter,
) -> Result<
    BTreeMap<CallableTemplateOrigin, &'f SourceDeclarationKey>,
    InheritanceProtectedCallableBindingError,
> {
    use InheritanceProtectedCallableBindingError as Error;
    let foundation = properties.foundation;
    let canonical = foundation.foundation.as_canonical();
    let path = WirePath::root();
    let functions = binding_keys::index(canonical.type_source_function_records(), meter, &path)?;
    let generics = binding_keys::index(
        canonical.type_source_generic_function_records(),
        meter,
        &path,
    )?;
    binding_keys::charge_map(callables.records().len(), meter, &path)?;
    let mut keys = BTreeMap::new();
    for record in callables.records() {
        let declaration = record.declaration();
        let key = match declaration {
            CallableTemplateOrigin::Function(id) => {
                query(functions.len(), meter)?;
                let key = functions
                    .get(&id)
                    .copied()
                    .ok_or(Error::MissingKey(declaration))?;
                binding_keys::verify(id, key, foundation.identities, meter, &path)?;
                key
            }
            CallableTemplateOrigin::GenericFunction(id) => {
                query(generics.len(), meter)?;
                let key = generics
                    .get(&id)
                    .copied()
                    .ok_or(Error::MissingKey(declaration))?;
                binding_keys::verify(id, key, foundation.identities, meter, &path)?;
                key
            }
            CallableTemplateOrigin::Accessor(id) => {
                query(
                    foundation.source().entries().accessor_keys.values().len(),
                    meter,
                )?;
                let PropertyOwner::Property(property) = foundation.accessor_key(id)?.owner() else {
                    return Err(Error::Declaration(declaration));
                };
                query(properties.properties.records().len(), meter)?;
                properties.property_key(property)?
            }
            other => return Err(Error::Declaration(other)),
        };
        keys.insert(declaration, key);
    }
    Ok(keys)
}
