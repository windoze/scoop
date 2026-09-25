use super::*;
use std::collections::BTreeSet;

pub(super) fn validate(
    members: &BoundNominalMemberSourcesV1<'_, '_, '_>,
    constructors: &BoundNominalConstructorSourcesV1<'_, '_, '_>,
    protocols: &CanonicalNominalSourceParameterProtocolsV1,
) -> Result<(), Error> {
    let mut required = BTreeSet::new();
    for constructor in constructors.table().records() {
        insert(
            &mut required,
            CallableTemplateOrigin::Constructor(constructor.declaration()),
        )?;
    }
    for callable in members.callables().records() {
        if !matches!(callable.declaration(), CallableTemplateOrigin::Accessor(_)) {
            insert(&mut required, callable.declaration())?;
        }
    }

    if !required.into_iter().eq(protocols
        .records()
        .iter()
        .map(NominalSourceParameterProtocolV1::owner))
    {
        return Err(Error::Inventory);
    }
    Ok(())
}
fn insert(
    required: &mut BTreeSet<CallableTemplateOrigin>,
    owner: CallableTemplateOrigin,
) -> Result<(), Error> {
    if !required.insert(owner) {
        return Err(Error::Inventory);
    }
    Ok(())
}
