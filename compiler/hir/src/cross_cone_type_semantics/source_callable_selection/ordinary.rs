use super::*;
use crate::{CallableInterfaceRecordV1, NominalExactLeafClassifierV1};
use scoop_identity::PropertyAccessorKey;

/// Replays the established ordinary partition without granting type layouts.
pub fn select_ordinary_source_callables<'a>(
    provider: ConeIdentity,
    public: &'a CrossConeHirInterfaceSectionV1,
    classifier: &NominalExactLeafClassifierV1,
    identities: &ValidatedIdentityGraph,
) -> Result<BTreeMap<Declaration, &'a CallableInterfaceRecordV1>, Error> {
    let mut required = BTreeMap::new();
    for source in public.callable_interfaces().records() {
        let Some(classified) =
            classifier
                .classify_callable(source)
                .map_err(|error| match error {
                    crate::NominalCallableClassificationError::Resource(error) => {
                        Error::Resource(error)
                    }
                    error => Error::CallableClassification(error),
                })?
        else {
            continue;
        };
        if !identity::is_local(provider, identities, source.declaration())?
            || !requires_body(public, identities, source.declaration())?
        {
            continue;
        }

        required.insert(classified.declaration(), source);
    }
    Ok(required)
}

fn requires_body(
    public: &CrossConeHirInterfaceSectionV1,
    identities: &ValidatedIdentityGraph,
    declaration: Origin,
) -> Result<bool, Error> {
    let Origin::Accessor(accessor) = declaration else {
        return Ok(true);
    };

    let key = identities.canonical_key::<_, PropertyAccessorKey>(accessor)?;

    let property = public
        .property_interfaces()
        .declaration(key.owner())
        .ok_or(Error::CallableContract(declaration))?;
    std::iter::once(property.accessors().getter_source())
        .chain(property.accessors().setter_source())
        .find(|source| source.accessor() == accessor)
        .map(|source| source.implementation().requires_body())
        .ok_or(Error::CallableContract(declaration))
}
