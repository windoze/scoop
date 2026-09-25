//! Complete protected and nested source records joined to shared declarations.

use super::*;
use crate::{
    DeclaredVisibilityV1, NominalSourcePropertyPayloadV1, ProtectedDeclarationInterfaceV1,
    ProtectedDeclarationRefV1,
};
use scoop_identity::{
    CallableTemplateOrigin, DefinitionOriginSubject, PersistentPropertyId, PropertyOwner,
};

mod inventory;
mod nested;
mod properties;

pub(super) fn validate<'a>(
    provider: CheckedSharedTypeFoundationV1<'a>,
    dependencies: &[CheckedSharedTypeFoundationV1<'a>],
    context: &Context<'_>,
    graph: &CheckedNominalInheritanceGraphV1<'_>,
) -> Result<(), Error> {
    inventory::validate(provider)?;
    for record in provider.section.protected_declarations().records() {
        let source = record.declaration_access();

        let owner = source
            .lexical_owners()
            .last()
            .copied()
            .ok_or(Error::ProtectedMember(record.reference()))?;
        if context.source(owner)?.key.declaration_kind()
            != scoop_identity::SourceDeclarationKind::Class
        {
            return Err(Error::ProtectedMember(record.reference()));
        }
        match record {
            ProtectedDeclarationInterfaceV1::Callable(callable) => contracts::validate_callable(
                provider.metadata,
                callable.declaration(),
                source,
                callable.payload(),
            )?,
            ProtectedDeclarationInterfaceV1::Constructor(constructor) => {
                contracts::validate_callable(
                    provider.metadata,
                    CallableTemplateOrigin::Constructor(constructor.declaration()),
                    source,
                    constructor.payload(),
                )?
            }
            ProtectedDeclarationInterfaceV1::Property(property) => properties::validate(
                provider.metadata,
                property.declaration(),
                source,
                property.payload(),
            )?,
            ProtectedDeclarationInterfaceV1::NestedNominal(nominal) => nested::validate(
                provider,
                dependencies,
                nominal.source_record(),
                context,
                graph,
            )?,
        }
    }
    Ok(())
}

fn property<'a>(
    metadata: SharedTypeMetadataV1<'a>,
    id: PersistentPropertyId,
) -> Result<&'a crate::PropertyDeclarationRecordV1, Error> {
    let table = metadata.public.property_interfaces();

    table
        .declaration(PropertyOwner::Property(id))
        .ok_or(Error::PropertyContract(id))
}

fn property_access(
    metadata: SharedTypeMetadataV1<'_>,
    id: PersistentPropertyId,
) -> Result<DeclarationAccessSourceV1, Error> {
    let source = property(metadata, id)?;

    let key = metadata
        .identities
        .canonical_key::<_, SourceDeclarationKey>(id)?;
    super::super::sources::source_access(
        metadata,
        DefinitionOriginSubject::Property(id),
        &key,
        source.declared_visibility(),
    )
}
