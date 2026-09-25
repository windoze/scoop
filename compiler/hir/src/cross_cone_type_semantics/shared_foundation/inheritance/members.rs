use super::*;
use crate::{
    CanonicalProtectedDeclarationRefsV1, DeclaredVisibilityV1, NestedSourceMemberRefV1,
    NominalInheritanceInterfaceV1, NominalInterfaceRecordV1, ProtectedCallableDeclarationRefV1,
    ProtectedDeclarationRefV1,
};
use scoop_identity::{CallableTemplateOrigin, DefinitionOriginSubject, PropertyOwner};

pub(super) fn validate(
    provider: CheckedSharedTypeFoundationV1<'_>,
    nominal: &NominalInterfaceRecordV1,
    record: &NominalInheritanceInterfaceV1,
    context: &Context<'_>,
) -> Result<(), Error> {
    let required = required(provider.metadata, nominal, context)?;

    if record.protected_members() != &required {
        return Err(Error::ProtectedMemberInventory(record.owner()));
    }
    Ok(())
}

fn required(
    metadata: SharedTypeMetadataV1<'_>,
    nominal: &NominalInterfaceRecordV1,
    context: &Context<'_>,
) -> Result<CanonicalProtectedDeclarationRefsV1, Error> {
    let path = WirePath::root();
    let mut references = Vec::new();
    for member in nominal.declaration_details().members().values() {
        match *member {
            NestedSourceMemberRefV1::Function(id) => {
                push_callable(
                    metadata,
                    CallableTemplateOrigin::Function(id),
                    &mut references,
                )?;
            }
            NestedSourceMemberRefV1::GenericFunction(id) => {
                push_callable(
                    metadata,
                    CallableTemplateOrigin::GenericFunction(id),
                    &mut references,
                )?;
            }
            NestedSourceMemberRefV1::Property(id) => {
                let table = metadata.public.property_interfaces();

                let property = table.declaration(PropertyOwner::Property(id)).ok_or(
                    Error::DeclarationMetadata(DefinitionOriginSubject::Property(id)),
                )?;
                if property.declared_visibility() == DeclaredVisibilityV1::Protected {
                    scoop_wire::allocation::try_reserve(&mut references, 1, &path)?;
                    references.push(ProtectedDeclarationRefV1::Property(id));
                }
                if property.representation() == crate::PropertyRepresentationV1::Const {
                    continue;
                }
                let accessors = property.accessors();
                for id in std::iter::once(accessors.getter()).chain(accessors.setter()) {
                    push_callable(
                        metadata,
                        CallableTemplateOrigin::Accessor(id),
                        &mut references,
                    )?;
                }
            }
        }
    }
    for child in nominal.declaration_details().children().values() {
        if context.source(*child)?.access.declared_visibility() == DeclaredVisibilityV1::Protected {
            scoop_wire::allocation::try_reserve(&mut references, 1, &path)?;
            references.push(ProtectedDeclarationRefV1::NestedNominal(*child));
        }
    }

    CanonicalProtectedDeclarationRefsV1::try_new(references)
        .map_err(|_| Error::InheritanceSource(nominal.declaration()))
}

fn push_callable(
    metadata: SharedTypeMetadataV1<'_>,
    id: CallableTemplateOrigin,
    references: &mut Vec<ProtectedDeclarationRefV1>,
) -> Result<(), Error> {
    if contracts::callable(metadata, id)?.declared_visibility() == DeclaredVisibilityV1::Protected {
        let reference = ProtectedCallableDeclarationRefV1::try_new(id)
            .map_err(|_| Error::CallableContract(id))?;
        scoop_wire::allocation::try_reserve(references, 1, &WirePath::root())?;
        references.push(ProtectedDeclarationRefV1::Callable(reference));
    }
    Ok(())
}
