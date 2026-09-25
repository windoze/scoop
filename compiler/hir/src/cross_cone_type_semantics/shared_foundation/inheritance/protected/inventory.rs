use super::*;
use crate::{CanonicalProtectedDeclarationRefsV1, ProtectedCallableDeclarationRefV1};

pub(super) fn validate(provider: CheckedSharedTypeFoundationV1<'_>) -> Result<(), Error> {
    let metadata = provider.metadata;
    let mut required = Vec::new();
    let path = WirePath::root();
    for source in metadata.public.callable_interfaces().all_declarations() {
        if source.declared_visibility() != DeclaredVisibilityV1::Protected {
            continue;
        }
        let reference = match source.declaration() {
            CallableTemplateOrigin::Constructor(id) => ProtectedDeclarationRefV1::Constructor(id),
            declaration => ProtectedDeclarationRefV1::Callable(
                ProtectedCallableDeclarationRefV1::try_new(declaration)
                    .map_err(|_| Error::CallableContract(declaration))?,
            ),
        };
        scoop_wire::allocation::try_reserve(&mut required, 1, &path)?;
        required.push(reference);
    }
    for source in metadata.public.property_interfaces().all_declarations() {
        if source.declared_visibility() != DeclaredVisibilityV1::Protected {
            continue;
        }
        let PropertyOwner::Property(id) = source.declaration() else {
            return Err(Error::ProtectedInventory(metadata.provider));
        };
        scoop_wire::allocation::try_reserve(&mut required, 1, &path)?;
        required.push(ProtectedDeclarationRefV1::Property(id));
    }
    for source in metadata.public.nominal_interfaces().all_records() {
        if source.declaration_details().declared_visibility() == DeclaredVisibilityV1::Protected {
            scoop_wire::allocation::try_reserve(&mut required, 1, &path)?;
            required.push(ProtectedDeclarationRefV1::NestedNominal(
                source.declaration(),
            ));
        }
    }

    let required = CanonicalProtectedDeclarationRefsV1::try_new(required)
        .map_err(|_| Error::ProtectedInventory(metadata.provider))?;
    let table = provider.section.protected_declarations();
    for reference in required.values() {
        if table.get(*reference).is_none() {
            return Err(Error::ProtectedMember(*reference));
        }
    }
    if required.values().len() != table.records().len() {
        return Err(Error::ProtectedInventory(metadata.provider));
    }
    Ok(())
}
