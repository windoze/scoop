use scoop_identity::{DefinitionOriginSubject, DefinitionOwnerAtom, SourceDeclarationKey};
use scoop_wire::WirePath;

use super::*;
use crate::{
    DeclarationAccessSourceV1, DeclaredVisibilityV1, ExportDefinitionSourceV1, SourceNominalId,
};

pub(super) fn source_access(
    metadata: SharedTypeMetadataV1<'_>,
    subject: DefinitionOriginSubject,
    key: &SourceDeclarationKey,
    visibility: DeclaredVisibilityV1,
) -> Result<DeclarationAccessSourceV1, Error> {
    if key.origin() != metadata.provider {
        return Err(Error::DeclarationMetadata(subject));
    }
    let path = WirePath::root();
    let mut lexical = Vec::new();
    scoop_wire::allocation::try_reserve(&mut lexical, key.owners().owners().len(), &path)?;
    for parent in key.owners().owners() {
        lexical.push(match parent {
            DefinitionOwnerAtom::Type(owner) => SourceNominalId::Concrete(*owner),
            DefinitionOwnerAtom::GenericType(owner) => SourceNominalId::GenericTemplate(*owner),
            _ => return Err(Error::DeclarationMetadata(subject)),
        });
    }
    let source = definition_source(metadata, subject)?;
    DeclarationAccessSourceV1::try_new(visibility, lexical, source)
        .map_err(|_| Error::DeclarationMetadata(subject))
}

pub(super) fn definition_source(
    metadata: SharedTypeMetadataV1<'_>,
    subject: DefinitionOriginSubject,
) -> Result<ExportDefinitionSourceV1, Error> {
    let origin = metadata
        .foundation
        .definition_origin(subject)
        .ok_or(Error::DeclarationMetadata(subject))?;

    let source = ExportDefinitionSourceV1::new(origin.origin().clone());
    metadata
        .foundation
        .validate_definition_source_location(metadata.provider, &source)
        .map_err(|_| Error::DeclarationMetadata(subject))?;
    Ok(source)
}
