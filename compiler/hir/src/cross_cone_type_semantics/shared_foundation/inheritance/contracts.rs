//! Joins transported source signatures to the ordinary declaration records.

use super::*;
use crate::{CallableDeclarationRecordV1, NominalSourceCallablePayloadV1};
use scoop_identity::{
    CallableTemplateOrigin, DefinitionOriginSubject, PropertyAccessorKey, PropertyOwner,
};
use scoop_wire::WireEncode;

mod variants;

pub(super) fn callable<'a>(
    metadata: SharedTypeMetadataV1<'a>,
    declaration: CallableTemplateOrigin,
    meter: &mut BudgetMeter,
) -> Result<&'a CallableDeclarationRecordV1, Error> {
    lookup(
        metadata.public.callable_interfaces().declaration_count(),
        meter,
    )?;
    metadata
        .public
        .callable_interfaces()
        .declaration(declaration)
        .ok_or(Error::CallableContract(declaration))
}

pub(super) fn validate_callable(
    metadata: SharedTypeMetadataV1<'_>,
    declaration: CallableTemplateOrigin,
    access: &DeclarationAccessSourceV1,
    payload: &NominalSourceCallablePayloadV1,
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    let source = callable(metadata, declaration, meter)?;
    charge_compare(source, payload, meter)?;
    let expected_access = callable_access(metadata, source, meter)?;
    charge_compare(access, &expected_access, meter)?;
    if source.owner().nominal_owner() != Some(payload.owner())
        || source.receiver().is_some()
        || source.type_parameters() != payload.type_parameters()
        || source.parameters() != payload.parameters()
        || source.result() != payload.result()
        || source.effects() != payload.effects()
        || source.modality() != payload.modality()
        || source.slot_relations().values() != payload.slot_relations().slots()
        || access != &expected_access
    {
        return Err(Error::CallableContract(declaration));
    }
    Ok(())
}

pub(super) fn callable_access(
    metadata: SharedTypeMetadataV1<'_>,
    source: &CallableDeclarationRecordV1,
    meter: &mut BudgetMeter,
) -> Result<DeclarationAccessSourceV1, Error> {
    use CallableTemplateOrigin::*;
    lookup(metadata.identities.identity_count(), meter)?;
    let identities = metadata.identities;
    let (subject, key) = match source.declaration() {
        Function(id) => (
            DefinitionOriginSubject::Function(id),
            identities.canonical_key::<_, SourceDeclarationKey>(id)?,
        ),
        GenericFunction(id) => (
            DefinitionOriginSubject::GenericFunction(id),
            identities.canonical_key::<_, SourceDeclarationKey>(id)?,
        ),
        Constructor(id) => (
            DefinitionOriginSubject::Constructor(id),
            identities.canonical_key::<_, SourceDeclarationKey>(id)?,
        ),
        Accessor(id) => {
            let key = identities.canonical_key::<_, PropertyAccessorKey>(id)?;
            let PropertyOwner::Property(property) = key.owner() else {
                return Err(Error::CallableContract(source.declaration()));
            };
            lookup(identities.identity_count(), meter)?;
            (
                DefinitionOriginSubject::PropertyAccessor(id),
                identities.canonical_key::<_, SourceDeclarationKey>(property)?,
            )
        }
        VariantConstructor(id) => return variants::access(metadata, source, id, meter),
    };
    super::super::sources::source_access(
        metadata,
        subject,
        &key,
        source.declared_visibility(),
        meter,
    )
}

pub(super) fn lookup(length: usize, meter: &mut BudgetMeter) -> Result<(), Error> {
    Ok(meter.charge_work(1 + u64::from(length.max(1).ilog2()), &WirePath::root())?)
}

pub(super) fn charge_compare(
    left: &impl WireEncode,
    right: &impl WireEncode,
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    fn length(value: &impl WireEncode) -> Result<u64, Error> {
        scoop_wire::encoded_length(value).map_err(|error| Error::Key(error.to_string()))
    }
    Ok(meter.charge_work(
        length(left)?.saturating_add(length(right)?),
        &WirePath::root(),
    )?)
}
