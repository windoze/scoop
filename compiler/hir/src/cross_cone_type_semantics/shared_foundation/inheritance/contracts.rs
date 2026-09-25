//! Joins transported source signatures to the ordinary declaration records.

use super::*;
use crate::{CallableDeclarationRecordV1, NominalSourceCallablePayloadV1};
use scoop_identity::{
    CallableTemplateOrigin, DefinitionOriginSubject, PropertyAccessorKey, PropertyOwner,
};

mod variants;

pub(super) fn callable<'a>(
    metadata: SharedTypeMetadataV1<'a>,
    declaration: CallableTemplateOrigin,
) -> Result<&'a CallableDeclarationRecordV1, Error> {
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
) -> Result<(), Error> {
    let source = callable(metadata, declaration)?;

    let expected_access = callable_access(metadata, source)?;

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
) -> Result<DeclarationAccessSourceV1, Error> {
    use CallableTemplateOrigin::*;

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

            (
                DefinitionOriginSubject::PropertyAccessor(id),
                identities.canonical_key::<_, SourceDeclarationKey>(property)?,
            )
        }
        VariantConstructor(id) => return variants::access(metadata, source, id),
    };
    super::super::sources::source_access(metadata, subject, &key, source.declared_visibility())
}
