use scoop_identity::PersistentGenericTypeId;

use super::*;
use crate::NominalRepresentationShapeV1;

pub(super) fn collect<'a>(
    context: &mut Context<'a>,
    provider: CheckedSharedTypeFoundationV1<'a>,
) -> Result<(), Error> {
    let metadata = provider.metadata;

    for declaration in metadata.public.nominal_interfaces().all_records() {
        let owner = declaration.declaration();

        let key = match owner {
            SourceNominalId::Concrete(owner) => metadata
                .identities
                .canonical_key::<PersistentTypeId, SourceDeclarationKey>(owner)?,
            SourceNominalId::GenericTemplate(owner) => {
                metadata
                    .identities
                    .canonical_key::<PersistentGenericTypeId, SourceDeclarationKey>(owner)?
            }
        };
        let access = declaration_access(metadata, declaration, &key)?;

        context.origins.insert(access.definition_origin().clone());
        if context
            .sources
            .insert(owner, Source { key, access })
            .is_some()
        {
            return Err(Error::InheritanceSource(owner));
        }
    }
    for representation in provider.representations.table().records() {
        if let NominalRepresentationShapeV1::Object { backing_class, .. } = representation.shape() {
            let types = MetadataTypes {
                current: metadata,
                dependencies: &[],
            };
            let exact = types.nominal_exact(*backing_class)?;
            let key = types.key(exact)?;
            let generated = metadata
                .identities
                .canonical_key::<PersistentTypeId, GeneratedNominalKey>(*backing_class)?;

            context.exacts.insert(exact, key);
            context.generated.insert(*backing_class, generated);
            context
                .objects
                .insert(representation.owner(), representation);
        }
    }
    Ok(())
}
