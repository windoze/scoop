use super::*;
use scoop_identity::DefinitionOriginSubject;

pub(super) fn origin(
    bound: &BoundNominalMemberSourcesV1<'_, '_, '_>,
    subject: DefinitionOriginSubject,
    access: &DeclarationAccessSourceV1,
) -> Result<(), Error> {
    let foundation = bound.nominals.foundation;

    foundation.validate_origin(access.definition_origin())?;
    if foundation
        .foundation
        .definition_origin(subject)
        .map(|r| r.origin())
        != Some(access.definition_origin().origin())
    {
        return Err(Error::Origin(subject));
    }

    Ok(())
}
pub(super) fn key(
    bound: &BoundNominalMemberSourcesV1<'_, '_, '_>,
    key: &SourceDeclarationKey,
) -> Result<(), Error> {
    if key.origin() != bound.provider() {
        return Err(Error::Inventory("foreign member key"));
    }
    Ok(())
}
