use super::*;
use scoop_identity::{AccessorRole, PropertyOwner};

pub(super) fn validate(
    bound: &BoundInheritanceProtectedCallableSourcesV1<'_, '_>,
    record: &ProtectedCallableInterfaceV1,
) -> Result<(), InheritanceProtectedCallableBindingError> {
    use InheritanceProtectedCallableBindingError as Error;

    let key = bound.callable_key(record.declaration())?;

    let entries = bound.foundation.source().entries();
    if key.origin() != entries.provider {
        return Err(Error::Owner(record.declaration()));
    }
    let access = record.declaration_access();

    let subject = subject(record.declaration())?;
    if bound
        .foundation
        .foundation
        .definition_origin(subject)
        .map(|r| r.origin())
        != Some(access.definition_origin().origin())
    {
        return Err(Error::DefinitionOrigin(subject));
    }
    if let CallableTemplateOrigin::Accessor(id) = record.declaration() {
        let accessor = bound.foundation.accessor_key(id)?;
        let PropertyOwner::Property(property) = accessor.owner() else {
            return Err(Error::Declaration(record.declaration()));
        };

        let property = bound
            .properties
            .get(property)
            .ok_or(Error::MissingProperty(property))?;
        let hir_access = match accessor.role() {
            AccessorRole::Getter => property.declaration_access(),
            AccessorRole::Setter => {
                let NominalSupportPropertyPayloadV1::Runtime { interface } = property.payload()
                else {
                    return Err(Error::MissingProperty(property.declaration()));
                };
                let ProtectedPropertyMutabilityV1::ReadWrite {
                    setter,
                    setter_access,
                } = interface.mutability()
                else {
                    return Err(Error::AccessorAccess(id));
                };
                if *setter != id {
                    return Err(Error::AccessorAccess(id));
                }
                setter_access
            }
        };
        if hir_access.declared_visibility() != access.declared_visibility()
            || hir_access.lexical_owners() != access.lexical_owners()
        {
            return Err(Error::AccessorAccess(id));
        }
    }
    Ok(())
}
