use super::*;
use scoop_identity::DefinitionOriginSubject;

pub(super) fn origin(
    bound: &BoundNominalMemberSourcesV1<'_, '_, '_>,
    subject: DefinitionOriginSubject,
    access: &DeclarationAccessSourceV1,
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    let foundation = bound.nominals.foundation;
    query(
        foundation
            .foundation
            .as_canonical()
            .counts()
            .definition_origins,
        meter,
    )?;
    let path = WirePath::root();
    foundation.validate_origin(access.definition_origin(), meter, &path)?;
    if foundation
        .foundation
        .definition_origin(subject)
        .map(|r| r.origin())
        != Some(access.definition_origin().origin())
    {
        return Err(Error::Origin(subject));
    }
    for owner in access.lexical_owners() {
        query(foundation.source().entries().sources.records().len(), meter)?;
        NominalRepresentationSupportV1::charge_source_key_resources(
            foundation.nominal_key(*owner)?,
            meter,
            &path,
        )?;
    }
    Ok(())
}
pub(super) fn key(
    bound: &BoundNominalMemberSourcesV1<'_, '_, '_>,
    key: &SourceDeclarationKey,
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    NominalRepresentationSupportV1::charge_source_key_resources(key, meter, &WirePath::root())?;
    if key.origin() != bound.provider() {
        return Err(Error::Inventory("foreign member key"));
    }
    Ok(())
}
