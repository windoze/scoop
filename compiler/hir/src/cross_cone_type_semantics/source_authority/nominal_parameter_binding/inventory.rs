use super::*;
use std::collections::BTreeSet;

pub(super) fn validate(
    members: &BoundNominalMemberSourcesV1<'_, '_, '_>,
    constructors: &BoundNominalConstructorSourcesV1<'_, '_, '_>,
    protocols: &CanonicalNominalSourceParameterProtocolsV1,
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    let mut required = BTreeSet::new();
    for constructor in constructors.table().records() {
        insert(
            &mut required,
            CallableTemplateOrigin::Constructor(constructor.declaration()),
            meter,
        )?;
    }
    for callable in members.callables().records() {
        meter.charge_work(1, &WirePath::root())?;
        if !matches!(callable.declaration(), CallableTemplateOrigin::Accessor(_)) {
            insert(&mut required, callable.declaration(), meter)?;
        }
    }
    meter.check_table_entries(protocols.records().len() as u64, &WirePath::root())?;
    meter.charge_work(protocols.records().len() as u64, &WirePath::root())?;
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
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    query(required.len(), meter)?;
    meter.check_table_entries(required.len() as u64 + 1, &WirePath::root())?;
    meter.charge_collection_slots(1, &WirePath::root())?;
    if !required.insert(owner) {
        return Err(Error::Inventory);
    }
    Ok(())
}
