use super::*;
use scoop_identity::CallableTemplateOrigin;

pub(super) fn validate(
    dispatch: &BoundInheritanceDispatchSourcesV1<'_, '_>,
    properties: &CanonicalInheritanceSourcePropertiesV1,
    meter: &mut BudgetMeter,
) -> Result<(), InheritancePropertyBindingError> {
    use InheritancePropertyBindingError as Error;
    let mut required = BTreeSet::new();
    for source in dispatch.inventory().records() {
        meter.charge_work(1, &WirePath::root())?;
        for member in source.protected_members().values() {
            meter.charge_work(1, &WirePath::root())?;
            let (property, visibility) = match member {
                ProtectedDeclarationRefV1::Property(id) => {
                    query(properties.records().len(), meter)?;
                    let record = properties.get(*id).ok_or(Error::MissingSource(*id))?;
                    (*id, record.declaration_access().declared_visibility())
                }
                ProtectedDeclarationRefV1::Callable(callable) => {
                    let CallableTemplateOrigin::Accessor(id) = callable.declaration() else {
                        continue;
                    };
                    accessor(dispatch, properties, id, meter)?
                }
                ProtectedDeclarationRefV1::Constructor(_)
                | ProtectedDeclarationRefV1::NestedNominal(_) => continue,
            };
            query(properties.records().len(), meter)?;
            let record = properties
                .get(property)
                .ok_or(Error::MissingSource(property))?;
            if exact_owner(record.owner(), meter)? != source.owner() {
                return Err(Error::Owner(property));
            }
            if visibility != DeclaredVisibilityV1::Protected {
                return Err(Error::Visibility(property));
            }
            insert(&mut required, property, meter)?;
        }
    }
    for callable in dispatch.callables.records() {
        meter.charge_work(1, &WirePath::root())?;
        let id = match callable.declaration() {
            InheritanceCallableDeclarationV1::Function(_) => continue,
            InheritanceCallableDeclarationV1::Getter(id)
            | InheritanceCallableDeclarationV1::Setter(id) => id,
        };
        let (property, _) = accessor(dispatch, properties, id, meter)?;
        insert(&mut required, property, meter)?;
    }
    meter.charge_work(properties.records().len() as u64, &WirePath::root())?;
    if !required
        .iter()
        .copied()
        .eq(properties.records().iter().map(|r| r.declaration()))
    {
        return Err(Error::Inventory);
    }
    Ok(())
}

fn accessor(
    dispatch: &BoundInheritanceDispatchSourcesV1<'_, '_>,
    properties: &CanonicalInheritanceSourcePropertiesV1,
    id: PersistentPropertyAccessorId,
    meter: &mut BudgetMeter,
) -> Result<(PersistentPropertyId, DeclaredVisibilityV1), InheritancePropertyBindingError> {
    use InheritancePropertyBindingError as Error;
    query(
        dispatch
            .foundation
            .source()
            .entries()
            .accessor_keys
            .values()
            .len(),
        meter,
    )?;
    let key = dispatch.accessor_key(id)?;
    let PropertyOwner::Property(property) = key.owner() else {
        return Err(Error::Accessor(id));
    };
    query(properties.records().len(), meter)?;
    let record = properties
        .get(property)
        .ok_or(Error::MissingSource(property))?;
    let payload = payload(record)?;
    let visibility = match key.role() {
        AccessorRole::Getter if payload.getter() == id => {
            record.declaration_access().declared_visibility()
        }
        AccessorRole::Setter => {
            let ProtectedPropertyMutabilityV1::ReadWrite {
                setter,
                setter_access,
            } = payload.mutability()
            else {
                return Err(Error::Accessor(id));
            };
            if *setter != id {
                return Err(Error::Accessor(id));
            }
            setter_access.declared_visibility()
        }
        AccessorRole::Getter => return Err(Error::Accessor(id)),
    };
    Ok((property, visibility))
}

fn insert(
    required: &mut BTreeSet<PersistentPropertyId>,
    property: PersistentPropertyId,
    meter: &mut BudgetMeter,
) -> Result<(), InheritancePropertyBindingError> {
    query(required.len(), meter)?;
    if !required.contains(&property) {
        meter.check_table_entries(required.len() as u64 + 1, &WirePath::root())?;
        meter.charge_collection_slots(1, &WirePath::root())?;
        query(required.len(), meter)?;
        required.insert(property);
    }
    Ok(())
}
