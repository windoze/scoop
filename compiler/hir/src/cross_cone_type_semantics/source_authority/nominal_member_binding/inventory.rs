use super::*;

pub(super) fn validate(
    nominals: &BoundNominalSourceContractsV1<'_, '_>,
    properties: &CanonicalNominalSourcePropertiesV1,
    callables: &CanonicalNominalSourceCallablesV1,
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    let mut required_properties = BTreeMap::new();
    let mut required_callables = BTreeMap::new();
    for nominal in nominals.table().records() {
        meter.charge_work(1, &WirePath::root())?;
        for member in nominal.members().values() {
            match member {
                NestedSourceMemberRefV1::Property(id) => {
                    insert(&mut required_properties, *id, nominal.owner(), meter)?
                }
                NestedSourceMemberRefV1::Function(id) => insert(
                    &mut required_callables,
                    CallableTemplateOrigin::Function(*id),
                    nominal.owner(),
                    meter,
                )?,
                NestedSourceMemberRefV1::GenericFunction(id) => insert(
                    &mut required_callables,
                    CallableTemplateOrigin::GenericFunction(*id),
                    nominal.owner(),
                    meter,
                )?,
            }
        }
        if let NominalSourceShapeV1::Enum(shape) = nominal.source_shape() {
            for variant in shape.variants() {
                insert(
                    &mut required_callables,
                    CallableTemplateOrigin::VariantConstructor(variant.variant()),
                    nominal.owner(),
                    meter,
                )?;
            }
        }
    }
    meter.charge_work(properties.records().len() as u64, &WirePath::root())?;
    if !required_properties.keys().copied().eq(properties
        .records()
        .iter()
        .map(NominalSupportPropertyInterfaceV1::declaration))
    {
        return Err(Error::Inventory("properties"));
    }
    for record in properties.records() {
        query(required_properties.len(), meter)?;
        if record.owner() != required_properties[&record.declaration()] {
            return Err(Error::Inventory("property owners"));
        }
        if let NominalSupportPropertyPayloadV1::Runtime { interface } = record.payload() {
            insert(
                &mut required_callables,
                CallableTemplateOrigin::Accessor(interface.getter()),
                record.owner(),
                meter,
            )?;
            if let ProtectedPropertyMutabilityV1::ReadWrite { setter, .. } = interface.mutability()
            {
                insert(
                    &mut required_callables,
                    CallableTemplateOrigin::Accessor(*setter),
                    record.owner(),
                    meter,
                )?;
            }
        }
    }
    meter.charge_work(callables.records().len() as u64, &WirePath::root())?;
    if !required_callables.keys().copied().eq(callables
        .records()
        .iter()
        .map(NominalSupportCallableInterfaceV1::declaration))
    {
        return Err(Error::Inventory("callables"));
    }
    for record in callables.records() {
        query(required_callables.len(), meter)?;
        if record.payload().owner() != required_callables[&record.declaration()] {
            return Err(Error::Inventory("callable owners"));
        }
    }
    Ok(())
}
fn insert<K: Ord>(
    map: &mut BTreeMap<K, SourceNominalId>,
    key: K,
    owner: SourceNominalId,
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    let path = WirePath::root();
    query(map.len(), meter)?;
    meter.check_table_entries(map.len() as u64 + 1, &path)?;
    meter.charge_collection_slots(1, &path)?;
    if map.insert(key, owner).is_some() {
        return Err(Error::Inventory("repeated member owner"));
    }
    Ok(())
}
