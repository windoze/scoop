use super::*;
use scoop_identity::PropertyOwner;

pub(super) fn bind<'f>(
    properties: &BoundInheritancePropertySourcesV1<'_, 'f>,
    callables: &CanonicalInheritanceSourceProtectedCallablesV1,
) -> Result<
    BTreeMap<CallableTemplateOrigin, &'f SourceDeclarationKey>,
    InheritanceProtectedCallableBindingError,
> {
    use InheritanceProtectedCallableBindingError as Error;
    let foundation = properties.foundation;
    let canonical = foundation.foundation.as_canonical();

    let functions = binding_keys::index(canonical.type_source_function_records())?;
    let generics = binding_keys::index(canonical.type_source_generic_function_records())?;

    let mut keys = BTreeMap::new();
    for record in callables.records() {
        let declaration = record.declaration();
        let key = match declaration {
            CallableTemplateOrigin::Function(id) => {
                let key = functions
                    .get(&id)
                    .copied()
                    .ok_or(Error::MissingKey(declaration))?;
                binding_keys::verify(id, key, foundation.identities)?;
                key
            }
            CallableTemplateOrigin::GenericFunction(id) => {
                let key = generics
                    .get(&id)
                    .copied()
                    .ok_or(Error::MissingKey(declaration))?;
                binding_keys::verify(id, key, foundation.identities)?;
                key
            }
            CallableTemplateOrigin::Accessor(id) => {
                let PropertyOwner::Property(property) = foundation.accessor_key(id)?.owner() else {
                    return Err(Error::Declaration(declaration));
                };

                properties.property_key(property)?
            }
            other => return Err(Error::Declaration(other)),
        };
        keys.insert(declaration, key);
    }
    Ok(keys)
}
