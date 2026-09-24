use scoop_identity::{DefinitionOriginSubject, DefinitionOwnerAtom, SourceDeclarationKey};
use scoop_wire::WirePath;

use super::*;
use crate::{
    DeclarationAccessSourceV1, DeclaredVisibilityV1, ExportDefinitionSourceV1,
    NominalInterfaceRecordV1, SourceNominalId,
};

pub(crate) fn declaration_access(
    metadata: SharedTypeMetadataV1<'_>,
    declaration: &NominalInterfaceRecordV1,
    key: &SourceDeclarationKey,
    meter: &mut BudgetMeter,
) -> Result<DeclarationAccessSourceV1, Error> {
    let owner = declaration.declaration();
    let subject = match owner {
        SourceNominalId::Concrete(owner) => DefinitionOriginSubject::Type(owner),
        SourceNominalId::GenericTemplate(owner) => DefinitionOriginSubject::GenericType(owner),
    };
    if key.origin() != metadata.provider
        || SourceNominalId::from_source_declaration(key).ok() != Some(owner)
    {
        return Err(Error::InheritanceSource(owner));
    }
    source_access(
        metadata,
        subject,
        key,
        declaration.declaration_details().declared_visibility(),
        meter,
    )
}

pub(super) fn source_access(
    metadata: SharedTypeMetadataV1<'_>,
    subject: DefinitionOriginSubject,
    key: &SourceDeclarationKey,
    visibility: DeclaredVisibilityV1,
    meter: &mut BudgetMeter,
) -> Result<DeclarationAccessSourceV1, Error> {
    if key.origin() != metadata.provider {
        return Err(Error::DeclarationMetadata(subject));
    }
    let path = WirePath::root();
    let mut lexical = Vec::new();
    meter.try_reserve_collection_slots(&mut lexical, key.owners().owners().len(), &path)?;
    for parent in key.owners().owners() {
        lexical.push(match parent {
            DefinitionOwnerAtom::Type(owner) => SourceNominalId::Concrete(*owner),
            DefinitionOwnerAtom::GenericType(owner) => SourceNominalId::GenericTemplate(*owner),
            _ => return Err(Error::DeclarationMetadata(subject)),
        });
    }
    let origin = metadata
        .foundation
        .definition_origin(subject)
        .ok_or(Error::DeclarationMetadata(subject))?;
    let length =
        scoop_wire::encoded_length(origin).map_err(|error| Error::Key(error.to_string()))?;
    meter.charge_work(length, &path)?;
    meter.charge_owned_bytes(length, &path)?;
    let source = ExportDefinitionSourceV1::new(origin.origin().clone());
    metadata
        .foundation
        .validate_definition_source_location(metadata.provider, &source, meter, &path)
        .map_err(|_| Error::DeclarationMetadata(subject))?;
    DeclarationAccessSourceV1::try_new(visibility, lexical, source)
        .map_err(|_| Error::DeclarationMetadata(subject))
}
