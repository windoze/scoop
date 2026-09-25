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
) -> Result<(), Error> {
    let source = property(metadata, id)?;
    let expected_access = property_access(metadata, id)?;

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
                contracts::callable(metadata, CallableTemplateOrigin::Accessor(expected))?;
            let expected_access = contracts::callable_access(metadata, declaration)?;

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
            contracts::callable(metadata, CallableTemplateOrigin::Accessor(accessor))?;
        for slot in declaration.slot_relations().values() {
            slots.insert(*slot);
        }
    }

    if !slots.iter().eq(payload.slot_relations().slots()) {
        return Err(Error::PropertyContract(id));
    }
    Ok(())
}

pub(super) fn support(
    types: MetadataTypes<'_, '_>,
    record: &NominalSupportPropertyInterfaceV1,
) -> Result<(), Error> {
    match record.payload() {
        NominalSupportPropertyPayloadV1::Runtime { interface } => validate(
            types.current,
            record.declaration(),
            record.declaration_access(),
            interface,
        ),
        NominalSupportPropertyPayloadV1::Const { value } => {
            constants::validate(types, record, value)
        }
    }
}
