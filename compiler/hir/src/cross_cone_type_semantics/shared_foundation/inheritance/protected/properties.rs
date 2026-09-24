use super::*;
use crate::{
    NominalSupportPropertyInterfaceV1, NominalSupportPropertyPayloadV1,
    ProtectedPropertyMutabilityV1,
};

mod constants;

pub(super) fn validate(
    metadata: SharedTypeMetadataV1<'_>,
    id: PersistentPropertyId,
    access: &DeclarationAccessSourceV1,
    payload: &NominalSourcePropertyPayloadV1,
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    let source = property(metadata, id, meter)?;
    let expected_access = property_access(metadata, id, meter)?;
    contracts::charge_compare(access, &expected_access, meter)?;
    contracts::charge_compare(source, payload, meter)?;
    if source.owner().nominal_owner() != Some(payload.owner())
        || source.receiver().is_some()
        || !source.type_parameters().is_empty()
        || source.value_type() != payload.value_type()
        || source.representation() != payload.representation()
        || source.accessors().getter() != payload.getter()
        || access != &expected_access
    {
        return Err(Error::PropertyContract(id));
    }
    match (source.accessors().setter(), payload.mutability()) {
        (None, ProtectedPropertyMutabilityV1::ReadOnly) => {}
        (
            Some(expected),
            ProtectedPropertyMutabilityV1::ReadWrite {
                setter,
                setter_access,
            },
        ) if expected == *setter => {
            let declaration =
                contracts::callable(metadata, CallableTemplateOrigin::Accessor(expected), meter)?;
            let expected_access = contracts::callable_access(metadata, declaration, meter)?;
            contracts::charge_compare(setter_access, &expected_access, meter)?;
            if setter_access != &expected_access {
                return Err(Error::PropertyContract(id));
            }
        }
        _ => return Err(Error::PropertyContract(id)),
    }
    let mut slots = BTreeSet::new();
    for accessor in std::iter::once(source.accessors().getter()).chain(source.accessors().setter())
    {
        let declaration =
            contracts::callable(metadata, CallableTemplateOrigin::Accessor(accessor), meter)?;
        for slot in declaration.slot_relations().values() {
            contracts::lookup(slots.len(), meter)?;
            meter.charge_collection_slots(1, &WirePath::root())?;
            slots.insert(*slot);
        }
    }
    meter.charge_work(
        slots.len() as u64 + payload.slot_relations().slots().len() as u64,
        &WirePath::root(),
    )?;
    if !slots.iter().eq(payload.slot_relations().slots()) {
        return Err(Error::PropertyContract(id));
    }
    Ok(())
}

pub(super) fn support(
    types: MetadataTypes<'_, '_>,
    record: &NominalSupportPropertyInterfaceV1,
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    match record.payload() {
        NominalSupportPropertyPayloadV1::Runtime { interface } => validate(
            types.current,
            record.declaration(),
            record.declaration_access(),
            interface,
            meter,
        ),
        NominalSupportPropertyPayloadV1::Const { value } => {
            constants::validate(types, record, value, meter)
        }
    }
}
