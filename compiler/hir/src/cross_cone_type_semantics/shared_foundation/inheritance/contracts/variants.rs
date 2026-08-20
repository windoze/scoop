use super::*;
use scoop_identity::{DefinitionOwnerAtom, EnumVariantIdentityKey, PersistentEnumVariantId};

pub(super) fn access(
    metadata: SharedTypeMetadataV1<'_>,
    source: &CallableDeclarationRecordV1,
    variant: PersistentEnumVariantId,
) -> Result<DeclarationAccessSourceV1, Error> {
    let declaration = source.declaration();
    let key = metadata
        .identities
        .canonical_key::<_, EnumVariantIdentityKey>(variant)?;
    let owner = key
        .source_owner()
        .ok_or(Error::CallableContract(declaration))?;
    if source.owner().nominal_owner() != Some(owner)
        || source.declared_visibility() != crate::DeclaredVisibilityV1::Public
    {
        return Err(Error::CallableContract(declaration));
    }

    let owner_key = match owner {
        SourceNominalId::Concrete(id) => metadata
            .identities
            .canonical_key::<_, SourceDeclarationKey>(id)?,
        SourceNominalId::GenericTemplate(id) => metadata
            .identities
            .canonical_key::<_, SourceDeclarationKey>(id)?,
    };
    if owner_key.origin() != metadata.provider {
        return Err(Error::CallableContract(declaration));
    }
    let path = WirePath::root();
    let mut owners = Vec::new();
    scoop_wire::allocation::try_reserve(&mut owners, owner_key.owners().owners().len() + 1, &path)?;
    for parent in owner_key.owners().owners() {
        owners.push(match parent {
            DefinitionOwnerAtom::Type(id) => SourceNominalId::Concrete(*id),
            DefinitionOwnerAtom::GenericType(id) => SourceNominalId::GenericTemplate(*id),
            _ => return Err(Error::CallableContract(declaration)),
        });
    }
    owners.push(owner);
    let origin = super::super::super::sources::definition_source(
        metadata,
        DefinitionOriginSubject::EnumVariant(variant),
    )?;
    DeclarationAccessSourceV1::try_new(source.declared_visibility(), owners, origin)
        .map_err(|_| Error::CallableContract(declaration))
}
