use super::*;
use std::collections::BTreeSet;

pub(super) fn validate(
    callables: &BoundInheritanceProtectedCallableSourcesV1<'_, '_>,
    constructors: &BoundInheritanceConstructorSourcesV1<'_, '_>,
    protocols: &CanonicalInheritanceSourceParameterProtocolsV1,
    meter: &mut BudgetMeter,
) -> Result<(), InheritanceParameterBindingError> {
    use InheritanceParameterBindingError as Error;
    super::super::binding_keys::charge_inheritance_inventory(
        callables.inventory,
        meter,
        &WirePath::root(),
    )?;
    super::super::binding_keys::charge_inheritance_inventory(
        constructors.inventory,
        meter,
        &WirePath::root(),
    )?;
    if callables.inventory != constructors.inventory {
        return Err(Error::Inventory);
    }
    let mut required = BTreeSet::new();
    for constructor in constructors.table().records() {
        insert(
            &mut required,
            CallableTemplateOrigin::Constructor(constructor.declaration()),
            meter,
        )?;
    }
    for callable in callables.table().records() {
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
        .map(InheritanceSourceParameterProtocolV1::owner))
    {
        return Err(Error::Inventory);
    }
    Ok(())
}
fn insert(
    required: &mut BTreeSet<CallableTemplateOrigin>,
    declaration: CallableTemplateOrigin,
    meter: &mut BudgetMeter,
) -> Result<(), InheritanceParameterBindingError> {
    query(required.len(), meter)?;
    meter.check_table_entries(required.len() as u64 + 1, &WirePath::root())?;
    meter.charge_collection_slots(1, &WirePath::root())?;
    if !required.insert(declaration) {
        return Err(InheritanceParameterBindingError::Inventory);
    }
    Ok(())
}
