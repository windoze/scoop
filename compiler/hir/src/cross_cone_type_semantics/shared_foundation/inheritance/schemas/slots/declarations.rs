use super::*;
use scoop_identity::{AccessorRole, PropertyOwner};

pub(super) fn collect<'a>(
    data: &mut Data<'a>,
    schemas: &mut SchemaDeclarations<'_>,
    provider: CheckedSharedTypeFoundationV1<'a>,
) -> Result<(), Error> {
    let metadata = provider.metadata;
    for source in metadata.public.callable_interfaces().all_declarations() {
        let Some(_) = source.owner().nominal_owner() else {
            continue;
        };
        if source.declared_visibility() == DeclaredVisibilityV1::Private
            || !source.type_parameters().is_empty()
            || !matches!(
                source.declaration(),
                CallableTemplateOrigin::Function(_) | CallableTemplateOrigin::Accessor(_)
            )
        {
            continue;
        }
        let declaration = identity(schemas, metadata, source.declaration())?;
        let access = contracts::callable_access(metadata, source)?;

        data.origins.insert(access.definition_origin().clone());
        if data
            .members
            .insert(declaration, Member { source, access })
            .is_some()
        {
            return Err(Error::SlotCallable(declaration));
        }
    }
    Ok(())
}

fn identity(
    schemas: &mut SchemaDeclarations<'_>,
    metadata: SharedTypeMetadataV1<'_>,
    declaration: CallableTemplateOrigin,
) -> Result<Declaration, Error> {
    match declaration {
        CallableTemplateOrigin::Function(id) => {
            let key = metadata
                .identities
                .canonical_key::<_, SourceDeclarationKey>(id)?;

            schemas.functions.insert(id, key);
            Ok(Declaration::Function(id))
        }
        CallableTemplateOrigin::Accessor(id) => {
            let accessor = metadata
                .identities
                .canonical_key::<_, PropertyAccessorKey>(id)?;
            let PropertyOwner::Property(property) = accessor.owner() else {
                return Err(Error::CallableContract(declaration));
            };

            let key = metadata
                .identities
                .canonical_key::<_, SourceDeclarationKey>(property)?;
            let declaration = match accessor.role() {
                AccessorRole::Getter => Declaration::Getter(id),
                AccessorRole::Setter => Declaration::Setter(id),
            };

            schemas.properties.insert(property, key);
            schemas.accessors.insert(id, accessor);
            Ok(declaration)
        }
        _ => Err(Error::CallableContract(declaration)),
    }
}
