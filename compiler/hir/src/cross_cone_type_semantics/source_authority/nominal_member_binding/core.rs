use super::*;
use scoop_identity::PersistentTypeId;

pub(super) fn validate(
    bound: &BoundNominalMemberSourcesV1<'_, '_, '_>,
    id: PersistentTypeId,
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    let key = bound
        .nominals
        .foundation
        .identities
        .canonical_key::<PersistentTypeId, SourceDeclarationKey>(id)
        .map_err(|error| Error::Identity(error.to_string()))?;
    NominalRepresentationSupportV1::charge_source_key_resources(&key, meter, &WirePath::root())?;
    if key.origin() != ConeIdentity::CORE {
        return Err(Error::Identity(
            "fundamental role is not owned by core".into(),
        ));
    }
    Ok(())
}
