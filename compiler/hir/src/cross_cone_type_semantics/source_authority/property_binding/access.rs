use super::super::foundation::binding::sources::AccessAuthority;
use super::*;

pub(super) fn validate<'a>(
    foundation: &BoundTypeFoundationSourcesV1<'_>,
    declaration: PersistentPropertyId,
    key: &'a SourceDeclarationKey,
    subject: DefinitionOriginSubject,
    access: &'a DeclarationAccessSourceV1,
) -> Result<CheckedDeclarationAccessSourceV1<'a>, InheritancePropertyBindingError> {
    use InheritancePropertyBindingError as Error;

    let entries = foundation.source().entries();

    if key.origin() != entries.provider {
        return Err(Error::Owner(declaration));
    }

    if foundation
        .foundation
        .definition_origin(subject)
        .map(|record| record.origin())
        != Some(access.definition_origin().origin())
    {
        return Err(Error::DefinitionOrigin(subject));
    }
    access
        .validate_for_declaration(key, &mut AccessAuthority(foundation))
        .map_err(|error| Error::Access {
            declaration,
            reason: error.to_string(),
        })
}

pub(super) fn accessor(
    foundation: &BoundTypeFoundationSourcesV1<'_>,
    property: PersistentPropertyId,
    accessor: PersistentPropertyAccessorId,
    role: AccessorRole,
) -> Result<(), InheritancePropertyBindingError> {
    let key = foundation.accessor_key(accessor)?;
    if key.owner() != PropertyOwner::Property(property) || key.role() != role {
        return Err(InheritancePropertyBindingError::Accessor(accessor));
    }
    Ok(())
}
