use super::super::foundation::binding::sources::AccessAuthority;
use super::*;
use scoop_identity::DefinitionOriginSubject;

pub(super) fn validate(
    foundation: &BoundTypeFoundationSourcesV1<'_>,
    key: &SourceDeclarationKey,
    record: &NominalSupportConstructorInterfaceV1,
) -> Result<(), Error> {
    let declaration = record.declaration();
    let access = record.declaration_access();
    let origin = access.definition_origin();
    let entries = foundation.source().entries();

    if key.origin() != entries.provider {
        return Err(invalid(
            declaration,
            "constructor belongs to another provider",
        ));
    }

    if foundation
        .foundation
        .definition_origin(DefinitionOriginSubject::Constructor(declaration))
        .map(|record| record.origin())
        != Some(origin.origin())
    {
        return Err(Error::Origin(declaration));
    }

    access
        .validate_for_declaration(key, &mut AccessAuthority(foundation))
        .map_err(|error| invalid(declaration, error))?;
    Ok(())
}
